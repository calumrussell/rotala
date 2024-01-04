use alator::broker::uist::UistBrokerBuilder;
use alator::broker::BrokerCost;
use alator::strategy::Strategy;
use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use rotala::clock::Frequency;

use alator::strategy::staticweight::StaticWeightStrategyBuilder;
use alator::types::{CashValue, PortfolioAllocation};
use rotala::exchange::uist::UistV1;
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

#[test]
fn staticweight_integration_test() {
    env_logger::init();
    let initial_cash: CashValue = 100_000.0.into();
    let length_in_days: i64 = 1000;

    let price_source_builder = build_data(length_in_days);
    let (source, clock) =
        price_source_builder.build_with_frequency(rotala::clock::Frequency::Second);

    let mut weights: PortfolioAllocation = PortfolioAllocation::new();
    weights.insert("ABC", 0.5);
    weights.insert("BCD", 0.5);

    let exchange = UistV1::new(clock.clone(), source, "RANDOM");

    let brkr = UistBrokerBuilder::new()
        .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
        .with_exchange(exchange)
        .build();

    let mut strat = StaticWeightStrategyBuilder::new()
        .with_brkr(brkr)
        .with_weights(weights)
        .with_clock(clock.clone())
        .default();

    strat.init(&initial_cash);
    strat.run();

    let _perf = strat.perf(Frequency::Daily);
}
