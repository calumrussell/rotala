use std::env;
use std::sync::Mutex;

use actix_web::{web, App, HttpServer};
use rotala::exchange::uist::random_uist_generator;
use rotala::server::uist::{delete_order, fetch_quotes, info, init, insert_order, tick, AppState};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let address: String = args[1].clone();
    let port: u16 = args[2].parse().unwrap();

    let app_state = web::Data::new(
        AppState {
            exchange: Mutex::new(random_uist_generator(3000).0)
        }
    );

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/", web::get().to(info))
            .route("/init", web::get().to(init))
            .route("/fetch_quotes", web::get().to(fetch_quotes))
            .route("/tick", web::get().to(tick))
            .route("/insert_order", web::post().to(insert_order))
            .route("/delete_order", web::post().to(delete_order))
    })
    .bind((address, port))?
    .run()
    .await
}
