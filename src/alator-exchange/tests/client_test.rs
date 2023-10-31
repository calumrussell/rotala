use alator_exchange::orderbook::DefaultPriceSource;
use alator_exchange::DefaultClient;
use tonic::codegen::tokio_stream;
use tonic::transport::{Endpoint, Server, Uri};
use tower::service_fn;

use alator_exchange::proto::{exchange_client::ExchangeClient, exchange_server::ExchangeServer};

pub mod proto {
    tonic::include_proto!("exchange");
}

#[tokio::test]
async fn test_system() -> Result<(), Box<dyn std::error::Error>> {
    let (client, server) = tokio::io::duplex(1024);

    let clock = alator::clock::ClockBuilder::with_length_in_seconds(100, 100)
        .with_frequency(&alator::types::Frequency::Second)
        .build();

    let mut source = DefaultPriceSource::new();
    for date in clock.peek() {
        source.add_quotes(100.0, 101.0, *date, "ABC".to_string());
    }

    let exchange = alator_exchange::DefaultExchange::new(clock.clone(), source);

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

    let broker_1 = DefaultClient::init(&mut client).await?;
    let broker_2 = DefaultClient::init(&mut client).await?;

    for _date in clock.peek() {
        let _oid0 = broker_1
            .send_order(
                &mut client,
                alator_exchange::orderbook::OrderType::MarketBuy,
                None,
                100.0,
                "ABC",
            )
            .await?;
        let _oid1 = broker_2
            .send_order(
                &mut client,
                alator_exchange::orderbook::OrderType::MarketBuy,
                None,
                100.0,
                "ABC",
            )
            .await?;

        broker_1.tick(&mut client).await?;
        if let Err(_err) = broker_2.tick(&mut client).await {
            break;
        }
    }
    Ok(())
}
