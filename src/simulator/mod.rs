use crate::broker::order::OrderExecutor;
use crate::broker::sim::SimulatedBroker;
use crate::broker::CashManager;
use crate::data::SimSource;
use crate::perf::PortfolioPerformance;
use crate::portfolio::{Portfolio, SimPortfolio};
use crate::trading::TradingSystem;

pub struct Simulator<T>
where
    T: SimSource,
{
    sim_dates: Vec<i64>,
    port: SimPortfolio,
    brkr: SimulatedBroker<T>,
    system: Box<dyn TradingSystem>,
    perf: PortfolioPerformance,
    initial_cash: f64,
}

impl<T> Simulator<T>
where
    T: SimSource,
{
    pub fn run(&mut self) {
        self.sim_dates.sort();
        self.brkr.deposit_cash(self.initial_cash);
        for date in &self.sim_dates {
            self.perf.update(&self.port, &self.brkr);
            self.brkr.set_date(date);
            if self.system.should_trade_now(&date) {
                let weights = self.system.calculate_weights();
                let orders = self.port.update_weights(&weights, &mut self.brkr);
                self.brkr.execute_orders(orders);
            }
        }
    }

    pub fn calculate_perf(&mut self) -> (f64, f64, f64, f64) {
        let ret = self.perf.get_portfolio_return();
        let vol = self.perf.get_portfolio_volatility();
        let mdd = self.perf.get_portfolio_max_dd();
        let sharpe = self.perf.get_portfolio_sharpe_ratio();
        (ret, vol, mdd, sharpe)
    }

    pub fn new(
        sim_dates: Vec<i64>,
        port: SimPortfolio,
        brkr: SimulatedBroker<T>,
        system: Box<dyn TradingSystem>,
        perf: PortfolioPerformance,
        initial_cash: f64,
    ) -> Simulator<T> {
        Simulator {
            sim_dates,
            port,
            brkr,
            system,
            perf,
            initial_cash,
        }
    }
}
