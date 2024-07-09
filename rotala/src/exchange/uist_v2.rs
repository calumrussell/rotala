use std::collections::VecDeque;

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

impl Order {
    fn market(order_type: OrderType, symbol: impl Into<String>, shares: f64) -> Self {
        Self {
            order_type,
            symbol: symbol.into(),
            qty: shares,
            price: None,
        }
    }

    pub fn market_buy(symbol: impl Into<String>, shares: f64) -> Self {
        Order::market(OrderType::MarketBuy, symbol, shares)
    }

    pub fn market_sell(symbol: impl Into<String>, shares: f64) -> Self {
        Order::market(OrderType::MarketSell, symbol, shares)
    }
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
        }
    }

    pub fn insert_order(&mut self, order: Order) {
        self.inner.push_back(order.clone());
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn fill_order(depth: &Depth, order: &Order, is_buy: bool) -> Vec<Trade> {
        let mut to_fill = order.qty;
        let mut trades = Vec::new();

        if is_buy {
            for ask in &depth.asks {
                let qty = if ask.size >= to_fill {
                    to_fill
                } else {
                    ask.size
                };
                to_fill -= qty;
                let trade = Trade {
                    symbol: order.symbol.clone(),
                    value: ask.price * order.qty,
                    quantity: qty,
                    date: depth.date,
                    typ: TradeType::Buy,
                };
                trades.push(trade);
                if to_fill == 0.0 {
                    break;
                }
            }
        }
        trades
    }

    fn trade_loop(depth: &Depth, order: &Order) -> Option<Vec<Trade>> {
        let res = match order.order_type {
            OrderType::MarketBuy => Self::fill_order(depth, order, true),
            OrderType::MarketSell => Self::fill_order(depth, order, false),
        };
        Some(res)
    }

    pub fn execute_orders(&mut self, quotes: crate::input::athena::DateQuotes) -> Vec<Trade> {
        let mut trade_results = Vec::new();
        if self.is_empty() {
            return trade_results;
        }

        for order in self.inner.iter() {
            let security_id = &order.symbol;

            if let Some(depth) = quotes.get(security_id) {
                if let Some(mut trade) = Self::trade_loop(depth, order) {
                    trade_results.append(&mut trade);
                }
            }
        }
        trade_results
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        exchange::uist_v2::{Order, OrderBook},
        input::athena::{DateQuotes, Depth, Level},
    };

    #[test]
    fn test_that_order_will_lift_all_volume_when_order_is_equal_to_depth_size() {
        let bid_level = Level {
            price: 100.0,
            size: 100.0,
        };

        let ask_level = Level {
            price: 102.0,
            size: 100.0,
        };

        let mut depth = Depth::new(100, "ABC");
        depth.add_level(bid_level, crate::input::athena::Side::Bid);
        depth.add_level(ask_level, crate::input::athena::Side::Ask);

        let mut quotes: DateQuotes = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();
        let order = Order::market_buy("ABC", 100.0);
        orderbook.insert_order(order);

        let res = orderbook.execute_orders(quotes);
        assert!(res.len() == 1);
        let trade = res.first().unwrap();
        assert!(trade.quantity == 100.00);
        assert!(trade.value / trade.quantity == 102.00);
    }

    #[test]
    fn test_that_order_will_lift_order_qty_when_order_is_less_than_depth_size() {
        let bid_level = Level {
            price: 100.0,
            size: 100.0,
        };

        let ask_level = Level {
            price: 102.0,
            size: 100.0,
        };

        let mut depth = Depth::new(100, "ABC");
        depth.add_level(bid_level, crate::input::athena::Side::Bid);
        depth.add_level(ask_level, crate::input::athena::Side::Ask);

        let mut quotes: DateQuotes = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();
        let order = Order::market_buy("ABC", 50.0);
        orderbook.insert_order(order);

        let res = orderbook.execute_orders(quotes);
        assert!(res.len() == 1);
        let trade = res.first().unwrap();
        assert!(trade.quantity == 50.00);
        assert!(trade.value / trade.quantity == 102.00);
    }

    #[test]
    fn test_that_order_will_lift_qty_from_other_levels_when_price_is_good() {
        let bid_level = Level {
            price: 100.0,
            size: 100.0,
        };

        let ask_level = Level {
            price: 102.0,
            size: 80.0,
        };

        let ask_level_1 = Level {
            price: 102.0,
            size: 20.0,
        };

        let mut depth = Depth::new(100, "ABC");
        depth.add_level(bid_level, crate::input::athena::Side::Bid);
        depth.add_level(ask_level, crate::input::athena::Side::Ask);
        depth.add_level(ask_level_1, crate::input::athena::Side::Ask);

        let mut quotes: DateQuotes = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();
        let order = Order::market_buy("ABC", 100.0);
        orderbook.insert_order(order);

        let res = orderbook.execute_orders(quotes);
        assert!(res.len() == 2);
        let first_trade = res.first().unwrap();
        let second_trade = res.get(1).unwrap();

        println!("{:?}", first_trade);
        println!("{:?}", second_trade);
        assert!(first_trade.quantity == 80.0);
        assert!(second_trade.quantity == 20.0);
    }
}
