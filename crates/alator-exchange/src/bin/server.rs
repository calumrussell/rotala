use tonic::transport::Server;

use alator_exchange::{input::DefaultPriceSource, RPCExchange};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();

    let clock = alator_clock::ClockBuilder::with_length_in_seconds(100, 1000)
        .with_frequency(&alator_clock::Frequency::Second)
        .build();

    let mut source = DefaultPriceSource::new();
    for date in clock.peek() {
        source.add_quotes(100.0, 101.0, *date, "ABC".to_string());
    }

    let exchange_server = RPCExchange::build_exchange_server(clock, source);

    println!("DefaultExchange listening on {}", addr);

    Server::builder()
        .add_service(exchange_server)
        .serve(addr)
        .await?;

    Ok(())
}
