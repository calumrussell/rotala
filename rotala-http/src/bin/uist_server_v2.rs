use std::env;
use std::sync::Mutex;

use actix_web::{web, App, HttpServer};
use rotala::input::athena::Athena;
use rotala_http::http::uist_v2::server::*;
use rotala_http::http::uist_v2::AppState;
use std::path::Path;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let address: String = args[1].clone();
    let port: u16 = args[2].parse().unwrap();
    let file_path = Path::new(&args[3]);

    let source = Athena::from_file(file_path);
    let app_state = AppState::single("Test", source);

    let uist_state = web::Data::new(Mutex::new(app_state));

    HttpServer::new(move || {
        App::new()
            .app_data(uist_state.clone())
            .service(info)
            .service(init)
            .service(tick)
            .service(insert_order)
    })
    .bind((address, port))?
    .run()
    .await
}
