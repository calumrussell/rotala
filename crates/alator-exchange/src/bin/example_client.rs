use alator_exchange::{ExchangeAsync, RPCExchange};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rpc_exchange = RPCExchange::build_exchange_client("http://[::]:50051").await?;

    let subscriber_id = rpc_exchange.register_source().await.unwrap();
    let order = alator_exchange::ExchangeOrder {
        subscriber_id,
        order_type: alator_exchange::OrderType::MarketBuy,
        price: None,
        shares: 100.0,
        symbol: "ABC".to_string(),
    };

    rpc_exchange.send_order(subscriber_id, order).await?;
    rpc_exchange.tick(subscriber_id).await?;

    Ok(())
}

