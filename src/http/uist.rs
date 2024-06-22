pub mod uistv1_client {

    use std::future::{self, Future};

    use anyhow::{Error, Result};

    use super::uistv1_server::{
        AppState, DeleteOrderRequest, FetchQuotesResponse, InfoResponse, InitResponse, InsertOrderRequest, TickResponse, UistV1Error
    };

    use crate::exchange::uist_v1::{Order, OrderId, UistV1};

    pub type BacktestId = u64;

    pub trait UistClient {
        fn tick(&mut self, backtest_id: BacktestId) -> impl Future<Output = Result<TickResponse>>;
        fn delete_order(&mut self, order_id: OrderId, backtest_id: BacktestId) -> impl Future<Output = Result<()>>;
        fn insert_order(&mut self, order: Order, backtest_id: BacktestId) -> impl Future<Output = Result<()>>;
        fn fetch_quotes(&mut self, backtest_id: BacktestId) -> impl Future<Output = Result<FetchQuotesResponse>>;
        fn init(&mut self, dataset_name: String) -> impl Future<Output = Result<InitResponse>>;
        fn info(&mut self, backtest_id: BacktestId) -> impl Future<Output = Result<InfoResponse>>;
    }

    pub struct TestClient {
        state: AppState
    }

    impl UistClient for TestClient {
        fn init(&mut self, dataset_name: String) -> impl Future<Output = Result<InitResponse>> {
            if let Some(backtest) = self.state.new_backtest(dataset_name) {
                return future::ready(Ok(InitResponse {
                    backtest_id: backtest.0,
                    start: backtest.1.start,
                    frequency: backtest.1.frequency,
                }));
            } else {
                return future::ready(Err(Error::new(UistV1Error::UnknownDataset)));
            }
        }

        fn tick(&mut self, backtest_id: BacktestId) -> impl Future<Output = Result<TickResponse>> {
            if let Some(state) = self.state.exchanges.get_mut(&backtest_id) {
                let tick = state.exchange.tick();
                future::ready(Ok(TickResponse {
                    inserted_orders: tick.2,
                    executed_trades: tick.1,
                    has_next: tick.0,
                }))
            } else {
                return future::ready(Err(Error::new(UistV1Error::UnknownBacktest)));
            }
        }

        fn insert_order(&mut self, order: Order, backtest_id: BacktestId) -> impl Future<Output = Result<()>> {
            let insert_order_request = InsertOrderRequest { order };
            if let Some(state) = self.state.exchanges.get_mut(&backtest_id) {
                state.exchange.insert_order(insert_order_request.order.clone());
                future::ready(Ok(()))
            } else {
                return future::ready(Err(Error::new(UistV1Error::UnknownBacktest)));
            }
        }

        fn delete_order(&mut self, order_id: OrderId, backtest_id: BacktestId) -> impl Future<Output = Result<()>> {
            let delete_order_request = DeleteOrderRequest { order_id };
            if let Some(state) = self.state.exchanges.get_mut(&backtest_id) {
                state.exchange.delete_order(delete_order_request.order_id.clone());
                future::ready(Ok(()))
            } else {
                return future::ready(Err(Error::new(UistV1Error::UnknownBacktest)));
            }
        }

        fn fetch_quotes(&mut self, backtest_id: BacktestId) -> impl Future<Output = Result<FetchQuotesResponse>> {
            if let Some(state) = self.state.exchanges.get_mut(&backtest_id) {
                future::ready(Ok(FetchQuotesResponse {
                    quotes: state.exchange.fetch_quotes(),
                }))
            } else {
                return future::ready(Err(Error::new(UistV1Error::UnknownBacktest)));
            }   
        }

        fn info(&mut self, backtest_id: BacktestId) -> impl Future<Output = Result<InfoResponse>> {
            if let Some(state) = self.state.exchanges.get_mut(&backtest_id) {
                let info = state.exchange.info();
                future::ready(Ok(InfoResponse {
                    version: info.version,
                    dataset: info.dataset,
                }))
            } else {
                return future::ready(Err(Error::new(UistV1Error::UnknownBacktest)));
            }
        }
    }

    impl TestClient {
        pub fn new(datasets: &mut std::collections::HashMap<String, UistV1>) -> Self {
            Self {
                state: AppState::create(datasets)
            }
        }
    }

    #[derive(Debug)]
    pub struct Client {
        pub path: String,
        pub client: reqwest::Client,
    }

    impl UistClient for Client {
        async fn tick(&mut self, backtest_id: BacktestId) -> Result<TickResponse> {
            Ok(self.client
                .get(self.path.clone() + format!("/backtest/{backtest_id}/tick").as_str())
                .send()
                .await?
                .json::<TickResponse>()
                .await?)
        }

        async fn delete_order(&mut self, order_id: OrderId, backtest_id: BacktestId) -> Result<()> {
            let req = DeleteOrderRequest { order_id };
            Ok(self.client
                .post(self.path.clone() + format!("/backtest/{backtest_id}/delete_order").as_str())
                .json(&req)
                .send()
                .await?
                .json::<()>()
                .await?)
        }

        async fn insert_order(&mut self, order: Order, backtest_id: BacktestId) -> Result<()> {
            let req = InsertOrderRequest { order };
            Ok(self.client
                .post(self.path.clone() + format!("/backtest/{backtest_id}/insert_order").as_str())
                .json(&req)
                .send()
                .await?
                .json::<()>()
                .await?)
        }

        async fn fetch_quotes(&mut self, backtest_id: BacktestId) -> Result<FetchQuotesResponse> {
            Ok(self.client
                .get(self.path.clone() + format!("/backtest/{backtest_id}/fetch_quotes").as_str())
                .send()
                .await?
                .json::<FetchQuotesResponse>()
                .await?)
        }

        async fn init(&mut self, dataset_name: String) -> Result<InitResponse> {
            Ok(self.client
                .get(self.path.clone() + format!("/init/{dataset_name}").as_str())
                .send()
                .await?
                .json::<InitResponse>()
                .await?)
        }

        async fn info(&mut self, backtest_id: BacktestId) -> Result<InfoResponse> {
            Ok(self.client
                .get(self.path.clone() + format!("/backtest/{backtest_id}/info").as_str())
                .send()
                .await?
                .json::<InfoResponse>()
                .await?)
        }
    }

    impl Client {
        pub fn new(path: String) -> Self {
            Self {
                path,
                client: reqwest::Client::new(),
            }
        }
    }
}

