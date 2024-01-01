use std::io::Result;

use rotala::{client::uist::UistClient, exchange::uist::UistOrder};

#[tokio::main]
async fn main() -> Result<()> {
    let client = UistClient::new("http://127.0.0.1:8080".to_string());
    let _ = client.init().await.unwrap();
    if let Ok(check) = client.check().await {
        if check.has_next {
            let _ = client
                .insert_order(UistOrder::market_buy("ABC", 100.0))
                .await;
        }
    }
    Ok(())
}
