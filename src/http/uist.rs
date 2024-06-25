use std::collections::HashMap;

use crate::exchange::uist_v1::{Order, OrderId, Trade, UistV1};
use crate::input::penelope::{Penelope, PenelopeQuoteByDate};

type BacktestId = u64;

pub struct BacktestState {
    pub id: BacktestId,
    pub date: i64,
    pub exchange: UistV1,
    pub dataset_name: String,
}

pub struct AppState {
    pub backtests: HashMap<BacktestId, BacktestState>,
    pub last: BacktestId,
    pub datasets: HashMap<String, Penelope>,
}

impl AppState {
    pub fn create(datasets: &mut HashMap<String, Penelope>) -> Self {
        Self {
            backtests: HashMap::new(),
            last: 0,
            datasets: std::mem::take(datasets),
        }
    }

    pub fn single(name: &str, data: Penelope) -> Self {
        let exchange = UistV1::new();
        let backtest = BacktestState {
            id: 0,
            date: *data.get_first_date(),
            exchange,
            dataset_name: name.into(),
        };

        let mut datasets = HashMap::new();
        datasets.insert(name.into(), data);

        let mut backtests = HashMap::new();
        backtests.insert(0, backtest);

        Self {
            backtests,
            last: 1,
            datasets,
        }
    }

    pub fn tick(&mut self, backtest_id: BacktestId) -> Option<(bool, Vec<Trade>, Vec<Order>)> {
        if let Some(backtest) = self.backtests.get_mut(&backtest_id) {
            if let Some(dataset) = self.datasets.get(&backtest.dataset_name) {
                if let Some(quotes) = dataset.get_quotes(&backtest.date) {
                    let res = backtest.exchange.tick(quotes);
                    let mut has_next = false;
                    if let Some(next_date) = dataset.get_next_date(&backtest.date) {
                        has_next = true;
                        backtest.date = *next_date;
                    }
                    return Some((has_next, res.0, res.1));
                }
            }
        }
        None
    }

    pub fn fetch_quotes(&self, backtest_id: BacktestId) -> Option<&PenelopeQuoteByDate> {
        if let Some(backtest) = self.backtests.get(&backtest_id) {
            if let Some(dataset) = self.datasets.get(&backtest.dataset_name) {
                return dataset.get_quotes(&backtest.date);
            }
        }
        None
    }

    pub fn init(&mut self, dataset_name: String) -> Option<BacktestId> {
        if let Some(dataset) = self.datasets.get(&dataset_name) {
            let new_id = self.last + 1;
            let exchange = UistV1::new();
            let backtest = BacktestState {
                id: new_id,
                date: *dataset.get_first_date(),
                exchange,
                dataset_name,
            };
            self.backtests.insert(new_id, backtest);
            return Some(new_id);
        }
        None
    }

    pub fn insert_order(&mut self, order: Order, backtest_id: BacktestId) -> Option<()> {
        if let Some(backtest) = self.backtests.get_mut(&backtest_id) {
            backtest.exchange.insert_order(order);
            return Some(());
        }
        None
    }

    pub fn delete_order(&mut self, order_id: OrderId, backtest_id: BacktestId) -> Option<()> {
        if let Some(backtest) = self.backtests.get_mut(&backtest_id) {
            backtest.exchange.delete_order(order_id);
            return Some(());
        }
        None
    }

    pub fn new_backtest(&mut self, dataset_name: &str) -> Option<BacktestId> {
        let new_id = self.last + 1;

        // Check that dataset exists
        if let Some(dataset) = self.datasets.get(dataset_name) {
            let exchange = UistV1::new();

            let backtest = BacktestState {
                id: new_id,
                date: *dataset.get_first_date(),
                exchange,
                dataset_name: dataset_name.into(),
            };

            self.backtests.insert(new_id, backtest);

            self.last = new_id;
            return Some(new_id);
        }
        None
    }
}

pub mod uistv1_client {

    use std::future;
    use std::future::Future;

    use anyhow::{Error, Result};

    use super::uistv1_server::{
        DeleteOrderRequest, FetchQuotesResponse, InfoResponse, InitResponse, InsertOrderRequest, NowResponse, TickResponse, UistV1Error
    };
    use super::AppState;

    use crate::exchange::uist_v1::{Order, OrderId};
    use crate::input::penelope::Penelope;

    pub type BacktestId = u64;

    pub trait UistClient {
        fn tick(&mut self, backtest_id: BacktestId) -> impl Future<Output = Result<TickResponse>>;
        fn delete_order(
            &mut self,
            order_id: OrderId,
            backtest_id: BacktestId,
        ) -> impl Future<Output = Result<()>>;
        fn insert_order(
            &mut self,
            order: Order,
            backtest_id: BacktestId,
        ) -> impl Future<Output = Result<()>>;
        fn fetch_quotes(
            &mut self,
            backtest_id: BacktestId,
        ) -> impl Future<Output = Result<FetchQuotesResponse>>;
        fn init(&mut self, dataset_name: String) -> impl Future<Output = Result<InitResponse>>;
        fn info(&mut self, backtest_id: BacktestId) -> impl Future<Output = Result<InfoResponse>>;
        fn now(&mut self, backtest_id: BacktestId) -> impl Future<Output = Result<NowResponse>>;
    }

