use tonic::Request;
use std::time::Instant;

use exchange::proto;

use proto::exchange_client::ExchangeClient;
use proto::{ Order, SendOrderRequest, RegisterSourceRequest };

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

        let quotes_1 = client.fetch_quotes(Request::new(proto::FetchQuotesRequest { })).await?;

        let trades_1 = client.fetch_trades(Request::new(proto::FetchTradesRequest { })).await?;
        println!("Executed: {:?}", trades_1.into_inner().trades);

        client.tick(Request::new(proto::TickRequest { subscriber_id: subscriber_id_1 })).await?;
        client.tick(Request::new(proto::TickRequest { subscriber_id: subscriber_id_2 })).await?;

        let duration = start.elapsed();
        println!("Order id: {:?} in {:?}", order_id, duration);
    }
}

#[cfg(test)]
mod tests {
    use exchange::orderbook::DefaultPriceSource;
    use tonic::{
        transport::{Endpoint, Server, Uri},
        Request,
    };
    use tonic::codegen::tokio_stream;
    use tower::service_fn;

    use super::proto::{exchange_client::ExchangeClient, RegisterSourceRequest, exchange_server::ExchangeServer};

    pub mod proto {
        tonic::include_proto!("exchange");
    }

    #[tokio::test]
    async fn test_system() -> Result<(), Box<dyn std::error::Error>> {
        let (client, server) = tokio::io::duplex(1024);

        let clock = alator::clock::ClockBuilder::with_length_in_seconds(100, 1000)
            .with_frequency(&alator::types::Frequency::Second)
            .build();

        let mut source = DefaultPriceSource::new();
        for date in clock.peek() {
            source.add_quotes(100.0, 101.0, *date, "ABC".to_string());
        }

        let exchange = exchange::DefaultExchange::new(clock, source);

        tokio::spawn(async move {
            Server::builder()
                .add_service(ExchangeServer::new(exchange))
                .serve_with_incoming(tokio_stream::iter(vec![Ok::<_, std::io::Error>(server)]))
                .await
        });

        // Move client to an option so we can _move_ the inner value
        // on the first attempt to connect. All other attempts will fail.
        let mut client = Some(client);
        let channel = Endpoint::try_from("http://[::]:50051")?
            .connect_with_connector(service_fn(move |_: Uri| {
                let client = client.take();

                async move {
                    if let Some(client) = client {
                        Ok(client)
                    } else {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "Client already taken",
                        ))
                    }
                }
            }))
            .await?;

        let mut client = ExchangeClient::new(channel);

        let subscriber_id_request_1 = client.register_source(Request::new(RegisterSourceRequest {})).await?;
        let subscriber_id_1 = subscriber_id_request_1.into_inner().subscriber_id;

        let subscriber_id_request_2 = client.register_source(Request::new(RegisterSourceRequest {})).await?;
        let subscriber_id_2 = subscriber_id_request_2.into_inner().subscriber_id;

        println!("RESPONSE={:?}", subscriber_id_1);
        println!("RESPONSE={:?}", subscriber_id_2);

        assert!(true==false);

        Ok(())

    }
}