use std::{
    collections::{HashMap, VecDeque},
    fmt::Display,
};

use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::input::athena::{DateDepth, Depth, Level};

pub type OrderId = u64;

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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum OrderModification {
    CancelOrder(OrderId),
    ModifyOrder(OrderId, f64),
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

    fn delayed(order_type: OrderType, symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        Self {
            order_type,
            symbol: symbol.into(),
            qty: shares,
            price: Some(price),
        }
    }

    pub fn market_buy(symbol: impl Into<String>, shares: f64) -> Self {
        Order::market(OrderType::MarketBuy, symbol, shares)
    }

    pub fn market_sell(symbol: impl Into<String>, shares: f64) -> Self {
        Order::market(OrderType::MarketSell, symbol, shares)
    }

    pub fn limit_buy(symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        Order::delayed(OrderType::LimitBuy, symbol, shares, price)
    }

    pub fn limit_sell(symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        Order::delayed(OrderType::LimitSell, symbol, shares, price)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ModifyResultType {
    Modify,
    Cancel,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModifyResult {
    pub modify_type: ModifyResultType,
    pub order_id: OrderId,
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
    pub order_id: OrderId,
}

#[derive(Debug)]
pub struct UistV2 {
    orderbook: OrderBook,
    trade_log: Vec<Trade>,
    //This is cleared on every tick
    order_buffer: Vec<Order>,
    order_modification_buffer: Vec<OrderModification>,
}

impl UistV2 {
    pub fn new() -> Self {
        Self {
            orderbook: OrderBook::default(),
            trade_log: Vec::new(),
            order_buffer: Vec::new(),
            order_modification_buffer: Vec::new(),
        }
    }

    fn sort_order_buffer(&mut self) {
        self.order_buffer.sort_by(|a, _b| match a.order_type {
            OrderType::LimitSell | OrderType::MarketSell => std::cmp::Ordering::Less,
            _ => std::cmp::Ordering::Greater,
        })
    }

    pub fn modify_order(&mut self, order_id: OrderId, qty_change: f64) {
        let order_mod = OrderModification::ModifyOrder(order_id, qty_change);
        self.order_modification_buffer.push(order_mod);
    }

    pub fn cancel_order(&mut self, order_id: OrderId) {
        let order_mod = OrderModification::CancelOrder(order_id);
        self.order_modification_buffer.push(order_mod);
    }

    pub fn insert_order(&mut self, order: Order) {
        // Orders are only inserted into the book when tick is called, this is to ensure proper
        // ordering of trades
        // This impacts order_id where an order X can come in before order X+1 but the latter can
        // have an order_id that is less than the former.
        self.order_buffer.push(order);
    }

    pub fn tick(
        &mut self,
        quotes: &DateDepth,
        now: i64,
    ) -> (Vec<Trade>, Vec<InnerOrder>, Vec<ModifyResult>) {
        //To eliminate lookahead bias, we only insert new orders after we have executed any orders
        //that were on the stack first
        let executed_trades = self.orderbook.execute_orders(quotes, now);
        for executed_trade in &executed_trades {
            self.trade_log.push(executed_trade.clone());
        }
        let mut inserted_orders = Vec::new();

        self.sort_order_buffer();
        //TODO: remove this overhead, shouldn't need a clone here
        for order in self.order_buffer.iter() {
            let inner_order = self.orderbook.insert_order(order.clone(), now);
            inserted_orders.push(inner_order);
        }

        let mut modified_orders = Vec::new();
        for order_mod in &self.order_modification_buffer {
            let res = match order_mod {
                OrderModification::CancelOrder(order_id) => self.orderbook.cancel_order(*order_id),
                OrderModification::ModifyOrder(order_id, qty_change) => {
                    self.orderbook.modify_order(*order_id, *qty_change)
                }
            };

            //If we didn't succeed then we tried to modify an order that didn't exist so we just
            //ignore this totally as a no-op and move on
            if res.is_ok() {
                let modification_result_destructure = match order_mod {
                    OrderModification::CancelOrder(order_id) => {
                        (order_id, ModifyResultType::Cancel)
                    }
                    OrderModification::ModifyOrder(order_id, _qty_change) => {
                        (order_id, ModifyResultType::Modify)
                    }
                };

                let modification_result = ModifyResult {
                    order_id: *modification_result_destructure.0,
                    modify_type: modification_result_destructure.1,
                };
                modified_orders.push(modification_result);
            }
        }

        self.order_buffer.clear();
        (executed_trades, inserted_orders, modified_orders)
    }
}

impl Default for UistV2 {
    fn default() -> Self {
        Self::new()
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
    fn cmp_order(&self, now: i64, order: &InnerOrder) -> bool {
        match self {
            Self::None => true,
            Self::FixedPeriod(period) => order.recieved_timestamp + period < now,
        }
    }
}

//Representation of order used internally, this is sent back to clients.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InnerOrder {
    pub order_type: OrderType,
    pub symbol: String,
    pub qty: f64,
    pub price: Option<f64>,
    pub recieved_timestamp: i64,
    pub order_id: OrderId,
}

#[derive(Debug)]
pub enum OrderBookError {
    OrderIdNotFound,
}

impl Display for OrderBookError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "OrderBookError")
    }
}

