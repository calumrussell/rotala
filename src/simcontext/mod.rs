use crate::{
    data::{CashValue, DateTime},
    strategy::Strategy,
};

pub struct SimContext {
    sim_dates: Vec<DateTime>,
    initial_cash: CashValue,
    strat: Box<dyn Strategy>,
}

pub struct PerfResults(f64, f64, f64, f64, f64, Vec<f64>, Vec<f64>, Vec<DateTime>);

impl SimContext {
    pub fn run(&mut self) {
        self.sim_dates.sort();
        self.strat.init(&self.initial_cash);
        for date in &self.sim_dates {
            self.strat.set_date(date);
            self.strat.run();
        }
    }

    pub fn calculate_perf(&mut self) -> PerfResults {
        let perf = self.strat.get_perf();
        PerfResults(
            perf.ret,
            perf.cagr,
            perf.vol,
            perf.mdd,
            perf.sharpe,
            perf.values,
            perf.returns,
            self.sim_dates.clone(),
        )
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
