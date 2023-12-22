use std::sync::Mutex;

use actix_web::{App, web, HttpServer};
use rotala::{exchange::uist::{random_uist_generator, Uist, InitMessage, UistOrder, UistOrderId, UistTrade}, input::penelope::PenelopeQuote};
use serde::Deserialize;

async fn check(exchange: web::Data<Mutex<Uist>>) -> web::Json<Vec<UistTrade>> {
    let mut ex = exchange.lock().unwrap();
    web::Json(ex.check())
}

#[derive(Deserialize)]
pub struct DeleteOrderRequest {
    order_id: UistOrderId,
}

async fn delete_order(exchange: web::Data<Mutex<Uist>>, delete_order: web::Json<DeleteOrderRequest>) -> web::Json<()> {
    let mut ex = exchange.lock().unwrap();
    ex.delete_order(delete_order.order_id.clone());
    web::Json({})
}

#[derive(Deserialize)]
pub struct InsertOrderRequest {
    order: UistOrder,
}

async fn insert_order(exchange: web::Data<Mutex<Uist>>, insert_order: web::Json<InsertOrderRequest>) -> web::Json<()> {
    let mut ex = exchange.lock().unwrap();
    ex.insert_order(insert_order.order.clone());
    web::Json({})
}

#[derive(Deserialize)]
pub struct FetchTradeRequest {
    from: usize,
}

async fn fetch_trade(exchange: web::Data<Mutex<Uist>>, fetch_trade: web::Json<FetchTradeRequest>) -> web::Json<Vec<UistTrade>> {
    let ex = exchange.lock().unwrap();
    web::Json(ex.fetch_trades(fetch_trade.from))

}

async fn fetch_quotes(exchange: web::Data<Mutex<Uist>>) -> web::Json<Vec<PenelopeQuote>> {
    let ex = exchange.lock().unwrap();
    web::Json(ex.fetch_quotes())
}

async fn init(exchange: web::Data<Mutex<Uist>>) -> web::Json<InitMessage> {
    let ex = exchange.lock().unwrap();
    web::Json(ex.init())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .app_data(web::Data::new(random_uist_generator(3000)))
            .route("/", web::get().to(init))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
