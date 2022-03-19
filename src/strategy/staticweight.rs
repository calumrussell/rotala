use std::collections::HashMap;

use crate::sim::portfolio::SimPortfolio;
use crate::portfolio::{Portfolio, PortfolioStats};
use crate::schedule::{LastBusinessDayTradingSchedule, TradingSchedule};
use crate::strategy::Strategy;
use crate::universe::StaticUniverse;

#[derive(Clone)]
pub struct StaticWeightStrategyRulesMonthlyRebalancing {
    portfolio: SimPortfolio,
    date: i64,
    universe: StaticUniverse,
    target_weights: Vec<HashMap<String, f64>>,
    count: usize,
}

impl Strategy for StaticWeightStrategyRulesMonthlyRebalancing {
    fn set_date(&mut self, date: &i64) {
        self.portfolio.set_date(date);
        self.date = *date;
    }

    fn init(&mut self, initital_cash: &f64) {
        self.portfolio.deposit_cash(initital_cash);
    }

    fn run(&mut self) -> f64 {
        if LastBusinessDayTradingSchedule::should_trade(&self.date) {
            let weights = &self.target_weights[self.count];
            self.count += 1;
            let orders = self.portfolio.update_weights(weights, &self.universe);

            if orders.len() > 0 {
                self.portfolio.execute_orders(orders);
            }
        }
        self.portfolio.get_total_value(&self.universe)
    }
}

impl StaticWeightStrategyRulesMonthlyRebalancing {
    pub fn new(
        portfolio: SimPortfolio,
        universe: StaticUniverse,
        target_weights: Vec<HashMap<String, f64>>,
    ) -> Self {
        StaticWeightStrategyRulesMonthlyRebalancing {
            portfolio,
            date: -1,
            universe,
            target_weights,
            count: 0,
        }
    }
}

impl From<&StaticWeightStrategyRulesMonthlyRebalancing>
    for Box<StaticWeightStrategyRulesMonthlyRebalancing>
{
    fn from(strat: &StaticWeightStrategyRulesMonthlyRebalancing) -> Self {
        let owned: StaticWeightStrategyRulesMonthlyRebalancing = strat.clone();
        Box::new(owned)
    }
}

impl From<&StaticWeightStrategyRulesMonthlyRebalancing> for Box<dyn Strategy> {
    fn from(strat: &StaticWeightStrategyRulesMonthlyRebalancing) -> Self {
        let owned: StaticWeightStrategyRulesMonthlyRebalancing = strat.clone();
        Box::new(owned)
    }
}
