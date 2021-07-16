use std::collections::HashMap;

use crate::types::{Event, EventType, Order, OrderType, StockQuote};

pub trait Executable {
    fn execute_order(&self, order: &Order) -> Option<Event>;
}

pub trait ExecutableSim {
    fn execute_order(&self, order: &Order, prices: &HashMap<String, StockQuote>) -> Option<Event>;
}

pub struct SimulatedBroker;

impl ExecutableSim for SimulatedBroker {
    fn execute_order(&self, order: &Order, prices: &HashMap<String, StockQuote>) -> Option<Event> {
        match order.order_type {
            OrderType::MarketBuy => {
                let quote_res = prices.get(&order.symbol);

                if quote_res.is_none() {
                    return None;
                }
                let quote = quote_res.unwrap();
                let price = quote.quote.ask;
                let trade = EventType::TradeSuccess {
                    order: order.clone(),
                    price,
                };
                let ev = Event { event_type: trade };
                Some(ev)
            }
            OrderType::MarketSell => {
                let quote_res = prices.get(&order.symbol);
                if quote_res.is_none() {
                    return None;
                }
                let quote = quote_res.unwrap();
                let price = quote.quote.bid;
                let trade = EventType::TradeSuccess {
                    order: order.clone(),
                    price,
                };
                let ev = Event { event_type: trade };
                Some(ev)
            }
        }
    }
}

impl SimulatedBroker {
    pub fn new() -> SimulatedBroker {
        SimulatedBroker
    }
}
