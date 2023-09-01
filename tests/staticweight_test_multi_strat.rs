use alator::clock::{Clock, ClockBuilder};
use alator::exchange::ConcurrentExchangeBuilder;
use alator::input::HashMapInputBuilder;
use alator::strategy::AsyncStaticWeightStrategyBuilder;
use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use std::collections::HashMap;
use std::sync::Arc;

use alator::broker::{BrokerCost, ConcurrentBrokerBuilder, Quote};
use alator::input::HashMapInput;
use alator::simcontext::SimContextMultiBuilder;
use alator::types::{CashValue, DateTime, Frequency, PortfolioAllocation};

fn build_data(clock: Clock) -> HashMapInput {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

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

    let source = HashMapInputBuilder::new()
        .with_quotes(raw_data)
        .with_clock(clock.clone())
        .build();
    source
}

#[tokio::test]
async fn staticweight_integration_test() {
    env_logger::init();
    let initial_cash: CashValue = 100_000.0.into();
    let length_in_days: i64 = 1000;
    let start_date: i64 = 1609750800; //Date - 4/1/21 9:00:0000
    let clock = ClockBuilder::with_length_in_days(start_date, length_in_days)
        .with_frequency(&Frequency::Daily)
        .build();

    let data = build_data(clock.clone());

    let mut first_weights: PortfolioAllocation = PortfolioAllocation::new();
    first_weights.insert("ABC", 0.5);
    first_weights.insert("BCD", 0.5);

    let mut second_weights: PortfolioAllocation = PortfolioAllocation::new();
    second_weights.insert("ABC", 0.3);
    second_weights.insert("BCD", 0.7);

    let mut third_weights: PortfolioAllocation = PortfolioAllocation::new();
    third_weights.insert("ABC", 0.7);
    third_weights.insert("BCD", 0.3);

    let mut exchange = ConcurrentExchangeBuilder::new()
        .with_data_source(data.clone())
        .with_clock(clock.clone())
        .build();

    let simbrkr_first = ConcurrentBrokerBuilder::new()
        .with_data(data.clone())
        .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
        .build(&mut exchange)
        .await;

    let strat_first = AsyncStaticWeightStrategyBuilder::new()
        .with_brkr(simbrkr_first)
        .with_weights(first_weights)
        .with_clock(clock.clone())
        .default();

    let simbrkr_second = ConcurrentBrokerBuilder::new()
        .with_data(data.clone())
        .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
        .build(&mut exchange)
        .await;

    let strat_second = AsyncStaticWeightStrategyBuilder::new()
        .with_brkr(simbrkr_second)
        .with_weights(second_weights)
        .with_clock(clock.clone())
        .default();

    let simbrkr_third = ConcurrentBrokerBuilder::new()
        .with_data(data.clone())
        .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
        .build(&mut exchange)
        .await;

    let strat_third = AsyncStaticWeightStrategyBuilder::new()
        .with_brkr(simbrkr_third)
        .with_weights(third_weights)
        .with_clock(clock.clone())
        .default();

    let mut sim = SimContextMultiBuilder::new()
        .with_clock(clock.clone())
        .with_exchange(exchange)
        .add_strategy(strat_first)
        .add_strategy(strat_second)
        .add_strategy(strat_third)
        .init(&initial_cash)
        .await;

    sim.run().await;

    let _perf = sim.perf(Frequency::Daily);
}
