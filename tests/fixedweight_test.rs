mod common;
mod trading;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use alator::broker::SimulatedBroker;
use alator::data::{CSVDataSource, CSVDataSourceWrapper};
use alator::portfolio::SimPortfolio;
use alator::simulator::Simulator;
use alator::trading::TradingSystem;
use alator::types::StockQuote;

use trading::FixedWeightTradingSystem;

#[test]
fn fixedweight_integration_test() {
    let (universe, weights) = common::get_universe_weights();
    let mut raw_data: HashMap<i64, Vec<StockQuote>> = HashMap::new();
    common::build_csv(&mut raw_data);

    let raw_source = Rc::new(CSVDataSource { data: raw_data });
    let source = Rc::new(CSVDataSourceWrapper::new(Rc::clone(&raw_source)));

    let fws: Rc<Box<dyn TradingSystem>> = Rc::new(Box::new(FixedWeightTradingSystem::new(weights)));
    let brkr = Box::new(SimulatedBroker::new());
    let port = Rc::new(RefCell::new(SimPortfolio::new(brkr, universe)));

    let mut sim = Simulator::new(source.clone(), port.clone(), fws.clone(), 100, 50000);
    sim.run();
}
