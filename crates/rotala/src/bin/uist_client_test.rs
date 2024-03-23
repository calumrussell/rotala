use std::io::Result;

use rotala::exchange::uist_v1::Order;
use rotala::http::uist::uistv1_client::Client;

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::new("http://127.0.0.1:8080".to_string());
    let _ = client.init().await.unwrap();
    if let Ok(check) = client.tick().await {
        if check.has_next {
            let _ = client.insert_order(Order::market_buy("ABC", 100.0)).await;
        }
    }
    Ok(())
}
