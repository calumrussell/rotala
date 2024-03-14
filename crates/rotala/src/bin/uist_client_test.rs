use std::io::Result;

use rotala::exchange::uist::UistOrder;
use rotala::http::uist::UistV1Client;

#[tokio::main]
async fn main() -> Result<()> {
    let client = UistV1Client::new("http://127.0.0.1:8080".to_string());
    let _ = client.init().await.unwrap();
    if let Ok(check) = client.tick().await {
        if check.has_next {
            let _ = client
                .insert_order(UistOrder::market_buy("ABC", 100.0))
                .await;
        }
    }
    Ok(())
}
