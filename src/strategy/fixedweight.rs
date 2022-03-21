use std::collections::HashMap;

use crate::portfolio::{Portfolio, PortfolioStats};
use crate::schedule::{DefaultTradingSchedule, TradingSchedule};
use crate::sim::portfolio::SimPortfolio;
use crate::strategy::Strategy;
use crate::universe::StaticUniverse;

#[derive(Clone)]
pub struct FixedWeightStrategy {
    portfolio: SimPortfolio,
    date: i64,
    universe: StaticUniverse,
    target_weights: HashMap<String, f64>,
}

impl Strategy for FixedWeightStrategy {
    fn set_date(&mut self, date: &i64) {
        self.portfolio.set_date(date);
        self.date = *date;
    }

    fn init(&mut self, initital_cash: &f64) {
        self.portfolio.deposit_cash(initital_cash);
    }

    fn run(&mut self) -> f64 {
        if DefaultTradingSchedule::should_trade(&self.date) {
            let orders = self
                .portfolio
                .update_weights(&self.target_weights, &self.universe);
            if orders.len() > 0 {
                self.portfolio.execute_orders(orders);
            }
        }
        self.portfolio.get_total_value(&self.universe)
    }
}

impl FixedWeightStrategy {
    pub fn new(
        portfolio: SimPortfolio,
        universe: StaticUniverse,
        target_weights: HashMap<String, f64>,
    ) -> Self {
        FixedWeightStrategy {
            portfolio,
            date: -1,
            universe,
            target_weights,
        }
    }
}

impl From<&FixedWeightStrategy> for Box<FixedWeightStrategy> {
    fn from(strat: &FixedWeightStrategy) -> Self {
        let owned: FixedWeightStrategy = strat.clone();
        Box::new(owned)
    }
}

impl From<&FixedWeightStrategy> for Box<dyn Strategy> {
    fn from(strat: &FixedWeightStrategy) -> Self {
        let owned: FixedWeightStrategy = strat.clone();
        Box::new(owned)
    }
}
