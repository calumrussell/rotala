use std::collections::HashMap;

use alator::broker::uist::UistBrokerBuilder;
use alator::broker::BrokerCost;

use alator::strategy::staticweight::{PortfolioAllocation, StaticWeightStrategyBuilder};
use rotala::http::uist_v1::{TestClient, Client};
use rotala::input::penelope::Penelope;

#[tokio::test]
async fn staticweight_integration_test() {
    println!("{:?}", "Test");
    env_logger::init();
    let initial_cash = 100_000.0;
    let length_in_days: i64 = 1000;

    let mut weights: PortfolioAllocation = HashMap::new();
    weights.insert("ABC".to_string(), 0.5);
    weights.insert("BCD".to_string(), 0.5);

    let source = Penelope::random(length_in_days, vec!["ABC", "BCD"]);
    let mut client = TestClient::single("Random", source);
    let resp = client.init("Random".to_string()).await.unwrap();

    let brkr = UistBrokerBuilder::new()
        .with_client(client, resp.backtest_id)
        .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
        .build()
        .await;

    let mut strat = StaticWeightStrategyBuilder::new()
        .with_brkr(brkr)
        .with_weights(weights)
        .default();

    strat.init(&initial_cash);
    strat.run().await;

    let _perf = strat.perf(alator::perf::Frequency::Daily);
}
