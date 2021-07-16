use itertools::Itertools;
use math::round;
use std::collections::HashMap;

use super::types;
use crate::broker::ExecutableSim;
use crate::data::universe::DefinedUniverse;
use crate::types::{EventType, StockQuote};

pub trait Portfolio {
    fn get_total_value(&self) -> f64;
    fn get_symbol_shares(&self, symbol: &String) -> &i64;
    fn update_weights(
        &mut self,
        target_weights: &HashMap<String, f64>,
        prices: &HashMap<String, StockQuote>,
    );
}

pub struct SimPortfolio {
    brkr: Box<dyn ExecutableSim>,
    cash: f64,
    allocations: HashMap<String, i64>,
    total_value: f64,
}

impl Portfolio for SimPortfolio {
    fn get_total_value(&self) -> f64 {
        self.total_value
    }

    fn get_symbol_shares(&self, symbol: &String) -> &i64 {
        self.allocations.get(symbol).unwrap()
    }

    fn update_weights(
        &mut self,
        target_weights: &HashMap<String, f64>,
        prices: &HashMap<String, StockQuote>,
    ) {
        let total_value = self.get_total_value();
        let mut orders = Vec::new();

        for symbol in target_weights.keys() {
            let target_weight = target_weights.get(symbol);
            if target_weight.is_none() {
                panic!("Weight not found for symbol");
            }
            let target_val = target_weights.get(symbol).unwrap() * total_value;
            let curr_shares = self.get_symbol_shares(symbol).clone();
            let curr_price = prices.get(symbol).unwrap().quote.ask;
            let curr_val = curr_shares as f64 * curr_price;
            let diff_val = target_val - curr_val;

            let quote = prices.get(symbol);
            if quote.is_none() {
                panic!("Unable to find price for symbol");
            }
            if diff_val > 0.0 {
                let target_shares = round::floor(diff_val / quote.unwrap().quote.ask, 0) as i64;
                let order_type = types::OrderType::MarketBuy;
                let order = types::Order {
                    order_type,
                    symbol: symbol.clone(),
                    shares: target_shares,
                };
                orders.push(order);
            } else {
                let target_shares = round::floor(diff_val / quote.unwrap().quote.bid, 0) as i64;
                let order_type = types::OrderType::MarketSell;
                let order = types::Order {
                    order_type,
                    symbol: symbol.clone(),
                    shares: target_shares,
                };
                orders.push(order);
            }
        }

        let trades = orders
            .iter()
            .map(|o| self.brkr.execute_order(o, prices))
            .collect_vec();
        for trade in trades {
            if trade.is_some() {
                match trade.unwrap().event_type {
                    EventType::TradeSuccess { order, price } => {
                        let trade_value = order.shares as f64 * price;
                        let new_cash = self.cash - trade_value;
                        self.cash += new_cash;

                        let curr_alloc = self.allocations.get(&order.symbol).unwrap();
                        let new_alloc = curr_alloc + order.shares;
                        self.allocations.insert(order.symbol.clone(), new_alloc);
                    }
                    _ => (),
                }
            }
        }
    }
}

impl SimPortfolio {
    pub fn new(brkr: Box<dyn ExecutableSim>, universe: Box<dyn DefinedUniverse>) -> SimPortfolio {
        let initial_cash = 1e6;
        let mut allocations = HashMap::new();

        let _: Vec<Option<i64>> = universe
            .get_assets()
            .iter()
            .map(|f| allocations.insert(f.clone(), 0))
            .collect_vec();

        SimPortfolio {
            brkr,
            cash: initial_cash,
            total_value: initial_cash,
            allocations,
        }
    }
}
