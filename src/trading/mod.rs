use std::collections::HashMap;

pub trait TradingSystem {
    fn calculate_weights(&self) -> HashMap<String, f64>;
}
