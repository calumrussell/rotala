use std::collections::HashMap;

use crate::broker::order::Order;
use crate::universe::StaticUniverse;

pub trait Portfolio {
    fn deposit_cash(&mut self, cash: &f64);
    fn update_weights(
        &self,
        target_weights: &HashMap<String, f64>,
        universe: &StaticUniverse,
    ) -> Vec<Order>;
}

pub trait PortfolioStats {
    fn get_total_value(&self, universe: &StaticUniverse) -> f64;
}
