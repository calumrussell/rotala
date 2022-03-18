use std::collections::HashMap;

mod book;
mod execution;
pub mod order;
mod record;
pub mod sim;

#[derive(Clone, Debug)]
pub struct Quote {
    pub bid: f64,
    pub ask: f64,
    pub date: i64,
    pub symbol: String,
}

#[derive(Clone, Debug)]
pub struct Trade {
    symbol: String,
    value: f64,
    quantity: f64,
}

#[derive(Clone)]
pub struct Holdings(HashMap<String, f64>);

#[derive(Clone)]

pub enum BrokerEvent {
    TradeSuccess(Trade),
    TradeFailure(order::Order),
    OrderCreated(order::Order),
    OrderFailure(order::Order),
    SuccessfulWithdraw(f64),
    CashTransactionSuccess(f64),
    InsufficientCash(f64),
}

pub trait CashManager {
    fn withdraw_cash(&mut self, cash: f64) -> BrokerEvent;
    fn deposit_cash(&mut self, cash: f64) -> BrokerEvent;
    fn debit(&mut self, value: f64) -> BrokerEvent;
    fn credit(&mut self, value: f64) -> BrokerEvent;
    fn get_cash_balance(&self) -> f64;
}

pub trait PositionInfo {
    fn get_position_qty(&self, symbol: &String) -> Option<f64>;
    fn get_position_value(&self, symbol: &String) -> Option<f64>;
    fn get_position_cost(&self, symbol: &String) -> Option<f64>;
    fn get_position_profit(&self, symbol: &String) -> Option<f64>;
}

pub trait PriceQuote {
    fn get_quote(&self, symbol: &String) -> Option<Quote>;
}

pub trait ClientControlled {
    fn update_holdings(&mut self, symbol: &String, change: &f64);
    fn get_holdings(&self) -> &Holdings;
    fn get(&self, symbol: &String) -> Option<&f64>;
}

pub trait TradeLedger {
    fn record(&mut self, trade: &Trade);
    fn cost_basis(&self, symbol: &String) -> Option<f64>;
}

pub trait PendingOrders {
    fn insert_order(&mut self, order: &order::Order);
    fn delete_order(&mut self, id: &u8);
}
