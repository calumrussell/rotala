use std::collections::VecDeque;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::input::athena::Depth;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Quote {
    pub bid: f64,
    pub bid_volume: f64,
    pub ask: f64,
    pub ask_volume: f64,
    pub date: i64,
    pub symbol: String,
}

impl From<crate::input::athena::BBO> for Quote {
    fn from(value: crate::input::athena::BBO) -> Self {
        Self {
            bid: value.bid,
            bid_volume: value.bid_volume,
            ask: value.ask,
            ask_volume: value.ask_volume,
            date: value.date,
            symbol: value.symbol,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum OrderType {
    MarketSell,
    MarketBuy,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Order {
    pub order_type: OrderType,
    pub symbol: String,
    pub qty: f64,
    pub price: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum TradeType {
    Buy,
    Sell,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Trade {
    pub symbol: String,
    pub value: f64,
    pub quantity: f64,
    pub date: i64,
    pub typ: TradeType,
}

pub struct OrderBook {
    inner: VecDeque<Order>,
    last_inserted: u64,
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            inner: std::collections::VecDeque::new(),
            last_inserted: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn trade_loop(depth: &Depth, order: &Order) -> Option<Trade> {
        let best = depth.get_bbo()?;
        match order.order_type {
            OrderType::MarketBuy => {
                if order.price.unwrap() >= best.ask {
                    Some(Trade {
                        symbol: order.symbol.clone(),
                        value: best.ask * order.qty,
                        quantity: order.qty,
                        date: depth.date,
                        typ: TradeType::Buy,
                    })
                } else {
                    None
                }
            }
            OrderType::MarketSell => {
                if order.price.unwrap() <= best.bid {
                    Some(Trade {
                        symbol: order.symbol.clone(),
                        value: best.bid * order.qty,
                        quantity: order.qty,
                        date: depth.date,
                        typ: TradeType::Sell,
                    })
                } else {
                    None
                }
            }
        }
    }

    pub fn execute_orders(&mut self, quotes: crate::input::athena::DateQuotes) -> Vec<Trade> {
        let mut trade_results = Vec::new();
        if self.is_empty() {
            return trade_results;
        }

        for order in self.inner.iter() {
            let security_id = &order.symbol;

            if let Some(depth) = quotes.get(security_id) {
                if let Some(trade) = Self::trade_loop(depth, order) {
                    trade_results.push(trade);
                }
            }
        }
        trade_results
    }
}
