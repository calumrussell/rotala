use std::env;

use actix_web::{web, App, HttpServer};
use rotala_http::http::uist_v2::server::*;
use rotala_http::http::uist_v2::AppState;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let args: Vec<String> = env::args().collect();

    let address: String = args[1].clone();
    let port: u16 = args[2].parse().unwrap();
    let host: String = args[3].clone();
    let user: String = args[4].clone();
    let password: String = args[5].clone();
    let dbname: String = args[6].clone();

    let app_state = AppState::single(&user, &dbname, &host, &password);
    let uist_state = web::Data::new(app_state);

    HttpServer::new(move || {
        App::new()
            .app_data(uist_state.clone())
            .service(info)
            .service(init)
            .service(tick)
            .service(insert_orders)
            .service(dataset_info)
    })
    .bind((address, port))?
    .run()
    .await
}
