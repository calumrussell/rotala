mod common;
mod trading;

use std::collections::HashMap;
use std::rc::Rc;

use alator::broker::{Quote, SimulatedBroker};
use alator::data::{CSVDataSource, DataSourceSim};
use alator::portfolio::SimPortfolio;
use alator::simulator::Simulator;

use trading::FixedWeightTradingSystem;

#[test]
fn fixedweight_integration_test() {
    let initial_cash = 1e6;
    let (universe, weights) = common::get_universe_weights();

    let mut raw_data: HashMap<i64, Vec<Quote>> = HashMap::new();
    common::build_csv(&mut raw_data);

    let dates = raw_data.keys().map(|d| d.clone()).collect();
    let source: DataSourceSim<CSVDataSource> = DataSourceSim::<CSVDataSource>::get_csv(raw_data);
    let rc_source = Rc::new(source);

    let simbrkr = SimulatedBroker::new(Rc::clone(&rc_source));

    let port = SimPortfolio::new(universe);
    let fws = Box::new(FixedWeightTradingSystem::new(weights));

    let mut sim = Simulator::new(dates, port, simbrkr, fws, initial_cash);
    sim.run();
}
