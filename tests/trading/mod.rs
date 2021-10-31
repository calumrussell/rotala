use std::collections::HashMap;

use alator::trading::{DefaultTradingSchedule, TradingSchedule};
use alator::trading::{LastBusinessDayTradingSchedule, TradingSystem};

pub struct FixedWeightTradingSystem {
    target_weights: HashMap<String, f64>,
}

impl TradingSystem for FixedWeightTradingSystem {
    fn calculate_weights(&self) -> HashMap<String, f64> {
        self.target_weights.clone()
    }

    fn should_trade_now(&self, date: &i64) -> bool {
        DefaultTradingSchedule::should_trade(date)
    }
}

impl FixedWeightTradingSystem {
    pub fn new(weights: HashMap<String, f64>) -> FixedWeightTradingSystem {
        FixedWeightTradingSystem {
            target_weights: weights,
        }
    }
}

pub struct MonthlyRebalancingFixedWeightTradingSystem {
    target_weights: HashMap<String, f64>,
}

impl TradingSystem for MonthlyRebalancingFixedWeightTradingSystem {
    fn calculate_weights(&self) -> HashMap<String, f64> {
        self.target_weights.clone()
    }

    fn should_trade_now(&self, date: &i64) -> bool {
        LastBusinessDayTradingSchedule::should_trade(date)
    }
}

impl MonthlyRebalancingFixedWeightTradingSystem {
    pub fn new(weights: HashMap<String, f64>) -> Self {
        MonthlyRebalancingFixedWeightTradingSystem {
            target_weights: weights,
        }
    }
}
