mod common;
mod trading;

use rand::distributions::Uniform;
use std::collections::HashMap;
use std::rc::Rc;

use alator::broker::sim::SimulatedBroker;
use alator::broker::Quote;
use alator::data::universe::StaticUniverse;
use alator::data::{DataSourceSim, DefaultDataSource};
use alator::perf::PortfolioPerformance;
use alator::portfolio::SimPortfolio;
use alator::simulator::Simulator;

use common::build_fake_quote_stream;
use trading::MonthlyRebalancingFixedWeightTradingSystem;

#[test]
fn fixedweight_integration_test() {
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

    let universe = StaticUniverse::new(vec!["ABC", "BCD"]);
    let mut weights: HashMap<String, f64> = HashMap::new();
    weights.insert(String::from("ABC"), 0.5);
    weights.insert(String::from("BCD"), 0.5);

    let dates = raw_data.keys().map(|d| d.clone()).collect();
    let source: DataSourceSim<DefaultDataSource> =
        DataSourceSim::<DefaultDataSource>::from_hashmap(raw_data);
    let rc_source = Rc::new(source);

    let simbrkr = SimulatedBroker::new(Rc::clone(&rc_source));
    let port = SimPortfolio::new(universe);
    let fws = Box::new(MonthlyRebalancingFixedWeightTradingSystem::new(weights));
    let perf = PortfolioPerformance::new();

    let mut sim = Simulator::new(dates, port, simbrkr, fws, perf, initial_cash);
    sim.run();
}
