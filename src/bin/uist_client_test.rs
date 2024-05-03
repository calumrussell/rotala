use std::io::Result;

use rotala::exchange::uist_v1::Order;
use rotala::http::uist::uistv1_client::Client;

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::new("http://127.0.0.1:8080".to_string());
    let resp = client.init("RANDOM".to_string()).await.unwrap();
    let backtest_id = resp.backtest_id;

    if let Ok(check) = client.tick(backtest_id).await {
        if check.has_next {
            let _ = client
                .insert_order(Order::market_buy("ABC", 100.0), backtest_id)
                .await;
        }
    }
    Ok(())
}
