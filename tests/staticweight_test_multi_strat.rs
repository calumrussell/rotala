use alator::clock::{Clock, ClockBuilder};
use alator::exchange::ConcurrentExchangeBuilder;
use alator::input::{HashMapCorporateEventsSource, HashMapPriceSource};
use alator::strategy::{AsyncStaticWeightStrategy, AsyncStaticWeightStrategyBuilder};
use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;

use alator::broker::{BrokerCost, ConcurrentBroker, ConcurrentBrokerBuilder, Dividend, Quote};
use alator::simcontext::{SimContextMulti, SimContextMultiBuilder};
use alator::types::{CashValue, Frequency, PortfolioAllocation};

fn build_data(clock: Clock) -> HashMapPriceSource {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    let mut price_source = HashMapPriceSource::new(clock.clone());
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
        price_source.add_quotes(date, q1);
        price_source.add_quotes(date, q2);
    }

    price_source
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
        .with_price_source(data)
        .with_clock(clock.clone())
        .build();

    let simbrkr_first: ConcurrentBroker<Dividend, HashMapCorporateEventsSource, Quote> =
        ConcurrentBrokerBuilder::new()
            .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
            .build(&mut exchange)
            .await;

    let strat_first = AsyncStaticWeightStrategyBuilder::new()
        .with_brkr(simbrkr_first)
        .with_weights(first_weights)
        .with_clock(clock.clone())
        .default();

    let simbrkr_second: ConcurrentBroker<Dividend, HashMapCorporateEventsSource, Quote> =
        ConcurrentBrokerBuilder::new()
            .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
            .build(&mut exchange)
            .await;

    let strat_second = AsyncStaticWeightStrategyBuilder::new()
        .with_brkr(simbrkr_second)
        .with_weights(second_weights)
        .with_clock(clock.clone())
        .default();

    let simbrkr_third: ConcurrentBroker<Dividend, HashMapCorporateEventsSource, Quote> =
        ConcurrentBrokerBuilder::new()
            .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
            .build(&mut exchange)
            .await;

    let strat_third = AsyncStaticWeightStrategyBuilder::new()
        .with_brkr(simbrkr_third)
        .with_weights(third_weights)
        .with_clock(clock.clone())
        .default();

    let mut sim: SimContextMulti<
        Dividend,
        Quote,
        HashMapPriceSource,
        AsyncStaticWeightStrategy<Dividend, HashMapCorporateEventsSource, Quote>,
    > = SimContextMultiBuilder::new()
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
