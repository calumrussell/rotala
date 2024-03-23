pub mod uistv1_client {

    use reqwest::Result;

    use super::uistv1_server::{
        DeleteOrderRequest, FetchQuotesResponse, InsertOrderRequest, TickResponse,
    };

    use crate::exchange::uist_v1::{InfoMessage, InitMessage, Order, OrderId};

    pub struct Client {
        pub path: String,
        pub client: reqwest::Client,
    }

    impl Client {
        pub async fn tick(&self) -> Result<TickResponse> {
            reqwest::get(self.path.clone() + "/tick")
                .await?
                .json::<TickResponse>()
                .await
        }

        pub async fn delete_order(&self, order_id: OrderId) -> Result<()> {
            let req = DeleteOrderRequest { order_id };
            self.client
                .post(self.path.clone() + "/delete_order")
                .json(&req)
                .send()
                .await?
                .json::<()>()
                .await
        }

        pub async fn insert_order(&self, order: Order) -> Result<()> {
            let req = InsertOrderRequest { order };
            self.client
                .post(self.path.clone() + "/insert_order")
                .json(&req)
                .send()
                .await?
                .json::<()>()
                .await
        }

        pub async fn fetch_quotes(&self) -> Result<FetchQuotesResponse> {
            reqwest::get(self.path.clone() + "/fetch_quotes")
                .await?
                .json::<FetchQuotesResponse>()
                .await
        }

        pub async fn init(&self) -> Result<InitMessage> {
            reqwest::get(self.path.clone() + "/init")
                .await?
                .json::<InitMessage>()
                .await
        }

        pub async fn info(&self) -> Result<InfoMessage> {
            reqwest::get(self.path.clone() + "/")
                .await?
                .json::<InfoMessage>()
                .await
        }

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
    use std::sync::Mutex;

    use crate::exchange::uist_v1::{Order, OrderId, Trade, UistQuote, UistV1};
    use actix_web::web;

    pub struct AppState {
        pub exchange: Mutex<UistV1>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct TickResponse {
        pub has_next: bool,
        pub executed_trades: Vec<Trade>,
        pub inserted_orders: Vec<Order>,
    }

    pub async fn tick(app: web::Data<AppState>) -> web::Json<TickResponse> {
        let mut ex = app.exchange.lock().unwrap();

        let tick = ex.tick();
        web::Json(TickResponse {
            inserted_orders: tick.2,
            executed_trades: tick.1,
            has_next: tick.0,
        })
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct DeleteOrderRequest {
        pub order_id: OrderId,
    }

    pub async fn delete_order(
        app: web::Data<AppState>,
        delete_order: web::Json<DeleteOrderRequest>,
    ) -> web::Json<()> {
        let mut ex = app.exchange.lock().unwrap();
        ex.delete_order(delete_order.order_id);
        web::Json(())
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct InsertOrderRequest {
        pub order: Order,
    }

    pub async fn insert_order(
        app: web::Data<AppState>,
        insert_order: web::Json<InsertOrderRequest>,
    ) -> web::Json<()> {
        dbg!(&insert_order);
        let mut ex = app.exchange.lock().unwrap();
        ex.insert_order(insert_order.order.clone());
        web::Json(())
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct FetchTradesRequest {
        pub from: OrderId,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct FetchQuotesResponse {
        pub quotes: Vec<UistQuote>,
    }

    pub async fn fetch_quotes(app: web::Data<AppState>) -> web::Json<FetchQuotesResponse> {
        let ex = app.exchange.lock().unwrap();
        web::Json(FetchQuotesResponse {
            quotes: ex.fetch_quotes(),
        })
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct InitResponse {
        pub start: i64,
        pub frequency: u8,
    }

    pub async fn init(app: web::Data<AppState>) -> web::Json<InitResponse> {
        let ex = app.exchange.lock().unwrap();
        let init = ex.init();
        web::Json(InitResponse {
            start: init.start,
            frequency: init.frequency,
        })
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct InfoResponse {
        pub version: String,
        pub dataset: String,
    }

    pub async fn info(app: web::Data<AppState>) -> web::Json<InfoResponse> {
        let ex = app.exchange.lock().unwrap();
        let info = ex.info();
        web::Json(InfoResponse {
            version: info.version,
            dataset: info.dataset,
        })
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{test, web, App};

    use crate::exchange::uist_v1::{random_uist_generator, Order};

    use super::uistv1_server::*;
    use std::sync::Mutex;

    #[actix_web::test]
    async fn test_single_trade_loop() {
        let app_state = web::Data::new(AppState {
            exchange: Mutex::new(random_uist_generator(3000).0),
        });

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .route("/", web::get().to(info))
                .route("/init", web::get().to(init))
                .route("/fetch_quotes", web::get().to(fetch_quotes))
                .route("/tick", web::get().to(tick))
                .route("/insert_order", web::post().to(insert_order))
                .route("/delete_order", web::post().to(delete_order)),
        )
        .await;

        let req = test::TestRequest::get().uri("/init").to_request();
        let resp: InitResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.frequency == 0);

        let req1 = test::TestRequest::get().uri("/fetch_quotes").to_request();
        let _resp1: FetchQuotesResponse = test::call_and_read_body_json(&app, req1).await;

        let req2 = test::TestRequest::get().uri("/tick").to_request();
        let _resp2: TickResponse = test::call_and_read_body_json(&app, req2).await;

        let req3 = test::TestRequest::post()
            .set_json(InsertOrderRequest {
                order: Order::market_buy("ABC", 100.0),
            })
            .uri("/insert_order")
            .to_request();
        test::call_and_read_body(&app, req3).await;

        let req4 = test::TestRequest::get().uri("/tick").to_request();
        let resp4: TickResponse = test::call_and_read_body_json(&app, req4).await;

        assert!(resp4.executed_trades.len() == 1);
        assert!(resp4.executed_trades.get(0).unwrap().symbol == "ABC")
    }
}