pub mod uistv1_server {
    use serde::{Deserialize, Serialize};
    use core::fmt;
    use std::{collections::HashMap, error::Error, sync::Mutex};

    use crate::exchange::uist_v1::{InitMessage, Order, OrderId, Trade, UistQuote, UistV1};
    use actix_web::{get, post, web, ResponseError};

    type BacktestId = u64;
    pub type UistState = Mutex<AppState>;


    pub struct BacktestState {
        pub id: BacktestId,
        pub position: i64,
        pub exchange: UistV1,
    }

    pub struct AppState {
        pub exchanges: HashMap<BacktestId, BacktestState>,
        pub last: BacktestId,
        pub datasets: HashMap<String, UistV1>,
    }

    impl AppState {
        pub fn create(datasets: &mut HashMap<String, UistV1>) -> Self {
            Self {
                exchanges: HashMap::new(),
                last: 0,
                datasets: std::mem::take(datasets),
            }
        }

        pub fn new_backtest(&mut self, dataset_name: String) -> Option<(BacktestId, InitMessage)> {
            let new_id = self.last + 1;

            if let Some(exchange) = self.datasets.get(&dataset_name) {
                // Not efficient but it is easier than breaking the bind between the dataset and
                // the exchange.
                let copied_exchange = exchange.clone();

                let init_message = copied_exchange.init();

                let backtest = BacktestState {
                    id: new_id,
                    position: 0,
                    exchange: copied_exchange,
                };

                self.exchanges.insert(new_id, backtest);

                self.last = new_id;
                return Some((new_id, init_message));
            }
            None
        }
    }

    #[derive(Debug)]
    pub enum UistV1Error {
        UnknownBacktest,
        UnknownDataset,
    }

    impl Error for UistV1Error { }

