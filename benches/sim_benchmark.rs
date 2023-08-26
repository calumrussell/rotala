use alator::broker::{BacktestBroker, BrokerCost, Order, OrderType, Quote, TransferCash};
use alator::clock::ClockBuilder;
use alator::exchange::builder::DefaultExchangeBuilder;
use alator::input::HashMapInputBuilder;
use alator::sim::SimulatedBrokerBuilder;
use alator::simcontext::SimContextBuilder;
use alator::strategy::StaticWeightStrategyBuilder;
use alator::types::{CashValue, DateTime, Frequency, PortfolioAllocation};

use criterion::{criterion_group, criterion_main, Criterion};
use rand::distributions::Uniform;
use rand::thread_rng;
use rand_distr::Distribution;

use std::collections::HashMap;
use std::sync::Arc;

async fn full_backtest_random_data() {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();
    let length_in_days: i64 = 100;
    let start_date: i64 = 1609750800; //Date - 4/1/21 9:00:0000
    let clock = ClockBuilder::with_length_in_days(start_date, length_in_days)
        .with_frequency(&Frequency::Daily)
        .build();

    let initial_cash: CashValue = 100_000.0.into();

    let mut raw_data: HashMap<DateTime, Vec<Arc<Quote>>> = HashMap::with_capacity(clock.len());
    for date in clock.peek() {
        let q1 = Quote::new(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "ABC",
        );
        let q2 = Quote::new(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "BCD",
        );
        raw_data.insert(date, vec![Arc::new(q1), Arc::new(q2)]);
    }

    let data = HashMapInputBuilder::new()
        .with_quotes(raw_data)
        .with_clock(clock.clone())
        .build();

    let mut weights: PortfolioAllocation = PortfolioAllocation::new();
    weights.insert("ABC", 0.5);
    weights.insert("BCD", 0.5);

    let mut exchange = DefaultExchangeBuilder::new()
        .with_data_source(data.clone())
        .with_clock(clock.clone())
        .build();

    let simbrkr = SimulatedBrokerBuilder::new()
        .with_data(data)
        .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
        .build(&mut exchange);

    let strat = StaticWeightStrategyBuilder::new()
        .with_brkr(simbrkr)
        .with_weights(weights)
        .with_clock(clock.clone())
        .default();

    let mut sim = SimContextBuilder::new()
        .with_clock(clock.clone())
        .add_strategy(strat)
        .init_first(&initial_cash);

    sim.run();
}

fn trade_execution_logic() {
    let mut prices: HashMap<DateTime, Vec<Arc<Quote>>> = HashMap::new();
    let quote = Arc::new(Quote::new(100.00, 101.00, 100, "ABC"));
    let quote1 = Arc::new(Quote::new(10.00, 11.00, 100, "BCD"));

    let quote2 = Arc::new(Quote::new(100.00, 101.00, 101, "ABC"));
    let quote3 = Arc::new(Quote::new(10.00, 11.00, 101, "BCD"));

    let quote4 = Arc::new(Quote::new(104.00, 105.00, 102, "ABC"));
    let quote5 = Arc::new(Quote::new(10.00, 11.00, 102, "BCD"));

    let quote6 = Arc::new(Quote::new(104.00, 105.00, 103, "ABC"));
    let quote7 = Arc::new(Quote::new(12.00, 13.00, 103, "BCD"));

    prices.insert(100.into(), vec![quote, quote1]);
    prices.insert(101.into(), vec![quote2, quote3]);
    prices.insert(102.into(), vec![quote4, quote5]);
    prices.insert(103.into(), vec![quote6, quote7]);

    let clock = ClockBuilder::with_length_in_seconds(100, 5)
        .with_frequency(&Frequency::Second)
        .build();

    let source = HashMapInputBuilder::new()
        .with_quotes(prices)
        .with_clock(clock.clone())
        .build();

    let mut exchange = DefaultExchangeBuilder::new()
        .with_clock(clock.clone())
        .with_data_source(source.clone())
        .build();

    let mut brkr = SimulatedBrokerBuilder::new()
        .with_data(source)
        .build(&mut exchange);

    brkr.deposit_cash(&100_000.0);
    brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 100.0));
    brkr.send_order(Order::market(OrderType::MarketBuy, "BCD", 100.0));

    exchange.check();
    brkr.check();

    exchange.check();
    brkr.check();

    exchange.check();
    brkr.check();
}

fn benchmarks(c: &mut Criterion) {
    c.bench_function("full backtest", |b| b.iter(full_backtest_random_data));
    c.bench_function("trade test", |b| b.iter(trade_execution_logic));
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
