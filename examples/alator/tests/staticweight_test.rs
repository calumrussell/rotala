use std::collections::HashMap;

use alator::broker::uist::UistBrokerBuilder;
use alator::broker::BrokerCost;
use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use rotala::clock::Frequency;

use alator::strategy::staticweight::StaticWeightStrategyBuilder;
use alator::types::{CashValue, PortfolioAllocation};
use rotala::exchange::uist_v1::UistV1;
use rotala::http::uist::uistv1_client::{TestClient, UistClient};
use rotala::input::penelope::PenelopeBuilder;

fn build_data(length: i64) -> PenelopeBuilder {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    let mut source = PenelopeBuilder::new();
    for date in 1..length + 1 {
        source.add_quote(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "ABC",
        );
        source.add_quote(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "BCD",
        );
    }
    source
}

#[tokio::test]
async fn staticweight_integration_test() {
    env_logger::init();
    let initial_cash: CashValue = 100_000.0.into();
    let length_in_days: i64 = 1000;

    let mut price_source_builder = build_data(length_in_days);
    let (source, clock) =
        price_source_builder.build_with_frequency(rotala::clock::Frequency::Second);

    let mut weights: PortfolioAllocation = PortfolioAllocation::new();
    weights.insert("ABC", 0.5);
    weights.insert("BCD", 0.5);


    let exchange = UistV1::new(clock.clone(), source, "Random");
    let mut datasets = HashMap::new();
    datasets.insert("Random".to_string(), exchange);
    let mut client = TestClient::new(&mut datasets);
    let resp = client.init("Random".to_string()).await.unwrap();

    let brkr = UistBrokerBuilder::new()
        .with_client(client, resp.backtest_id)
        .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
        .build()
        .await;

    let mut strat = StaticWeightStrategyBuilder::new()
        .with_brkr(brkr)
        .with_weights(weights)
        .with_clock(clock.clone())
        .default();

    strat.init(&initial_cash);
    strat.run().await;

    let _perf = strat.perf(Frequency::Daily);
}
