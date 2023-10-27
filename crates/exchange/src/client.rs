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

    let subscriber_id_request_1 = client.register_source(Request::new(RegisterSourceRequest {})).await?;
    let subscriber_id_1 = subscriber_id_request_1.into_inner().subscriber_id;

    let subscriber_id_request_2 = client.register_source(Request::new(RegisterSourceRequest {})).await?;
    let subscriber_id_2 = subscriber_id_request_2.into_inner().subscriber_id;

    loop {
        let start = Instant::now();

        let order = Order { r#type: 1, symbol: "ABC".to_string(), price: None, quantity: 1.0 };
        let order_request = client.send_order(Request::new(SendOrderRequest { subscriber_id: subscriber_id_1, order: Some(order) })).await?;
        let order_id = order_request.into_inner().order_id;

        let order_1 = Order { r#type: 1, symbol: "ABC".to_string(), price: None, quantity: 10.0 };
        let order_request = client.send_order(Request::new(SendOrderRequest { subscriber_id: subscriber_id_2, order: Some(order_1) })).await?;
        let order_id = order_request.into_inner().order_id;

        let quotes_1 = client.fetch_quotes(Request::new(exchange::FetchQuotesRequest { })).await?;

        let trades_1 = client.fetch_trades(Request::new(exchange::FetchTradesRequest { })).await?;
        println!("Executed: {:?}", trades_1.into_inner().trades);

        client.tick(Request::new(exchange::TickRequest { subscriber_id: subscriber_id_1 })).await?;
        client.tick(Request::new(exchange::TickRequest { subscriber_id: subscriber_id_2 })).await?;

        let duration = start.elapsed();
        println!("Order id: {:?} in {:?}", order_id, duration);
    }
}