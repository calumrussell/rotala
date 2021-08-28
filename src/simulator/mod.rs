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
            let weights = self.system.calculate_weights();
            let orders = self.port.update_weights(&weights, &mut self.brkr);
            self.brkr.execute_orders(orders);
        }
    }

    pub fn calculate_perf(&self) -> (f64, f64) {
        let value = self.perf.get_portfolio_return();
        (value, 1.0)
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
