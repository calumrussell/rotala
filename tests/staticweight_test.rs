mod common;

use rand::distributions::Uniform;
use std::collections::HashMap;

use alator::broker::Quote;
use alator::data::DataSource;
use alator::perf::PortfolioPerformance;
use alator::sim::broker::SimulatedBroker;
use alator::sim::portfolio::SimPortfolio;
use alator::simcontext::SimContext;
use alator::strategy::staticweight::StaticWeightStrategyRulesMonthlyRebalancing;

use common::build_fake_quote_stream;

#[test]
fn staticweight_integration_test() {
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

    let mut weights: Vec<HashMap<String, f64>> = Vec::new();
    for _i in 0..length_in_days {
        let mut temp: HashMap<String, f64> = HashMap::new();
        temp.insert(String::from("ABC"), 0.5);
        temp.insert(String::from("BCD"), 0.5);
        weights.push(temp);
    }

    let dates = raw_data.keys().map(|d| d.clone()).collect();
    let source = DataSource::from_hashmap(raw_data);

    let simbrkr = SimulatedBroker::new(source);
    let port = SimPortfolio::new(simbrkr);

    let perf = PortfolioPerformance::yearly();
    let strat = StaticWeightStrategyRulesMonthlyRebalancing::new(port, perf, weights);
    let mut sim = SimContext::new(dates, initial_cash, &strat);
    sim.run();
}
