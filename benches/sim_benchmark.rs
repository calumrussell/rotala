use alator::strategy::StaticWeightStrategyBuilder;
use alator::simcontext::SimContextBuilder;
use alator::sim::SimulatedBrokerBuilder;
use alator::broker::{BrokerCost, Quote};
use alator::types::{CashValue, DateTime, Frequency, PortfolioAllocation};
use alator::exchange::DefaultExchangeBuilder;
use alator::clock::ClockBuilder;
use alator::input::HashMapInputBuilder;

use rand::thread_rng;
use rand::distributions::Uniform;
use rand_distr::Distribution;
use criterion::{criterion_group, criterion_main, Criterion};

use std::rc::Rc;
use std::collections::HashMap;

pub fn run_sim() {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();
    let length_in_days: i64 = 100;
    let start_date: i64 = 1609750800; //Date - 4/1/21 9:00:0000
    let clock = ClockBuilder::with_length_in_days(start_date, length_in_days)
        .with_frequency(&Frequency::Daily)
        .build();

    let initial_cash: CashValue = 100_000.0.into();
    let mut raw_data: HashMap<DateTime, Vec<Quote>> = HashMap::new();
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
        raw_data.insert(DateTime::from(date), vec![q1, q2]);
    }

    let data = HashMapInputBuilder::new()
        .with_quotes(raw_data)
        .with_clock(Rc::clone(&clock))
        .build();

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
}

fn benchmark_std_backtest(c: &mut Criterion) {
    c.bench_function("full backtest", |b| b.iter(|| run_sim()));
}

criterion_group!(benches, benchmark_std_backtest);
criterion_main!(benches);