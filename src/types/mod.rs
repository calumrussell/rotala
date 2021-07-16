use core::f64;
use std::collections::HashMap;

pub enum EventType {
    TradeCreated,
    SimPriceUpdate {
        date: u64,
        prices: HashMap<String, StockQuote>,
    },
    TargetWeight {
        weights: HashMap<String, f64>,
    },
    OrderCreated {
        orders: Vec<Order>,
    },
    TradeSuccess {
        order: Order,
        price: f64,
    },
}

pub struct Event {
    pub event_type: EventType,
}

#[derive(Clone, Copy)]
pub enum OrderType {
    MarketSell,
    MarketBuy,
}

#[derive(Clone)]
pub struct Order {
    pub order_type: OrderType,
    pub symbol: String,
    pub shares: i64,
}

#[derive(Copy, Clone)]
pub struct Quote {
    pub bid: f64,
    pub ask: f64,
    pub date: i64,
}

#[derive(Clone)]
pub struct StockQuote {
    pub symbol: String,
    pub quote: Quote,
}
