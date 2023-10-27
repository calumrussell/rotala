use exchange::exchange_client::ExchangeClient;
use tonic::Request;
use std::time::Instant;
use crate::exchange::{ Order, SendOrderRequest, RegisterSourceRequest };

pub mod exchange {
    tonic::include_proto!("exchange");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ExchangeClient::connect("http://[::1]:50051").await?;

    let subscriber_id_request = client.register_source(Request::new(RegisterSourceRequest {})).await?;
    let subscriber_id = subscriber_id_request.into_inner().source_id;

    loop {
        let start = Instant::now();

        let order = Order { r#type: 1, symbol: "ABC".to_string(), price: None, quantity: 1.0 };
        let order_request = client.send_order(Request::new(SendOrderRequest { source_id: subscriber_id, order: Some(order) })).await?;
        let order_id = order_request.into_inner().order_id;

        let duration = start.elapsed();
        println!("Order id: {:?} in {:?}", order_id, duration);
    }
}