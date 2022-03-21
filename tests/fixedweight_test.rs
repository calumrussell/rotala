mod common;

use alator::data::DataSource;
use alator::perf::PortfolioPerformance;
use alator::sim::broker::SimulatedBroker;
use alator::sim::portfolio::SimPortfolio;
use alator::simcontext::SimContext;
use alator::universe::StaticUniverse;

use alator::strategy::fixedweight::FixedWeightStrategy;

#[test]
fn fixedweight_integration_test() {
    let initial_cash = 1e6;
    let (universe, weights) = common::get_universe_weights();
    let raw_data = common::build_data(&universe);

    let dates = raw_data.keys().map(|d| d.clone()).collect();
    let source = DataSource::from_hashmap(raw_data);

    let universe = StaticUniverse::new(vec!["ABC", "BCD"]);
    let simbrkr = SimulatedBroker::new(source);
    let port = SimPortfolio::new(simbrkr);

    let strat = FixedWeightStrategy::new(port, universe, weights);
    let perf = PortfolioPerformance::new();
    let mut sim = SimContext::new(dates, initial_cash, &strat, perf);
    sim.run();
}
