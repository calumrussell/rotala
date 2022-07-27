use alator::clock::{Clock, ClockBuilder};
use alator::input::HashMapInputBuilder;
use alator::strategy::StaticWeightStrategyBuilder;
use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use std::collections::HashMap;
use std::rc::Rc;

use alator::broker::{BrokerCost, Quote};
use alator::input::HashMapInput;
use alator::sim::broker::SimulatedBrokerBuilder;
use alator::simcontext::SimContextBuilder;
use alator::types::{CashValue, DateTime, PortfolioAllocation, PortfolioWeight};

fn build_data(clock: Clock) -> HashMapInput {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    let mut raw_data: HashMap<DateTime, Vec<Quote>> = HashMap::new();
    for date in clock.borrow().peek() {
        let q1 = Quote {
            bid: price_dist.sample(&mut rng).into(),
            ask: price_dist.sample(&mut rng).into(),
            date: i64::from(date).into(),
            symbol: "ABC".to_string(),
        };
        let q2 = Quote {
            bid: price_dist.sample(&mut rng).into(),
            ask: price_dist.sample(&mut rng).into(),
            date: i64::from(date).into(),
            symbol: "BCD".to_string(),
        };
        raw_data.insert(i64::from(date).into(), vec![q1, q2]);
    }

    let source = HashMapInputBuilder::new()
        .with_quotes(raw_data)
        .with_clock(Rc::clone(&clock))
        .build();
    source
}

#[test]
fn staticweight_integration_test() {
    env_logger::init();
    let initial_cash: CashValue = 100_000.0.into();
    let length_in_days: i64 = 200;
    let start_date: i64 = 1609750800; //Date - 4/1/21 9:00:0000
    let clock = ClockBuilder::from_length(&start_date.into(), length_in_days).daily();

    let data = build_data(Rc::clone(&clock));

    let mut weights: PortfolioAllocation<PortfolioWeight> = PortfolioAllocation::new();
    weights.insert(&String::from("ABC"), &0.5.into());
    weights.insert(&String::from("BCD"), &0.5.into());

    let simbrkr = SimulatedBrokerBuilder::new()
        .with_data(data)
        .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
        .build();

    let strat = StaticWeightStrategyBuilder::new()
        .with_brkr(simbrkr)
        .with_weights(weights)
        .with_clock(Rc::clone(&clock))
        .daily();

    let mut sim = SimContextBuilder::new()
        .with_clock(Rc::clone(&clock))
        .with_strategy(strat)
        .init(&initial_cash);

    sim.run();
}
