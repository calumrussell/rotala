use std::collections::HashMap;
use std::env;
use std::sync::Mutex;

use actix_web::{web, App, HttpServer};
use rotala::exchange::jura_v1::random_jura_generator;
use rotala::http::jura::jurav1_server::{
    delete_order, fetch_quotes, info, init, insert_order, tick, AppState,
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let address: String = args[1].clone();
    let port: u16 = args[2].parse().unwrap();

    let jura = random_jura_generator(3000);
    let mut datasets = HashMap::new();
    datasets.insert("RANDOM".to_string(), jura.0);

    let app_state = Mutex::new(AppState::create(&mut datasets));
    let jura_state = web::Data::new(app_state);

    HttpServer::new(move || {
        App::new()
            .app_data(jura_state.clone())
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
