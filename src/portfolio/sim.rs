use std::collections::HashMap;

use math::round;

use crate::broker::order::{Order, OrderExecutor, OrderType};
use crate::broker::sim::SimulatedBroker;
use crate::broker::{BrokerEvent, CashManager, PositionInfo, PriceQuote};
use crate::data::DefaultDataSource;
use crate::portfolio::{Portfolio, PortfolioStats};
use crate::universe::{DefinedUniverse, StaticUniverse};

pub struct SimPortfolio {
    brkr: SimulatedBroker<DefaultDataSource>,
}

impl PortfolioStats for SimPortfolio {
    fn get_total_value(&self, universe: &StaticUniverse) -> f64 {
        let assets = universe.get_assets();
        let mut value = self.brkr.get_cash_balance();
        for a in assets {
            let symbol_value = self.brkr.get_position_value(a);
            if symbol_value.is_some() {
                value += symbol_value.unwrap()
            }
        }
        value
    }
}

impl SimPortfolio {
    pub fn new(brkr: SimulatedBroker<DefaultDataSource>) -> SimPortfolio {
        SimPortfolio { brkr }
    }

    pub fn set_date(&mut self, new_date: &i64) {
        self.brkr.set_date(new_date);
    }

    fn get_position_value(&self, symbol: &String) -> Option<f64> {
        self.brkr.get_position_value(symbol)
    }

    pub fn execute_orders(&mut self, orders: Vec<Order>) -> Vec<BrokerEvent> {
        self.brkr.execute_orders(orders)
    }

    fn get_position_diff(
        &self,
        symbol: &String,
        target_weights: &HashMap<String, f64>,
        total_value: f64,
    ) -> f64 {
        let target_value = target_weights.get(symbol).unwrap() * total_value;
        let curr_value = self.get_position_value(symbol).unwrap_or(0.0);
        target_value - curr_value
    }
}

impl Portfolio for SimPortfolio {
    fn deposit_cash(&mut self, cash: &f64) {
        self.brkr.deposit_cash(*cash);
    }

    fn update_weights(
        &self,
        target_weights: &HashMap<String, f64>,
        universe: &StaticUniverse,
    ) -> Vec<Order> {
        let total_value = self.get_total_value(universe);
        let mut orders: Vec<Order> = Vec::new();

        let mut buy_orders: Vec<Order> = Vec::new();
        let mut sell_orders: Vec<Order> = Vec::new();

        for symbol in target_weights.keys() {
            let diff_val = self.get_position_diff(symbol, target_weights, total_value);
            let quote = self.brkr.get_quote(symbol);
            match quote {
                Some(q) => {
                    if diff_val > 0.0 {
                        let target_shares = round::floor(diff_val / q.ask, 0);
                        let order = Order {
                            order_type: OrderType::MarketBuy,
                            symbol: symbol.clone(),
                            shares: target_shares,
                            price: None,
                        };
                        buy_orders.push(order);
                    } else {
                        let target_shares = round::floor(diff_val / q.bid, 0);
                        let order = Order {
                            order_type: OrderType::MarketSell,
                            symbol: symbol.clone(),
                            shares: target_shares * -1.0,
                            price: None,
                        };
                        sell_orders.push(order);
                    }
                }
                None => panic!("Can't find price for symbol"),
            }
        }
        //Sell orders have to be executed before buy orders
        orders.extend(sell_orders);
        orders.extend(buy_orders);
        orders
    }
}
