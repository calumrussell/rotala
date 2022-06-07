use std::collections::HashMap;

use crate::broker::Order;

pub trait Portfolio {
    fn deposit_cash(&mut self, cash: &u64) -> bool;
    fn withdraw_cash(&mut self, cash: &u64) -> bool;
    fn withdraw_cash_with_liquidation(&mut self, cash: &u64) -> bool;
    fn update_weights(&self, target_weights: &HashMap<String, f64>) -> Vec<Order>;
}

pub trait PortfolioStats {
    fn get_total_value(&self) -> f64;
    fn get_liquidation_value(&self) -> f64;
    fn get_position_value(&self, ticker: &String) -> Option<f64>;
    fn get_position_liquidation_value(&self, symbol: &String) -> Option<f64>;
    fn get_position_qty(&self, ticker: &String) -> Option<f64>;
    fn get_current_state(&self) -> PortfolioState;
    fn get_holdings(&self) -> Holdings;
    fn get_cash_value(&self) -> u64;
}

#[derive(Clone)]
pub struct Holdings {
    data: HashMap<String, f64>,
}

impl Holdings {
    pub fn get_ticker(&self, ticker: &String) -> Option<&f64> {
        self.data.get(ticker)
    }

    pub fn put(&mut self, ticker: &String, value: &f64) {
        self.data.insert(ticker.clone(), value.clone());
    }

    pub fn new() -> Self {
        let data = HashMap::new();
        Holdings { data }
    }
}

#[derive(Clone)]
pub struct PortfolioState {
    pub value: f64,
    pub positions: Holdings,
    pub net_cash_flow: f64,
}
