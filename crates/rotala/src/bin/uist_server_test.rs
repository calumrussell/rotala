use std::sync::Mutex;

use actix_web::{web, App, HttpServer};
use rotala::api::uist::{check, delete_order, fetch_quotes, fetch_trades, init, insert_order};
use rotala::exchange::uist::random_uist_generator;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .app_data(web::Data::new(Mutex::new(random_uist_generator(3000).0)))
            .route("/init", web::get().to(init))
            .route("/fetch_quotes", web::get().to(fetch_quotes))
            .route("/fetch_trades", web::post().to(fetch_trades))
            .route("/check", web::get().to(check))
            .route("/insert_order", web::post().to(insert_order))
            .route("/delete_order", web::post().to(delete_order))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