    pub struct TestClient {
        state: AppState,
    }

    impl UistClient for TestClient {
        fn init(&mut self, dataset_name: String) -> impl Future<Output = Result<InitResponse>> {
            if let Some(id) = self.state.init(dataset_name) {
                future::ready(Ok(InitResponse { backtest_id: id }))
            } else {
                future::ready(Err(Error::new(UistV1Error::UnknownDataset)))
            }
        }

        fn tick(&mut self, backtest_id: BacktestId) -> impl Future<Output = Result<TickResponse>> {
            if let Some(resp) = self.state.tick(backtest_id) {
                future::ready(Ok(TickResponse {
                    inserted_orders: resp.2,
                    executed_trades: resp.1,
                    has_next: resp.0,
                }))
            } else {
                future::ready(Err(Error::new(UistV1Error::UnknownBacktest)))
            }
        }

        fn insert_order(
            &mut self,
            order: Order,
            backtest_id: BacktestId,
        ) -> impl Future<Output = Result<()>> {
            if let Some(()) = self.state.insert_order(order, backtest_id) {
                future::ready(Ok(()))
            } else {
                future::ready(Err(Error::new(UistV1Error::UnknownBacktest)))
            }
        }

        fn delete_order(
            &mut self,
            order_id: OrderId,
            backtest_id: BacktestId,
        ) -> impl Future<Output = Result<()>> {
            if let Some(()) = self.state.delete_order(order_id, backtest_id) {
                future::ready(Ok(()))
            } else {
                future::ready(Err(Error::new(UistV1Error::UnknownBacktest)))
            }
        }

        fn fetch_quotes(
            &mut self,
            backtest_id: BacktestId,
        ) -> impl Future<Output = Result<FetchQuotesResponse>> {
            if let Some(quotes) = self.state.fetch_quotes(backtest_id) {
                future::ready(Ok(FetchQuotesResponse {
                    quotes: quotes.to_owned(),
                }))
            } else {
                future::ready(Err(Error::new(UistV1Error::UnknownBacktest)))
            }
        }

        fn info(&mut self, backtest_id: BacktestId) -> impl Future<Output = Result<InfoResponse>> {
            if let Some(backtest) = self.state.backtests.get(&backtest_id) {
                future::ready(Ok(InfoResponse {
                    version: "v1".to_string(),
                    dataset: backtest.dataset_name.clone(),
                }))
            } else {
                future::ready(Err(Error::new(UistV1Error::UnknownBacktest)))
            }
        }

        fn now(&mut self, backtest_id: BacktestId) -> impl Future<Output = Result<NowResponse>> {
            if let Some(backtest) = self.state.backtests.get(&backtest_id) {
                if let Some(dataset) = self.state.datasets.get(&backtest.dataset_name) {
                    let now = backtest.date;
                    let mut has_next = false;
                    if let Some(_has_next) = dataset.get_next_date(&now) {
                        has_next = true;
                    }
                    future::ready(Ok(NowResponse {
                        now,
                        has_next
                    }))
                } else {
                    future::ready(Err(Error::new(UistV1Error::UnknownDataset)))
                }
            } else {
                future::ready(Err(Error::new(UistV1Error::UnknownBacktest)))
            }
        }
    }

