use alator::broker::{
    BrokerCost, Dividend, Order, OrderType, Quote, ReceievesOrders, SingleBroker,
    SingleBrokerBuilder, TransferCash,
};
use alator::clock::ClockBuilder;
use alator::exchange::SingleExchangeBuilder;
use alator::input::{
    fake_price_source_generator, HashMapCorporateEventsSource, HashMapPriceSource,
};
use alator::simcontext::SimContextBuilder;
use alator::strategy::StaticWeightStrategyBuilder;
use alator::types::{CashValue, Frequency, PortfolioAllocation};

use criterion::{criterion_group, criterion_main, Criterion};

fn full_backtest_random_data() {
    let length_in_days: i64 = 100;
    let start_date: i64 = 1609750800; //Date - 4/1/21 9:00:0000
    let clock = ClockBuilder::with_length_in_days(start_date, length_in_days)
        .with_frequency(&Frequency::Daily)
        .build();

    let initial_cash: CashValue = 100_000.0.into();
    let price_source = fake_price_source_generator(clock.clone());

    let mut weights: PortfolioAllocation = PortfolioAllocation::new();
    weights.insert("ABC", 0.5);
    weights.insert("BCD", 0.5);

    let exchange = SingleExchangeBuilder::new()
        .with_price_source(price_source)
        .with_clock(clock.clone())
        .build();

    let simbrkr: SingleBroker<Dividend, HashMapCorporateEventsSource, Quote, HashMapPriceSource> =
        SingleBrokerBuilder::new()
            .with_exchange(exchange)
            .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
            .build();

    let strat = StaticWeightStrategyBuilder::new()
        .with_brkr(simbrkr)
        .with_weights(weights)
        .with_clock(clock.clone())
        .default();

    let mut sim = SimContextBuilder::new()
        .with_clock(clock.clone())
        .with_strategy(strat)
        .init(&initial_cash);

    sim.run();
}

fn trade_execution_logic() {
    let clock = ClockBuilder::with_length_in_seconds(100, 5)
        .with_frequency(&Frequency::Second)
        .build();

    let mut price_source = HashMapPriceSource::new(clock.clone());
    price_source.add_quotes(100, Quote::new(100.00, 101.00, 100, "ABC"));
    price_source.add_quotes(100, Quote::new(10.00, 11.00, 100, "BCD"));
    price_source.add_quotes(101, Quote::new(100.00, 101.00, 101, "ABC"));
    price_source.add_quotes(101, Quote::new(10.00, 11.00, 101, "BCD"));
    price_source.add_quotes(102, Quote::new(104.00, 105.00, 102, "ABC"));
    price_source.add_quotes(102, Quote::new(10.00, 11.00, 102, "BCD"));
    price_source.add_quotes(103, Quote::new(104.00, 105.00, 103, "ABC"));
    price_source.add_quotes(103, Quote::new(12.00, 13.00, 103, "BCD"));

    let exchange = SingleExchangeBuilder::new()
        .with_clock(clock.clone())
        .with_price_source(price_source)
        .build();

    let mut brkr: SingleBroker<Dividend, HashMapCorporateEventsSource, Quote, HashMapPriceSource> =
        SingleBrokerBuilder::new().with_exchange(exchange).build();

    brkr.deposit_cash(&100_000.0);
    brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 100.0));
    brkr.send_order(Order::market(OrderType::MarketBuy, "BCD", 100.0));

    brkr.check();

    brkr.check();

    brkr.check();
}

fn benchmarks(c: &mut Criterion) {
    c.bench_function("full backtest", |b| b.iter(full_backtest_random_data));
    c.bench_function("trade test", |b| b.iter(trade_execution_logic));
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
