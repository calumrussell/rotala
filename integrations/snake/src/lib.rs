use pyo3::prelude::*;

use alator::clock::{Clock, ClockBuilder};
use alator::exchange::DefaultExchangeBuilder;
use alator::input::HashMapInputBuilder;
use alator::strategy::StaticWeightStrategyBuilder;
use alator::broker::{BrokerCost, Quote};
use alator::input::HashMapInput;
use alator::sim::SimulatedBrokerBuilder;
use alator::simcontext::SimContextBuilder;
use alator::types::{CashValue, DateTime, Frequency, PortfolioAllocation};
use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use std::collections::HashMap;
use std::rc::Rc;

fn build_data(clock: Clock) -> HashMapInput {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    let mut raw_data: HashMap<DateTime, Vec<Quote>> = HashMap::with_capacity(clock.borrow().len());
    for date in clock.borrow().peek() {
        let q1 = Quote::new(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date.clone(),
            "ABC",
        );
        let q2 = Quote::new(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date.clone(),
            "BCD",
        );
        raw_data.insert(date, vec![q1, q2]);
    }

    let source = HashMapInputBuilder::new()
        .with_quotes(raw_data)
        .with_clock(Rc::clone(&clock))
        .build();
    source
}

#[pyfunction]
fn staticweight_example() -> PyResult<String> {
    let initial_cash: CashValue = 100_000.0.into();
    let length_in_days: i64 = 1000;
    let start_date: i64 = 1609750800; //Date - 4/1/21 9:00:0000
    let clock = ClockBuilder::with_length_in_days(start_date, length_in_days)
        .with_frequency(&Frequency::Daily)
        .build();

    let data = build_data(Rc::clone(&clock));

    let mut weights: PortfolioAllocation = PortfolioAllocation::new();
    weights.insert("ABC", 0.5);
    weights.insert("BCD", 0.5);

    let exchange = DefaultExchangeBuilder::new()
        .with_data_source(data.clone())
        .with_clock(Rc::clone(&clock))
        .build();

    let simbrkr = SimulatedBrokerBuilder::new()
        .with_data(data)
        .with_exchange(exchange)
        .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
        .build();

    let strat = StaticWeightStrategyBuilder::new()
        .with_brkr(simbrkr)
        .with_weights(weights)
        .with_clock(Rc::clone(&clock))
        .default();

    let mut sim = SimContextBuilder::new()
        .with_clock(Rc::clone(&clock))
        .with_strategy(strat)
        .init(&initial_cash);

    sim.run();

    let _perf = sim.perf(Frequency::Daily);

    Ok("Backtest completed".to_string())
}

#[pymodule]
fn snake(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(staticweight_example, m)?)?;
    Ok(())
}