    impl fmt::Display for UistV1Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                UistV1Error::UnknownBacktest => write!(f, "UnknownBacktest"),
                UistV1Error::UnknownDataset => write!(f, "UnknownDataset"),
            }
        }
    }

    impl ResponseError for UistV1Error {
        fn status_code(&self) -> actix_web::http::StatusCode {
            match self {
                UistV1Error::UnknownBacktest => actix_web::http::StatusCode::BAD_REQUEST,
                UistV1Error::UnknownDataset => actix_web::http::StatusCode::BAD_REQUEST,
            }
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct TickResponse {
        pub has_next: bool,
        pub executed_trades: Vec<Trade>,
        pub inserted_orders: Vec<Order>,
    }

    #[get("/backtest/{backtest_id}/tick")]
    pub async fn tick(
        app: web::Data<UistState>,
        path: web::Path<(BacktestId,)>,
    ) -> Result<web::Json<TickResponse>, UistV1Error> {
        let mut uist = app.lock().unwrap();
        let (backtest_id,) = path.into_inner();

        if let Some(state) = uist.exchanges.get_mut(&backtest_id) {
            let tick = state.exchange.tick();
            Ok(web::Json(TickResponse {
                inserted_orders: tick.2,
                executed_trades: tick.1,
                has_next: tick.0,
            }))
        } else {
            Err(UistV1Error::UnknownBacktest)
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct DeleteOrderRequest {
        pub order_id: OrderId,
    }

    #[post("/backtest/{backtest_id}/delete_order")]
    pub async fn delete_order(
        app: web::Data<UistState>,
        path: web::Path<(BacktestId,)>,
        delete_order: web::Json<DeleteOrderRequest>,
    ) -> Result<web::Json<()>, UistV1Error> {
        let mut uist = app.lock().unwrap();
        let (backtest_id,) = path.into_inner();

        if let Some(state) = uist.exchanges.get_mut(&backtest_id) {
            state.exchange.delete_order(delete_order.order_id);
            Ok(web::Json(()))
        } else {
            Err(UistV1Error::UnknownBacktest)
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct InsertOrderRequest {
        pub order: Order,
    }

    #[post("/backtest/{backtest_id}/insert_order")]
    pub async fn insert_order(
        app: web::Data<UistState>,
        path: web::Path<(BacktestId,)>,
        insert_order: web::Json<InsertOrderRequest>,
    ) -> Result<web::Json<()>, UistV1Error> {
        let mut uist = app.lock().unwrap();
        let (backtest_id,) = path.into_inner();
        if let Some(state) = uist.exchanges.get_mut(&backtest_id) {
            state.exchange.insert_order(insert_order.order.clone());
            Ok(web::Json(()))
        } else {
            Err(UistV1Error::UnknownBacktest)
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct FetchQuotesResponse {
        pub quotes: Vec<UistQuote>,
    }

    #[get("/backtest/{backtest_id}/fetch_quotes")]
    pub async fn fetch_quotes(
        app: web::Data<UistState>,
        path: web::Path<(BacktestId,)>,
    ) -> Result<web::Json<FetchQuotesResponse>, UistV1Error> {
        let mut uist = app.lock().unwrap();
        let (backtest_id,) = path.into_inner();

        if let Some(state) = uist.exchanges.get_mut(&backtest_id) {
            Ok(web::Json(FetchQuotesResponse {
                quotes: state.exchange.fetch_quotes(),
            }))
        } else {
            Err(UistV1Error::UnknownBacktest)
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct InitResponse {
        pub backtest_id: BacktestId,
        pub start: i64,
        pub frequency: u8,
    }

    #[get("/init/{dataset_name}")]
    pub async fn init(
        app: web::Data<UistState>,
        path: web::Path<(String,)>,
    ) -> Result<web::Json<InitResponse>, UistV1Error> {
        let mut uist = app.lock().unwrap();
        let (dataset_name,) = path.into_inner();

        if let Some(backtest) = uist.new_backtest(dataset_name) {
            Ok(web::Json(InitResponse {
                backtest_id: backtest.0,
                start: backtest.1.start,
                frequency: backtest.1.frequency,
            }))
        } else {
            Err(UistV1Error::UnknownDataset)
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct InfoResponse {
        pub version: String,
        pub dataset: String,
    }

    #[get("/backtest/{backtest_id}/info")]
    pub async fn info(
        app: web::Data<UistState>,
        path: web::Path<(BacktestId,)>,
    ) -> Result<web::Json<InfoResponse>, UistV1Error> {
        let mut uist = app.lock().unwrap();
        let (backtest_id,) = path.into_inner();
        if let Some(state) = uist.exchanges.get_mut(&backtest_id) {
            let info = state.exchange.info();
            Ok(web::Json(InfoResponse {
                version: info.version,
                dataset: info.dataset,
            }))
        } else {
            Err(UistV1Error::UnknownBacktest)
        }
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{test, web, App};

    use crate::exchange::uist_v1::{random_uist_generator, Order};

    use super::uistv1_server::*;
    use std::{collections::HashMap, sync::Mutex};

    #[actix_web::test]
    async fn test_single_trade_loop() {
        let uist = random_uist_generator(3000);
        let dataset_name = "random";

        let mut datasets = HashMap::new();
        datasets.insert(dataset_name.to_string(), uist.0);

        let app_state = Mutex::new(AppState::create(&mut datasets));
        let uist_state = web::Data::new(app_state);

        let app = test::init_service(
            App::new()
                .app_data(uist_state)
                .service(info)
                .service(init)
                .service(fetch_quotes)
                .service(tick)
                .service(insert_order)
                .service(delete_order),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(format!("/init/{dataset_name}").as_str())
            .to_request();
        let resp: InitResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.frequency == 0);

        let backtest_id = resp.backtest_id;

        let req1 = test::TestRequest::get()
            .uri(format!("/backtest/{backtest_id}/fetch_quotes").as_str())
            .to_request();
        let _resp1: FetchQuotesResponse = test::call_and_read_body_json(&app, req1).await;

        let req2 = test::TestRequest::get()
            .uri(format!("/backtest/{backtest_id}/tick").as_str())
            .to_request();
        let _resp2: TickResponse = test::call_and_read_body_json(&app, req2).await;

        let req3 = test::TestRequest::post()
            .set_json(InsertOrderRequest {
                order: Order::market_buy("ABC", 100.0),
            })
            .uri(format!("/backtest/{backtest_id}/insert_order").as_str())
            .to_request();
        test::call_and_read_body(&app, req3).await;

        let req4 = test::TestRequest::get()
            .uri(format!("/backtest/{backtest_id}/tick").as_str())
            .to_request();
        let resp4: TickResponse = test::call_and_read_body_json(&app, req4).await;

        assert!(resp4.executed_trades.len() == 1);
        assert!(resp4.executed_trades.get(0).unwrap().symbol == "ABC")
    }
}
