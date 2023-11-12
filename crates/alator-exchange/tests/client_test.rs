use alator_exchange::input::DefaultPriceSource;
use alator_exchange::{ExchangeAsync, ExchangeClient, RPCExchange};
use tonic::codegen::tokio_stream;
use tonic::transport::{Endpoint, Server, Uri};
use tower::service_fn;

#[tokio::test]
async fn test_system() -> Result<(), Box<dyn std::error::Error>> {
    let (client, server) = tokio::io::duplex(1024);

    let clock = alator::clock::ClockBuilder::with_length_in_seconds(100, 100)
        .with_frequency(&alator::types::Frequency::Second)
        .build();

    let mut copy_clock = clock.clone();

    let mut source = DefaultPriceSource::new();
    for date in clock.peek() {
        source.add_quotes(100.0, 101.0, *date, "ABC".to_string());
    }

    let exchange_server = RPCExchange::build_exchange_server(clock, source);

    tokio::spawn(async move {
        Server::builder()
            .add_service(exchange_server)
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

    let client = ExchangeClient::new(channel);
    let mut rpc_exchange = RPCExchange::new(client);

    let subscriber_id = rpc_exchange.register_source().await.unwrap();

    while copy_clock.has_next() {
        let order = alator_exchange::ExchangeOrder {
            subscriber_id,
            order_type: alator_exchange::OrderType::MarketBuy,
            price: None,
            shares: 100.0,
            symbol: "ABC".to_string(),
        };
        rpc_exchange.send_order(subscriber_id, order).await?;
        rpc_exchange.tick(subscriber_id).await?;
        //This looks synchronized but isn't actually, real clients would use the exchange to
        //co-ordinate their own tick
        copy_clock.tick();
    }
    Ok(())
}
