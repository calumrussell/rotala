use alator::broker::uist::UistBrokerBuilder;
use alator::broker::{BrokerCost, CashOperations, SendOrder};
use alator::strategy::Strategy;
use criterion::{criterion_group, criterion_main, Criterion};
use rand::thread_rng;
use rand_distr::{Distribution, Uniform};

use alator::strategy::staticweight::StaticWeightStrategyBuilder;
use alator::types::{CashValue, PortfolioAllocation};
use rotala::exchange::uist::{UistV1, UistOrder};
use rotala::input::penelope::PenelopeBuilder;

fn full_backtest_random_data() {
    let mut source_builder = PenelopeBuilder::new();

    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    for date in 0..100 {
        source_builder.add_quote(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "ABC",
        );
        source_builder.add_quote(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "BCD",
        );
    }

    let (price_source, clock) =
        source_builder.build_with_frequency(rotala::clock::Frequency::Second);
    let initial_cash: CashValue = 100_000.0.into();

    let mut weights: PortfolioAllocation = PortfolioAllocation::new();
    weights.insert("ABC", 0.5);
    weights.insert("BCD", 0.5);

    let uist = UistV1::new(clock.clone(), price_source, "RANDOM");
    let simbrkr = UistBrokerBuilder::new()
        .with_exchange(uist)
        .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
        .build();

    let mut strat = StaticWeightStrategyBuilder::new()
        .with_brkr(simbrkr)
        .with_weights(weights)
        .with_clock(clock.clone())
        .default();

    strat.init(&initial_cash);
    strat.run();
}

fn trade_execution_logic() {
    let mut source_builder = PenelopeBuilder::new();
    source_builder.add_quote(100.00, 101.00, 100, "ABC");
    source_builder.add_quote(10.00, 11.00, 100, "BCD");
    source_builder.add_quote(100.00, 101.00, 101, "ABC");
    source_builder.add_quote(10.00, 11.00, 101, "BCD");
    source_builder.add_quote(104.00, 105.00, 102, "ABC");
    source_builder.add_quote(10.00, 11.00, 102, "BCD");
    source_builder.add_quote(104.00, 105.00, 103, "ABC");
    source_builder.add_quote(12.00, 13.00, 103, "BCD");

    let (price_source, clock) = source_builder.build();
    let uist = UistV1::new(clock, price_source, "FAKE");
    let mut brkr = UistBrokerBuilder::new().with_exchange(uist).build();

    brkr.deposit_cash(&100_000.0);
    brkr.send_order(UistOrder::market_buy("ABC", 100.0));
    brkr.send_order(UistOrder::market_buy("BCD", 100.0));

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
