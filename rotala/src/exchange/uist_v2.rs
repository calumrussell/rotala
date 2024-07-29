use std::collections::{HashMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::input::athena::{DateQuotes, Depth, Level};

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
    LimitBuy,
    LimitSell,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Order {
    pub order_type: OrderType,
    pub symbol: String,
    pub qty: f64,
    pub price: Option<f64>,
    pub recieved: i64,
}

impl Order {
    fn market(order_type: OrderType, symbol: impl Into<String>, shares: f64, now: i64) -> Self {
        Self {
            order_type,
            symbol: symbol.into(),
            qty: shares,
            price: None,
            recieved: now,
        }
    }

    fn delayed(
        order_type: OrderType,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
        now: i64,
    ) -> Self {
        Self {
            order_type,
            symbol: symbol.into(),
            qty: shares,
            price: Some(price),
            recieved: now,
        }
    }

    pub fn market_buy(symbol: impl Into<String>, shares: f64, now: i64) -> Self {
        Order::market(OrderType::MarketBuy, symbol, shares, now)
    }

    pub fn market_sell(symbol: impl Into<String>, shares: f64, now: i64) -> Self {
        Order::market(OrderType::MarketSell, symbol, shares, now)
    }

    pub fn limit_buy(symbol: impl Into<String>, shares: f64, price: f64, now: i64) -> Self {
        Order::delayed(OrderType::LimitBuy, symbol, shares, price, now)
    }

    pub fn limit_sell(symbol: impl Into<String>, shares: f64, price: f64, now: i64) -> Self {
        Order::delayed(OrderType::LimitSell, symbol, shares, price, now)
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

#[derive(Debug)]
pub struct UistV2 {
    orderbook: OrderBook,
    trade_log: Vec<Trade>,
    //This is cleared on every tick
    order_buffer: Vec<Order>,
}

impl UistV2 {
    pub fn new() -> Self {
        Self {
            orderbook: OrderBook::default(),
            trade_log: Vec::new(),
            order_buffer: Vec::new(),
        }
    }

    fn sort_order_buffer(&mut self) {
        self.order_buffer.sort_by(|a, _b| match a.order_type {
            OrderType::LimitSell | OrderType::MarketSell => std::cmp::Ordering::Less,
            _ => std::cmp::Ordering::Greater,
        })
    }

    pub fn insert_order(&mut self, order: Order) {
        // Orders are only inserted into the book when tick is called, this is to ensure proper
        // ordering of trades
        // This impacts order_id where an order X can come in before order X+1 but the latter can
        // have an order_id that is less than the former.
        self.order_buffer.push(order);
    }

    pub fn tick(&mut self, quotes: &DateQuotes, now: i64) -> (Vec<Trade>, Vec<Order>) {
        //To eliminate lookahead bias, we only insert new orders after we have executed any orders
        //that were on the stack first
        let executed_trades = self.orderbook.execute_orders(quotes, now);
        for executed_trade in &executed_trades {
            self.trade_log.push(executed_trade.clone());
        }

        self.sort_order_buffer();
        //TODO: remove this overhead, shouldn't need a clone here
        for order in self.order_buffer.iter() {
            self.orderbook.insert_order(order.clone());
        }

        let inserted_orders = std::mem::take(&mut self.order_buffer);
        (executed_trades, inserted_orders)
    }
}

// FillTracker is stored over the life of an execution cycle.
// New data structure is created so that we do not have to modify the underlying quotes that are
// passed to the execute_orders function. Orderbook is intended to be relatively pure and so needs
// to hold the minimum amount of data itself. Modifying underlying quotes would mean copies, which
// would get expensive.
struct FillTracker {
    inner: HashMap<String, HashMap<String, f64>>,
}

impl FillTracker {
    fn get_fill(&self, symbol: &str, level: &Level) -> f64 {
        //Can default to zero instead of None
        if let Some(fills) = self.inner.get(symbol) {
            let level_string = level.price.to_string();
            if let Some(val) = fills.get(&level_string) {
                return *val;
            }
        }
        0.0
    }

    fn insert_fill(&mut self, symbol: &str, level: &Level, filled: f64) {
        if !self.inner.contains_key(symbol) {
            self.inner
                .insert(symbol.to_string().clone(), HashMap::new());
        }

        let fills = self.inner.get_mut(symbol).unwrap();
        let level_string = level.price.to_string();

        fills
            .entry(level_string)
            .and_modify(|count| *count += filled)
            .or_insert(filled);
    }

    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub enum LatencyModel {
    None,
    FixedPeriod(i64),
}

impl LatencyModel {
    fn cmp_order(&self, now: i64, order: &Order) -> bool {
        match self {
            Self::None => true,
            Self::FixedPeriod(period) => order.recieved + period < now,
        }
    }
}

#[derive(Debug)]
pub struct OrderBook {
    inner: VecDeque<Order>,
    latency: LatencyModel,
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
            latency: LatencyModel::None,
        }
    }

    pub fn with_latency(latency: i64) -> Self {
        Self {
            inner: std::collections::VecDeque::new(),
            latency: LatencyModel::FixedPeriod(latency),
        }
    }

    pub fn insert_order(&mut self, order: Order) {
        self.inner.push_back(order.clone());
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn fill_order(
        depth: &Depth,
        order: &Order,
        is_buy: bool,
        price_check: f64,
        filled: &mut FillTracker,
    ) -> Vec<Trade> {
        let mut to_fill = order.qty;
        let mut trades = Vec::new();

        if is_buy {
            for ask in &depth.asks {
                if ask.price > price_check {
                    break;
                }

                let filled_size = filled.get_fill(&order.symbol, ask);
                let size = ask.size - filled_size;
                if size == 0.0 {
                    break;
                }

                let qty = if size >= to_fill { to_fill } else { size };
                to_fill -= qty;
                let trade = Trade {
                    symbol: order.symbol.clone(),
                    value: ask.price * order.qty,
                    quantity: qty,
                    date: depth.date,
                    typ: TradeType::Buy,
                };
                trades.push(trade);
                filled.insert_fill(&order.symbol, ask, qty);

                if to_fill == 0.0 {
                    break;
                }
            }
        } else {
            for bid in &depth.bids {
                if price_check > bid.price {
                    break;
                }

                let filled_size = filled.get_fill(&order.symbol, bid);
                let size = bid.size - filled_size;
                if size == 0.0 {
                    break;
                }

                let qty = if size >= to_fill { to_fill } else { size };
                to_fill -= qty;
                let trade = Trade {
                    symbol: order.symbol.clone(),
                    value: bid.price * order.qty,
                    quantity: qty,
                    date: depth.date,
                    typ: TradeType::Sell,
                };
                trades.push(trade);
                filled.insert_fill(&order.symbol, bid, qty);

                if to_fill == 0.0 {
                    break;
                }
            }
        }
        trades
    }

    pub fn execute_orders(
        &mut self,
        quotes: &crate::input::athena::DateQuotes,
        now: i64,
    ) -> Vec<Trade> {
        //Tracks liquidity that has been used at each level
        let mut filled: FillTracker = FillTracker::new();

        let mut trade_results = Vec::new();
        if self.is_empty() {
            return trade_results;
        }

        for order in self.inner.iter() {
            let security_id = &order.symbol;

            if !self.latency.cmp_order(now, order) {
                continue;
            }

            if let Some(depth) = quotes.get(security_id) {
                let mut trades = match order.order_type {
                    OrderType::MarketBuy => {
                        Self::fill_order(depth, order, true, f64::MAX, &mut filled)
                    }
                    OrderType::MarketSell => {
                        Self::fill_order(depth, order, false, f64::MIN, &mut filled)
                    }
                    OrderType::LimitBuy => {
                        Self::fill_order(depth, order, true, order.price.unwrap(), &mut filled)
                    }
                    OrderType::LimitSell => {
                        Self::fill_order(depth, order, false, order.price.unwrap(), &mut filled)
                    }
                };
                trade_results.append(&mut trades)
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
    fn test_that_buy_order_will_lift_all_volume_when_order_is_equal_to_depth_size() {
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
        let order = Order::market_buy("ABC", 100.0, 100);
        orderbook.insert_order(order);

        let res = orderbook.execute_orders(&quotes, 100);
        assert!(res.len() == 1);
        let trade = res.first().unwrap();
        assert!(trade.quantity == 100.00);
        assert!(trade.value / trade.quantity == 102.00);
    }

    #[test]
    fn test_that_sell_order_will_lift_all_volume_when_order_is_equal_to_depth_size() {
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
        let order = Order::market_sell("ABC", 100.0, 100);
        orderbook.insert_order(order);

        let res = orderbook.execute_orders(&quotes, 100);
        assert!(res.len() == 1);
        let trade = res.first().unwrap();
        assert!(trade.quantity == 100.00);
        assert!(trade.value / trade.quantity == 100.00);
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
        let order = Order::market_buy("ABC", 50.0, 100);
        orderbook.insert_order(order);

        let res = orderbook.execute_orders(&quotes, 100);
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
            price: 103.0,
            size: 20.0,
        };

        let mut depth = Depth::new(100, "ABC");
        depth.add_level(bid_level, crate::input::athena::Side::Bid);
        depth.add_level(ask_level, crate::input::athena::Side::Ask);
        depth.add_level(ask_level_1, crate::input::athena::Side::Ask);

        let mut quotes: DateQuotes = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();
        let order = Order::market_buy("ABC", 100.0, 100);
        orderbook.insert_order(order);

        let res = orderbook.execute_orders(&quotes, 100);
        assert!(res.len() == 2);
        let first_trade = res.first().unwrap();
        let second_trade = res.get(1).unwrap();

        println!("{:?}", first_trade);
        println!("{:?}", second_trade);
        assert!(first_trade.quantity == 80.0);
        assert!(second_trade.quantity == 20.0);
    }

    #[test]
    fn test_that_limit_buy_order_lifts_all_volume_when_price_is_good() {
        let bid_level = Level {
            price: 100.0,
            size: 100.0,
        };

        let ask_level = Level {
            price: 102.0,
            size: 80.0,
        };

        let ask_level_1 = Level {
            price: 103.0,
            size: 20.0,
        };

        let ask_level_2 = Level {
            price: 104.0,
            size: 20.0,
        };

        let mut depth = Depth::new(100, "ABC");
        depth.add_level(bid_level, crate::input::athena::Side::Bid);
        depth.add_level(ask_level, crate::input::athena::Side::Ask);
        depth.add_level(ask_level_1, crate::input::athena::Side::Ask);
        depth.add_level(ask_level_2, crate::input::athena::Side::Ask);

        let mut quotes: DateQuotes = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();
        let order = Order::limit_buy("ABC", 120.0, 103.00, 100);
        orderbook.insert_order(order);

        let res = orderbook.execute_orders(&quotes, 100);
        println!("{:?}", res);
        assert!(res.len() == 2);
        let first_trade = res.first().unwrap();
        let second_trade = res.get(1).unwrap();

        println!("{:?}", first_trade);
        println!("{:?}", second_trade);
        assert!(first_trade.quantity == 80.0);
        assert!(second_trade.quantity == 20.0);
    }

    #[test]
    fn test_that_limit_sell_order_lifts_all_volume_when_price_is_good() {
        let bid_level_0 = Level {
            price: 98.0,
            size: 20.0,
        };

        let bid_level_1 = Level {
            price: 99.0,
            size: 20.0,
        };

        let bid_level_2 = Level {
            price: 100.0,
            size: 80.0,
        };

        let ask_level = Level {
            price: 102.0,
            size: 80.0,
        };

        let mut depth = Depth::new(100, "ABC");
        depth.add_level(bid_level_0, crate::input::athena::Side::Bid);
        depth.add_level(bid_level_1, crate::input::athena::Side::Bid);
        depth.add_level(bid_level_2, crate::input::athena::Side::Bid);
        depth.add_level(ask_level, crate::input::athena::Side::Ask);

        let mut quotes: DateQuotes = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();
        let order = Order::limit_sell("ABC", 120.0, 99.00, 100);
        orderbook.insert_order(order);

        let res = orderbook.execute_orders(&quotes, 100);
        println!("{:?}", res);
        assert!(res.len() == 2);
        let first_trade = res.first().unwrap();
        let second_trade = res.get(1).unwrap();

        println!("{:?}", first_trade);
        println!("{:?}", second_trade);
        assert!(first_trade.quantity == 80.0);
        assert!(second_trade.quantity == 20.0);
    }

    #[test]
    fn test_that_repeated_orders_do_not_use_same_liquidty() {
        let bid_level = Level {
            price: 98.0,
            size: 20.0,
        };

        let ask_level = Level {
            price: 102.0,
            size: 20.0,
        };

        let mut depth = Depth::new(100, "ABC");
        depth.add_level(bid_level, crate::input::athena::Side::Bid);
        depth.add_level(ask_level, crate::input::athena::Side::Ask);

        let mut quotes: DateQuotes = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();
        let first_order = Order::limit_buy("ABC", 20.0, 103.00, 100);
        orderbook.insert_order(first_order);
        let second_order = Order::limit_buy("ABC", 20.0, 103.00, 100);
        orderbook.insert_order(second_order);

        let res = orderbook.execute_orders(&quotes, 100);
        println!("{:?}", res);
        assert!(res.len() == 1);
    }

    #[test]
    fn test_that_latency_model_filters_orders() {
        let bid_level = Level {
            price: 98.0,
            size: 20.0,
        };

        let ask_level = Level {
            price: 102.0,
            size: 20.0,
        };

        let mut depth = Depth::new(100, "ABC");
        depth.add_level(bid_level.clone(), crate::input::athena::Side::Bid);
        depth.add_level(ask_level.clone(), crate::input::athena::Side::Ask);

        let mut depth_101 = Depth::new(101, "ABC");
        depth_101.add_level(bid_level.clone(), crate::input::athena::Side::Bid);
        depth_101.add_level(ask_level.clone(), crate::input::athena::Side::Ask);

        let mut depth_102 = Depth::new(102, "ABC");
        depth_102.add_level(bid_level, crate::input::athena::Side::Bid);
        depth_102.add_level(ask_level, crate::input::athena::Side::Ask);

        let mut quotes: DateQuotes = HashMap::new();
        quotes.insert("ABC".to_string(), depth);
        quotes.insert("ABC".to_string(), depth_101);
        quotes.insert("ABC".to_string(), depth_102);

        let mut orderbook = OrderBook::with_latency(1);
        let order = Order::limit_buy("ABC", 20.0, 103.00, 100);
        orderbook.insert_order(order);

        let trades_100 = orderbook.execute_orders(&quotes, 100);
        let trades_101 = orderbook.execute_orders(&quotes, 101);
        let trades_102 = orderbook.execute_orders(&quotes, 102);
        assert!(trades_100.is_empty());
        assert!(trades_101.is_empty());
        assert!(trades_102.len() == 1);
    }
}
