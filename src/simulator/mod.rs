use crate::broker::{SimulatedBroker, CashManager, OrderExecutor};
use crate::data::SimSource;
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
            self.brkr.set_date(date.clone());
            let weights = self.system.calculate_weights();
            let orders = self.port.update_weights(&weights, &mut self.brkr);
            self.brkr.execute_orders(orders);
        }
    }

    pub fn new(
        sim_dates: Vec<i64>,
        port: SimPortfolio,
        brkr: SimulatedBroker<T>,
        system: Box<dyn TradingSystem>,
        initial_cash: f64,
    ) -> Simulator<T> {
        Simulator {
            sim_dates,
            port,
            brkr,
            system,
            initial_cash,
        }
    }
}
