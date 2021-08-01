use math::round;
use std::collections::HashMap;

use crate::broker::{PositionInfo, PriceQuote};
use crate::data::universe::{DefinedUniverse, StaticUniverse};
use crate::types::Order;

pub trait Portfolio {
    fn update_weights(
        &self,
        target_weights: &HashMap<String, f64>,
        broker: &(impl PriceQuote + PositionInfo),
    ) -> Vec<Order>;
}

pub struct SimPortfolio {
    universe: StaticUniverse,
}

impl SimPortfolio {
    pub fn new(universe: StaticUniverse) -> SimPortfolio {
        SimPortfolio { universe }
    }

    fn get_total_value(&self, broker: &impl PositionInfo) -> f64 {
        let assets = self.universe.get_assets();
        let mut value = 0.0;
        for a in assets {
            let symbol_value = broker.get_position_value(a);
            if symbol_value.is_some() {
                value += symbol_value.unwrap()
            }
        }
        value
    }

    fn get_position_value(&self, symbol: &String, broker: &impl PositionInfo) -> Option<f64> {
        broker.get_position_value(symbol)
    }

    fn get_position_diff(
        &self,
        symbol: &String,
        broker: &impl PositionInfo,
        target_weights: &HashMap<String, f64>,
        total_value: f64,
    ) -> f64 {
        let target_value = target_weights.get(symbol).unwrap() * total_value;
        let curr_value = self.get_position_value(symbol, broker).unwrap_or(0.0);
        target_value - curr_value
    }
}

impl Portfolio for SimPortfolio {
    fn update_weights(
        &self,
        target_weights: &HashMap<String, f64>,
        broker: &(impl PriceQuote + PositionInfo),
    ) -> Vec<Order> {
        let total_value = self.get_total_value(broker);
        let mut orders = Vec::new();

        for symbol in target_weights.keys() {
            let diff_val = self.get_position_diff(symbol, broker, target_weights, total_value);
            let quote = broker.get_quote(symbol);
            match quote {
                Some(q) => {
                    if diff_val > 0.0 {
                        let target_shares = round::floor(diff_val / q.ask, 0) as i64;
                        let order = Order::build_order(target_shares, symbol);
                        orders.push(order);
                    } else {
                        let target_shares = round::floor(diff_val / q.bid, 0) as i64;
                        let order = Order::build_order(target_shares, symbol);
                        orders.push(order);
                    }
                }
                None => panic!("Can't find price for symbol"),
            }
        }
        orders
    }
}
