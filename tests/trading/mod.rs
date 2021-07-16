use std::collections::HashMap;

use alator::trading::TradingSystem;

pub struct FixedWeightTradingSystem {
    target_weights: HashMap<String, f64>,
}

impl TradingSystem for FixedWeightTradingSystem {
    fn calculate_weights(&self) -> HashMap<String, f64> {
        self.target_weights.clone()
    }
}

impl FixedWeightTradingSystem {
    pub fn new(weights: HashMap<String, f64>) -> FixedWeightTradingSystem {
        FixedWeightTradingSystem {
            target_weights: weights,
        }
    }
}