    impl TestClient {
        pub fn single(name: &str, data: Penelope) -> Self {
            Self {
                state: AppState::single(name, data),
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
            Ok(self
                .client
                .get(self.path.clone() + format!("/backtest/{backtest_id}/tick").as_str())
                .send()
                .await?
                .json::<TickResponse>()
                .await?)
        }

        async fn delete_order(&mut self, order_id: OrderId, backtest_id: BacktestId) -> Result<()> {
            let req = DeleteOrderRequest { order_id };
            Ok(self
                .client
                .post(self.path.clone() + format!("/backtest/{backtest_id}/delete_order").as_str())
                .json(&req)
                .send()
                .await?
                .json::<()>()
                .await?)
        }

        async fn insert_order(&mut self, order: Order, backtest_id: BacktestId) -> Result<()> {
            let req = InsertOrderRequest { order };
            Ok(self
                .client
                .post(self.path.clone() + format!("/backtest/{backtest_id}/insert_order").as_str())
                .json(&req)
                .send()
                .await?
                .json::<()>()
                .await?)
        }

        async fn fetch_quotes(&mut self, backtest_id: BacktestId) -> Result<FetchQuotesResponse> {
            Ok(self
                .client
                .get(self.path.clone() + format!("/backtest/{backtest_id}/fetch_quotes").as_str())
                .send()
                .await?
                .json::<FetchQuotesResponse>()
                .await?)
        }

        async fn init(&mut self, dataset_name: String) -> Result<InitResponse> {
            Ok(self
                .client
                .get(self.path.clone() + format!("/init/{dataset_name}").as_str())
                .send()
                .await?
                .json::<InitResponse>()
                .await?)
        }

        async fn info(&mut self, backtest_id: BacktestId) -> Result<InfoResponse> {
            Ok(self
                .client
                .get(self.path.clone() + format!("/backtest/{backtest_id}/info").as_str())
                .send()
                .await?
                .json::<InfoResponse>()
                .await?)
        }

        async fn now(&mut self, backtest_id: BacktestId) -> Result<NowResponse> {
            Ok(self
                .client
                .get(self.path.clone() + format!("/backtest/{backtest_id}/now").as_str())
                .send()
                .await?
                .json::<NowResponse>()
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
    use core::fmt;
    use serde::{Deserialize, Serialize};
    use std::{error::Error, sync::Mutex};

    use crate::exchange::uist_v1::{Order, OrderId, Trade};
    use crate::input::penelope::PenelopeQuoteByDate;
    use actix_web::{get, post, web, ResponseError};

    use super::{AppState, BacktestId};

    pub type UistState = Mutex<AppState>;

    #[derive(Debug)]
    pub enum UistV1Error {
        UnknownBacktest,
        UnknownDataset,
    }

    impl Error for UistV1Error {}

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

        if let Some(result) = uist.tick(backtest_id) {
            Ok(web::Json(TickResponse {
                inserted_orders: result.2,
                executed_trades: result.1,
                has_next: result.0,
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

        if let Some(()) = uist.delete_order(delete_order.order_id, backtest_id) {
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
        if let Some(()) = uist.insert_order(insert_order.order.clone(), backtest_id) {
            Ok(web::Json(()))
        } else {
            Err(UistV1Error::UnknownBacktest)
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct FetchQuotesResponse {
        pub quotes: PenelopeQuoteByDate,
    }

    #[get("/backtest/{backtest_id}/fetch_quotes")]
    pub async fn fetch_quotes(
        app: web::Data<UistState>,
        path: web::Path<(BacktestId,)>,
    ) -> Result<web::Json<FetchQuotesResponse>, UistV1Error> {
        let uist = app.lock().unwrap();
        let (backtest_id,) = path.into_inner();

        if let Some(quotes) = uist.fetch_quotes(backtest_id) {
            Ok(web::Json(FetchQuotesResponse {
                quotes: quotes.clone(),
            }))
        } else {
            Err(UistV1Error::UnknownBacktest)
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct InitResponse {
        pub backtest_id: BacktestId,
    }

    #[get("/init/{dataset_name}")]
    pub async fn init(
        app: web::Data<UistState>,
        path: web::Path<(String,)>,
    ) -> Result<web::Json<InitResponse>, UistV1Error> {
        let mut uist = app.lock().unwrap();
        let (dataset_name,) = path.into_inner();

        if let Some(backtest_id) = uist.init(dataset_name) {
            Ok(web::Json(InitResponse { backtest_id }))
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
        let uist = app.lock().unwrap();
        let (backtest_id,) = path.into_inner();

        if let Some(resp) = uist.backtests.get(&backtest_id) {
            Ok(web::Json(InfoResponse {
                version: "v1".to_string(),
                dataset: resp.dataset_name.clone(),
            }))
        } else {
            Err(UistV1Error::UnknownBacktest)
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct NowResponse {
        pub now: i64,
        pub has_next: bool,
    }

    #[get("/backtest/{backtest_id}/now")]
    pub async fn now(
        app: web::Data<UistState>,
        path: web::Path<(BacktestId,)>,
    ) -> Result<web::Json<NowResponse>, UistV1Error> {
        let uist = app.lock().unwrap();
        let (backtest_id,) = path.into_inner();

        if let Some(backtest) = uist.backtests.get(&backtest_id) {
            let now = backtest.date;
            if let Some(dataset) = uist.datasets.get(&backtest.dataset_name) {
                let mut has_next = false;
                if let Some(_has_next) = dataset.get_next_date(&now) {
                    has_next = true;
                }
                return Ok(web::Json(NowResponse {
                    now,
                    has_next,
                }));
            } else {
                Err(UistV1Error::UnknownDataset)
            }
        } else {
            Err(UistV1Error::UnknownBacktest)
        }
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{test, web, App};

    use crate::exchange::uist_v1::Order;
    use crate::input::penelope::Penelope;

    use super::uistv1_server::*;
    use super::AppState;
    use std::sync::Mutex;

    #[actix_web::test]
    async fn test_single_trade_loop() {
        let uist = Penelope::random(100, vec!["ABC", "BCD"]);
        let dataset_name = "fake";
        let state = AppState::single(dataset_name, uist);

        let app_state = Mutex::new(state);
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
        let _resp4: TickResponse = test::call_and_read_body_json(&app, req4).await;

        let req5 = test::TestRequest::get()
            .uri(format!("/backtest/{backtest_id}/tick").as_str())
            .to_request();
        let resp5: TickResponse = test::call_and_read_body_json(&app, req5).await;

        assert!(resp5.executed_trades.len() == 1);
        assert!(resp5.executed_trades.get(0).unwrap().symbol == "ABC")
    }
}
