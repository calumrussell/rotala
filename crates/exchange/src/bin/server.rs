use tonic::transport::Server;

use exchange::proto;
use proto::exchange_server::ExchangeServer;
use exchange::orderbook::DefaultPriceSource;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();

    let clock = alator::clock::ClockBuilder::with_length_in_seconds(100, 1000)
        .with_frequency(&alator::types::Frequency::Second)
        .build();

    let mut source = DefaultPriceSource::new();
    for date in clock.peek() {
       source.add_quotes(100.0, 101.0, *date, "ABC".to_string());
    }

    let exchange = exchange::DefaultExchange::new(clock, source);

    println!("DefaultExchange listening on {}", addr);

    Server::builder()
        .add_service(ExchangeServer::new(exchange))
        .serve(addr)
        .await?;

    Ok(())
}