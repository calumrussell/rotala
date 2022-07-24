use alator::strategy::StaticWeightStrategyBuilder;
use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use std::collections::HashMap;

use alator::broker::{BrokerCost, Dividend, Quote};
use alator::data::{CashValue, DataSource, DateTime, PortfolioAllocation, PortfolioWeight};
use alator::sim::broker::SimulatedBrokerBuilder;
use alator::simcontext::SimContext;

fn build_data() -> (DataSource, Vec<DateTime>) {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    let length_in_days = 200;
    let seconds_in_day = 86_400;
    let start_date = 1609750800; //Date - 4/1/21 9:00:0000
    let end_date = start_date + (seconds_in_day * length_in_days);
    let mut raw_data: HashMap<DateTime, Vec<Quote>> = HashMap::new();
    for date in (start_date..end_date).step_by(seconds_in_day) {
        let q1 = Quote {
            bid: price_dist.sample(&mut rng).into(),
            ask: price_dist.sample(&mut rng).into(),
            date: (date as i64).into(),
            symbol: "ABC".to_string(),
        };
        let q2 = Quote {
            bid: price_dist.sample(&mut rng).into(),
            ask: price_dist.sample(&mut rng).into(),
            date: (date as i64).into(),
            symbol: "BCD".to_string(),
        };
        raw_data.insert((date as i64).into(), vec![q1, q2]);
    }
    let dividends: HashMap<DateTime, Vec<Dividend>> = HashMap::new();
    let dates = raw_data.keys().map(|d| d.clone()).collect();
    let source = DataSource::from_hashmap(raw_data, dividends);
    (source, dates)
}

#[test]
fn staticweight_integration_test() {
    let initial_cash: CashValue = 100_000.0.into();
    let data = build_data();

    let mut weights: PortfolioAllocation<PortfolioWeight> = PortfolioAllocation::new();
    weights.insert(&String::from("ABC"), &0.5.into());
    weights.insert(&String::from("BCD"), &0.5.into());

    let simbrkr = SimulatedBrokerBuilder::new()
        .with_data(data.0)
        .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
        .build();

    let strat = StaticWeightStrategyBuilder::new()
        .with_brkr(simbrkr)
        .with_weights(weights)
        .daily();

    let mut sim = SimContext::new(data.1, initial_cash, &strat);
    sim.run();
}
