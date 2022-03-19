use crate::perf::PortfolioPerformance;
use crate::strategy::Strategy;

pub struct SimContext {
    sim_dates: Vec<i64>,
    initial_cash: f64,
    strat: Box<dyn Strategy>,
    perf: PortfolioPerformance,
}

impl SimContext {
    pub fn run(&mut self) {
        self.sim_dates.sort();
        self.strat.init(&self.initial_cash);
        for date in &self.sim_dates {
            self.strat.set_date(date);
            let portfolio_val = self.strat.run();
            self.perf.update(portfolio_val);
        }
    }

    pub fn calculate_perf(&mut self) -> (f64, f64, f64, f64) {
        let ret = self.perf.get_portfolio_return();
        let vol = self.perf.get_portfolio_volatility();
        let mdd = self.perf.get_portfolio_max_dd();
        let sharpe = self.perf.get_portfolio_sharpe_ratio();
        (ret, vol, mdd, sharpe)
    }

    pub fn new<T: Into<Box<dyn Strategy>>>(
        sim_dates: Vec<i64>,
        initial_cash: f64,
        strat: T,
        perf: PortfolioPerformance,
    ) -> SimContext {
        let boxed: Box<dyn Strategy> = strat.into();
        SimContext {
            sim_dates,
            initial_cash,
            strat: boxed,
            perf,
        }
    }
}
