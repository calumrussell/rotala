mod common;
mod trading;

use std::rc::Rc;

use alator::broker::sim::SimulatedBroker;
use alator::data::{DataSourceSim, DefaultDataSource};
use alator::perf::PortfolioPerformance;
use alator::portfolio::SimPortfolio;
use alator::simulator::Simulator;

use trading::FixedWeightTradingSystem;

#[test]
fn fixedweight_integration_test() {
    let initial_cash = 1e6;
    let (universe, weights) = common::get_universe_weights();
    let raw_data = common::build_data(&universe);

    let dates = raw_data.keys().map(|d| d.clone()).collect();
    let source: DataSourceSim<DefaultDataSource> =
        DataSourceSim::<DefaultDataSource>::from_hashmap(raw_data);
    let rc_source = Rc::new(source);

    let simbrkr = SimulatedBroker::new(Rc::clone(&rc_source));
    let port = SimPortfolio::new(Rc::clone(&universe));
    let fws = Box::new(FixedWeightTradingSystem::new(weights));
    let perf = PortfolioPerformance::new();

    let mut sim = Simulator::new(dates, port, simbrkr, fws, perf, initial_cash);
    sim.run();
}
