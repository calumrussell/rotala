use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::data::DataSourceSim;
use crate::portfolio::Portfolio;
use crate::trading::TradingSystem;
use crate::types::StockQuote;

pub struct Simulator {
    data: Rc<dyn DataSourceSim>,
    port: Rc<RefCell<dyn Portfolio>>,
    system: Rc<Box<dyn TradingSystem>>,
    start_dt: i64,
    end_dt: i64,
    ctxt: SimulatorContext,
}

impl Simulator {
    pub fn run(&mut self) {
        for (pos, prices) in &mut self.ctxt {
            let weights = self.system.calculate_weights();
            self.port.borrow_mut().update_weights(&weights, &prices)
        }
    }

    pub fn new(
        data: Rc<dyn DataSourceSim>,
        port: Rc<RefCell<dyn Portfolio>>,
        system: Rc<Box<dyn TradingSystem>>,
        start_dt: i64,
        end_dt: i64,
    ) -> Simulator {
        let ctxt = SimulatorContext::new(data.clone(), start_dt);
        Simulator {
            data,
            port,
            system,
            start_dt,
            end_dt,
            ctxt,
        }
    }
}

struct SimulatorContext {
    pos: i64,
    idx: usize,
    data: Rc<dyn DataSourceSim>,
    keys: Vec<i64>,
}

impl SimulatorContext {
    fn new(data: Rc<dyn DataSourceSim>, start_dt: i64) -> SimulatorContext {
        let keys = data.get_keys().iter().map(|&r| r.clone()).collect();
        SimulatorContext {
            pos: start_dt,
            idx: 0,
            data,
            keys,
        }
    }
}

impl Iterator for SimulatorContext {
    type Item = (i64, HashMap<String, StockQuote>);

    fn next(&mut self) -> Option<Self::Item> {
        let curr_date = self.keys.get(self.idx);
        self.idx += 1;
        if let Some(d) = curr_date {
            let mut prices = HashMap::new();
            let quotes = self.data.get_date(d);
            if quotes.is_none() {
                return None;
            }
            for quote in quotes.unwrap() {
                prices.insert(quote.symbol.clone(), quote.clone());
            }
            Some((d.clone(), prices))
        } else {
            None
        }
    }
}
