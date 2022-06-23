use crate::broker::Order;
use crate::data::{CashValue, PortfolioAllocation};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct PortfolioValues(pub HashMap<String, CashValue>);

impl PortfolioValues {
    pub fn insert(&mut self, ticker: &str, value: &CashValue) {
        self.0.insert(ticker.to_string(), *value);
    }

    pub fn new() -> Self {
        let map: HashMap<String, CashValue> = HashMap::new();
        Self(map)
    }
}

impl Default for PortfolioValues {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Portfolio {
    fn deposit_cash(&mut self, cash: &CashValue) -> bool;
    fn withdraw_cash(&mut self, cash: &CashValue) -> bool;
    fn withdraw_cash_with_liquidation(&mut self, cash: &CashValue) -> bool;
    fn update_weights(&self, target_weights: &PortfolioAllocation) -> Vec<Order>;
}

pub trait PortfolioStats {
    fn get_total_value(&self) -> CashValue;
    fn get_liquidation_value(&self) -> CashValue;
    fn get_position_value(&self, ticker: &str) -> Option<CashValue>;
    fn get_position_liquidation_value(&self, symbol: &str) -> Option<CashValue>;
    fn get_position_qty(&self, ticker: &str) -> Option<f64>;
    fn get_current_state(&self) -> PortfolioState;
    fn get_holdings(&self) -> PortfolioValues;
    fn get_cash_value(&self) -> CashValue;
}

#[derive(Clone, Debug)]
pub struct PortfolioState {
    pub value: CashValue,
    pub positions: PortfolioValues,
    pub net_cash_flow: CashValue,
}
