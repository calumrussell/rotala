use std::sync::Mutex;

use actix_web::web;
use serde::{Deserialize, Serialize};

use crate::{
    exchange::uist::{InitMessage, Uist, UistOrder, UistOrderId, UistTrade},
    input::penelope::PenelopeQuote,
};

pub async fn check(exchange: web::Data<Mutex<Uist>>) -> web::Json<Vec<UistTrade>> {
    let mut ex = exchange.lock().unwrap();
    web::Json(ex.check())
}

#[derive(Deserialize, Serialize)]
pub struct DeleteOrderRequest {
    order_id: UistOrderId,
}

pub async fn delete_order(
    exchange: web::Data<Mutex<Uist>>,
    delete_order: web::Json<DeleteOrderRequest>,
) -> web::Json<()> {
    let mut ex = exchange.lock().unwrap();
    ex.delete_order(delete_order.order_id.clone());
    web::Json(())
}

#[derive(Deserialize, Serialize)]
pub struct InsertOrderRequest {
    order: UistOrder,
}

pub async fn insert_order(
    exchange: web::Data<Mutex<Uist>>,
    insert_order: web::Json<InsertOrderRequest>,
) -> web::Json<()> {
    let mut ex = exchange.lock().unwrap();
    ex.insert_order(insert_order.order.clone());
    web::Json(())
}

#[derive(Deserialize, Serialize)]
pub struct FetchTradeRequest {
    from: usize,
}

pub async fn fetch_trades(
    exchange: web::Data<Mutex<Uist>>,
    fetch_trade: web::Json<FetchTradeRequest>,
) -> web::Json<Vec<UistTrade>> {
    let ex = exchange.lock().unwrap();
    web::Json(ex.fetch_trades(fetch_trade.from))
}

pub async fn fetch_quotes(exchange: web::Data<Mutex<Uist>>) -> web::Json<Vec<PenelopeQuote>> {
    let ex = exchange.lock().unwrap();
    web::Json(ex.fetch_quotes())
}

pub async fn init(exchange: web::Data<Mutex<Uist>>) -> web::Json<InitMessage> {
    let ex = exchange.lock().unwrap();
    web::Json(ex.init())
}

#[cfg(test)]
mod tests {
    use actix_web::{test, App};

    use crate::exchange::uist::random_uist_generator;

    use super::*;

    #[actix_web::test]
    async fn test_index_get() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(Mutex::new(random_uist_generator(3000))))
                .route("/", web::get().to(init))
                .route("/fetch_quotes", web::get().to(fetch_quotes))
                .route("/fetch_trades", web::get().to(fetch_trades))
                .route("/check", web::get().to(check))
                .route("/insert_order", web::post().to(insert_order))
                .route("/delete_order", web::post().to(delete_order)),
        )
        .await;

        let req = test::TestRequest::get().uri("/").to_request();
        let resp: InitMessage = test::call_and_read_body_json(&app, req).await;
        assert!(resp.frequency == 0);

        let req1 = test::TestRequest::get().uri("/fetch_quotes").to_request();
        let _resp1: Vec<PenelopeQuote> = test::call_and_read_body_json(&app, req1).await;

        let req2 = test::TestRequest::get().uri("/check").to_request();
        test::call_service(&app, req2).await;

        let order = UistOrder::market_buy("ABC", 100.0);
        let insert_req = InsertOrderRequest { order };

        let req3 = test::TestRequest::post()
            .set_json(insert_req)
            .uri("/insert_order")
            .to_request();
        test::call_service(&app, req3).await;

        let req4 = test::TestRequest::get().uri("/check").to_request();
        let resp4: Vec<UistTrade> = test::call_and_read_body_json(&app, req4).await;

        assert!(resp4.len() == 1);
        assert!(resp4.get(0).unwrap().symbol == "ABC")
    }
}
