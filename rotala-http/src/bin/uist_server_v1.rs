use std::env;
use std::sync::Mutex;

use actix_web::{web, App, HttpServer};

use rotala::input::penelope::Penelope;
use rotala_http::http::uist_v1::{
    server::{delete_order, fetch_quotes, info, init, insert_order, tick},
    AppState,
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let args: Vec<String> = env::args().collect();

    let address: String = args[1].clone();
    let port: u16 = args[2].parse().unwrap();

    let source = Penelope::random(3000, vec!["ABC", "BCD"]);
    let app_state = AppState::single("RANDOM", source);

    let uist_state = web::Data::new(Mutex::new(app_state));

    HttpServer::new(move || {
        App::new()
            .app_data(uist_state.clone())
            .service(info)
            .service(init)
            .service(fetch_quotes)
            .service(tick)
            .service(insert_order)
            .service(delete_order)
    })
    .bind((address, port))?
    .run()
    .await
}
