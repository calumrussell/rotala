use crate::data::{CashValue, DateTime};
use crate::perf::PerfStruct;
use crate::strategy::Strategy;

pub struct SimContext {
    sim_dates: Vec<DateTime>,
    initial_cash: CashValue,
    strat: Box<dyn Strategy>,
}

impl SimContext {
    pub fn run(&mut self) {
        self.sim_dates.sort();
        self.strat.init(&self.initial_cash);
        for date in &self.sim_dates {
            self.strat.set_date(date);
            self.strat.update();
        }
    }

    pub fn calculate_perf(&mut self) -> PerfStruct {
        self.strat.get_perf()
    }

    pub fn new<T: Into<Box<dyn Strategy>>>(
        sim_dates: Vec<DateTime>,
        initial_cash: CashValue,
        strat: T,
    ) -> SimContext {
        let boxed: Box<dyn Strategy> = strat.into();
        SimContext {
            sim_dates,
            initial_cash,
            strat: boxed,
        }
    }
}
