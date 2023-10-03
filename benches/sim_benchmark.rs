use alator::broker::{
    BrokerCost, Dividend, Order, OrderType, Quote, ReceievesOrders,
    TransferCash,
};
use alator::broker::implement::single::{SingleBroker, SingleBrokerBuilder};
use alator::clock::ClockBuilder;
use alator::exchange::implement::single::SingleExchangeBuilder;
use alator::input::{DefaultCorporateEventsSource, DefaultPriceSource};
use alator::simcontext::SimContextBuilder;
use alator::strategy::implement::staticweight::StaticWeightStrategyBuilder;
use alator::types::{CashValue, Frequency, PortfolioAllocation};

use criterion::{criterion_group, criterion_main, Criterion};
use rand::thread_rng;
use rand_distr::{Distribution, Uniform};

fn full_backtest_random_data() {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();
    let length_in_days: i64 = 100;
    let start_date: i64 = 1609750800; //Date - 4/1/21 9:00:0000
    let clock = ClockBuilder::with_length_in_days(start_date, length_in_days)
        .with_frequency(&Frequency::Daily)
        .build();

    let initial_cash: CashValue = 100_000.0.into();

    let mut price_source = DefaultPriceSource::new(clock.clone());
    for date in clock.peek() {
        price_source.add_quotes(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "ABC",
        );
        price_source.add_quotes(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "BCD",
        );
    }

    let mut weights: PortfolioAllocation = PortfolioAllocation::new();
    weights.insert("ABC", 0.5);
    weights.insert("BCD", 0.5);

    let exchange = SingleExchangeBuilder::new()
        .with_price_source(price_source)
        .with_clock(clock.clone())
        .build();

    let simbrkr: SingleBroker<Dividend, DefaultCorporateEventsSource, Quote, DefaultPriceSource> =
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

    let mut price_source = DefaultPriceSource::new(clock.clone());
    price_source.add_quotes(100.00, 101.00, 100, "ABC");
    price_source.add_quotes(10.00, 11.00, 100, "BCD");
    price_source.add_quotes(100.00, 101.00, 101, "ABC");
    price_source.add_quotes(10.00, 11.00, 101, "BCD");
    price_source.add_quotes(104.00, 105.00, 102, "ABC");
    price_source.add_quotes(10.00, 11.00, 102, "BCD");
    price_source.add_quotes(104.00, 105.00, 103, "ABC");
    price_source.add_quotes(12.00, 13.00, 103, "BCD");

    let exchange = SingleExchangeBuilder::new()
        .with_clock(clock.clone())
        .with_price_source(price_source)
        .build();

    let mut brkr: SingleBroker<Dividend, DefaultCorporateEventsSource, Quote, DefaultPriceSource> =
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