impl std::error::Error for OrderBookError {}

#[derive(Debug)]
pub struct OrderBook {
    inner: VecDeque<InnerOrder>,
    latency: LatencyModel,
    last_order_id: u64,
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            inner: VecDeque::new(),
            latency: LatencyModel::None,
            last_order_id: 0,
        }
    }

    //Used for testing
    pub fn get_total_order_qty_by_symbol(&self, symbol: &str) -> f64 {
        let mut total = 0.0;
        for order in &self.inner {
            if order.symbol == symbol {
                total += order.qty
            }
        }
        total
    }

    pub fn with_latency(latency: i64) -> Self {
        Self {
            inner: std::collections::VecDeque::new(),
            latency: LatencyModel::FixedPeriod(latency),
            last_order_id: 0,
        }
    }

    pub fn insert_order(&mut self, order: Order, now: i64) -> InnerOrder {
        let inner_order = InnerOrder {
            recieved_timestamp: now,
            order_id: self.last_order_id,
            order_type: order.order_type,
            symbol: order.symbol,
            qty: order.qty,
            price: order.price,
        };

        self.last_order_id += 1;
        self.inner.push_back(inner_order.clone());
        inner_order
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn fill_order(
        depth: &Depth,
        order: &InnerOrder,
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
                    order_id: order.order_id,
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
                    order_id: order.order_id,
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
        quotes: &crate::input::athena::DateDepth,
        now: i64,
    ) -> Vec<Trade> {
        //Tracks liquidity that has been used at each level
        let mut filled: FillTracker = FillTracker::new();

        let mut trade_results = Vec::new();
        if self.is_empty() {
            return trade_results;
        }

        let mut new_inner: VecDeque<InnerOrder> = VecDeque::new();

        while !self.inner.is_empty() {
            let order = self.inner.pop_front().unwrap();
            let security_id = &order.symbol;

            if !self.latency.cmp_order(now, &order) {
                new_inner.push_back(order);
                continue;
            }

            if let Some(depth) = quotes.get(security_id) {
                let mut trades = match order.order_type {
                    OrderType::MarketBuy => {
                        Self::fill_order(depth, &order, true, f64::MAX, &mut filled)
                    }
                    OrderType::MarketSell => {
                        Self::fill_order(depth, &order, false, f64::MIN, &mut filled)
                    }
                    OrderType::LimitBuy => {
                        Self::fill_order(depth, &order, true, order.price.unwrap(), &mut filled)
                    }
                    OrderType::LimitSell => {
                        Self::fill_order(depth, &order, false, order.price.unwrap(), &mut filled)
                    }
                };

                if trades.is_empty() {
                    new_inner.push_back(order);
                }

                trade_results.append(&mut trades)
            } else {
                new_inner.push_back(order);
            }
        }
        self.inner = new_inner;
        trade_results
    }

    //Users will either want to change the quantity or cancel, so we can accept qty_change argument
    //and there is no other behaviour we need to support
    pub fn modify_order(&mut self, order_id: OrderId, qty_change: f64) -> Result<OrderId> {
        let mut position: Option<usize> = None;

        for (i, order) in self.inner.iter().enumerate() {
            if order.order_id == order_id {
                position = Some(i);
                break;
            }
        }

        match position {
            Some(pos) => {
                //Can unwrap safely because this is produced above
                let mut order_copied = self.inner.get(pos).unwrap().clone();

                let mut new_order_qty = order_copied.qty;

                if qty_change > 0.0 {
                    new_order_qty += qty_change
                } else {
                    let qty_left = order_copied.qty + qty_change;
                    if qty_left > 0.0 {
                        new_order_qty += qty_change
                    } else {
                        // We are trying to remove more than the total number of shares
                        // left on the order so will assume user wants to cancel
                        self.inner.remove(pos);
                    }
                }

                order_copied.qty = new_order_qty;
                self.inner.remove(pos);
                self.inner.insert(pos, order_copied);
                Ok(order_id)
            }
            None => Err(Error::new(OrderBookError::OrderIdNotFound)),
        }
    }

    pub fn cancel_order(&mut self, order_id: OrderId) -> Result<OrderId> {
        for (i, order) in self.inner.iter().enumerate() {
            if order.order_id == order_id {
                self.inner.remove(i);
                return Ok(order_id);
            }
        }
        Err(Error::new(OrderBookError::OrderIdNotFound))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        exchange::uist_v2::{Order, OrderBook},
        input::athena::{DateDepth, Depth, Level},
    };

    #[test]
    fn test_that_nonexistent_buy_order_cancel_throws_error() {
        let mut orderbook = OrderBook::new();
        let res = orderbook.cancel_order(10);
        assert!(res.is_err())
    }

    #[test]
    fn test_that_nonexistent_buy_order_modify_throws_error() {
        let mut orderbook = OrderBook::new();
        let res = orderbook.modify_order(10, 100.0);
        assert!(res.is_err())
    }

    #[test]
    fn test_that_buy_order_can_be_cancelled_and_modified() {
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

        let mut quotes: DateDepth = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();

        let order = Order::market_buy("ABC", 100.0);
        let oid = orderbook.insert_order(order, 100).order_id;
        let _res = orderbook.cancel_order(oid);
        assert!(orderbook.get_total_order_qty_by_symbol("ABC") == 0.0);

        let order1 = Order::market_buy("ABC", 200.0);
        let oid1 = orderbook.insert_order(order1, 100).order_id;
        let _res1 = orderbook.modify_order(oid1, 100.0);
        assert!(orderbook.get_total_order_qty_by_symbol("ABC") == 300.0);
    }

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

        let mut quotes: DateDepth = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();
        let order = Order::market_buy("ABC", 100.0);
        orderbook.insert_order(order, 100);

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

        let mut quotes: DateDepth = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();
        let order = Order::market_sell("ABC", 100.0);
        orderbook.insert_order(order, 100);

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

        let mut quotes: DateDepth = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();
        let order = Order::market_buy("ABC", 50.0);
        orderbook.insert_order(order, 100);

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

        let mut quotes: DateDepth = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();
        let order = Order::market_buy("ABC", 100.0);
        orderbook.insert_order(order, 100);

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

        let mut quotes: DateDepth = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();
        let order = Order::limit_buy("ABC", 120.0, 103.00);
        orderbook.insert_order(order, 100);

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

        let mut quotes: DateDepth = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();
        let order = Order::limit_sell("ABC", 120.0, 99.00);
        orderbook.insert_order(order, 100);

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

        let mut quotes: DateDepth = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();
        let first_order = Order::limit_buy("ABC", 20.0, 103.00);
        orderbook.insert_order(first_order, 100);
        let second_order = Order::limit_buy("ABC", 20.0, 103.00);
        orderbook.insert_order(second_order, 100);

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

        let mut quotes: DateDepth = HashMap::new();
        quotes.insert("ABC".to_string(), depth);
        quotes.insert("ABC".to_string(), depth_101);
        quotes.insert("ABC".to_string(), depth_102);

        let mut orderbook = OrderBook::with_latency(1);
        let order = Order::limit_buy("ABC", 20.0, 103.00);
        orderbook.insert_order(order, 100);

        let trades_100 = orderbook.execute_orders(&quotes, 100);
        let trades_101 = orderbook.execute_orders(&quotes, 101);
        let trades_102 = orderbook.execute_orders(&quotes, 102);

        println!("{:?}", trades_101);

        assert!(trades_100.is_empty());
        assert!(trades_101.is_empty());
        assert!(trades_102.len() == 1);
    }

    #[test]
    fn test_that_orderbook_clears_after_execution() {
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

        let mut quotes: DateDepth = HashMap::new();
        quotes.insert("ABC".to_string(), depth);

        let mut orderbook = OrderBook::new();
        let order = Order::market_buy("ABC", 20.0);
        orderbook.insert_order(order, 100);
        let trades = orderbook.execute_orders(&quotes, 100);

        let trades1 = orderbook.execute_orders(&quotes, 101);

        assert!(trades.len() == 1);
        assert!(trades1.is_empty());
    }

    #[test]
    fn test_that_order_id_is_incrementing_and_unique() {
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

        let mut quotes: DateDepth = HashMap::new();
        quotes.insert("ABC".to_string(), depth);
        quotes.insert("ABC".to_string(), depth_101);
        quotes.insert("ABC".to_string(), depth_102);

        let mut orderbook = OrderBook::new();
        let order = Order::limit_buy("ABC", 20.0, 103.00);
        let order1 = Order::limit_buy("ABC", 20.0, 103.00);
        let order2 = Order::limit_buy("ABC", 20.0, 103.00);

        let res = orderbook.insert_order(order, 100);
        let res1 = orderbook.insert_order(order1, 100);
        let _ = orderbook.execute_orders(&quotes, 100);

        let res2 = orderbook.insert_order(order2, 101);
        let _ = orderbook.execute_orders(&quotes, 101);

        assert!(res.order_id == 0);
        assert!(res1.order_id == 1);
        assert!(res2.order_id == 2);
    }
}
