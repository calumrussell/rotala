use std::collections::HashMap;
use std::env;
use std::sync::Mutex;

use actix_web::{web, App, HttpServer};
use rotala::exchange::uist_v1::random_uist_generator;
use rotala::http::uist::uistv1_server::{
    delete_order, fetch_quotes, info, init, insert_order, tick, AppState,
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let address: String = args[1].clone();
    let port: u16 = args[2].parse().unwrap();

    let uist = random_uist_generator(3000);
    let mut datasets = HashMap::new();
    datasets.insert("RANDOM".to_string(), uist.0);

    let app_state = Mutex::new(AppState::create(&mut datasets));
    let uist_state = web::Data::new(app_state);

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
