mod common;

use rand::distributions::Uniform;
use std::collections::HashMap;

use alator::broker::sim::SimulatedBroker;
use alator::broker::Quote;
use alator::data::{DataSourceSim, DefaultDataSource};
use alator::perf::PortfolioPerformance;
use alator::portfolio::sim::SimPortfolio;
use alator::simulator::Simulator;
use alator::strategy::randomfake::RandomStrategyRulesWithFakeDataSource;
use alator::universe::StaticUniverse;

use common::build_fake_quote_stream;

#[test]
fn datasource_integration_test() {
    let initial_cash = 1e6;

    let price_dist = Uniform::new(1.0, 100.0);
    let vol_dist = Uniform::new(0.1, 0.2);

    let length_in_days = 200;
    let seconds_in_day = 86_400;
    let start_date = 1609750800; //Date - 4/1/21 9:00:0000
    let end_date = start_date + (seconds_in_day * length_in_days);

    let abc_quotes = build_fake_quote_stream(
        &String::from("ABC"),
        price_dist,
        vol_dist,
        start_date..end_date,
        Some(seconds_in_day as usize),
    );
    let bcd_quotes = build_fake_quote_stream(
        &String::from("BCD"),
        price_dist,
        vol_dist,
        start_date..end_date,
        Some(seconds_in_day as usize),
    );
    let mut raw_data: HashMap<i64, Vec<Quote>> = HashMap::new();
    for (_a, b) in abc_quotes.iter().zip(bcd_quotes.iter()).enumerate() {
        raw_data.insert(b.0.date.clone(), vec![b.0.clone(), b.1.clone()]);
    }
    let dates = raw_data.keys().map(|d| d.clone()).collect();
    let source: DataSourceSim<DefaultDataSource> =
        DataSourceSim::<DefaultDataSource>::from_hashmap(raw_data);

    let universe = StaticUniverse::new(vec!["ABC", "BCD"]);
    let simbrkr = SimulatedBroker::new(source);
    let port = SimPortfolio::new(simbrkr);

    let strat = RandomStrategyRulesWithFakeDataSource::new(port, universe);
    let perf = PortfolioPerformance::new();
    let mut sim = Simulator::new(dates, initial_cash, &strat, perf);
    sim.run();
}
