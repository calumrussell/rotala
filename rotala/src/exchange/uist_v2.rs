use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum OrderType {
    MarketSell,
    MarketBuy,
    LimitBuy,
    LimitSell,
    Cancel,
    Modify,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Order {
    pub order_type: OrderType,
    pub symbol: String,
    pub qty: f64,
    pub price: Option<f64>,
    pub order_id_ref: Option<OrderId>,
}

impl Order {
    fn market(order_type: OrderType, symbol: impl Into<String>, shares: f64) -> Self {
        Self {
            order_type,
            symbol: symbol.into(),
            qty: shares,
            price: None,
            order_id_ref: None,
        }
    }

    fn delayed(order_type: OrderType, symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        Self {
            order_type,
            symbol: symbol.into(),
            qty: shares,
            price: Some(price),
            order_id_ref: None,
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

    pub fn modify_order(symbol: impl Into<String>, order_id: OrderId, qty_change: f64) -> Self {
        Self {
            order_id_ref: Some(order_id),
            order_type: OrderType::Modify,
            symbol: symbol.into(),
            price: None,
            qty: qty_change,
        }
    }

    pub fn cancel_order(symbol: impl Into<String>, order_id: OrderId) -> Self {
        Self {
            order_id_ref: Some(order_id),
            order_type: OrderType::Cancel,
            symbol: symbol.into(),
            price: None,
            qty: 0.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum OrderResultType {
    Buy,
    Sell,
    Modify,
    Cancel,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OrderResult {
    pub symbol: String,
    pub value: f64,
    pub quantity: f64,
    pub date: i64,
    pub typ: OrderResultType,
    pub order_id: OrderId,
}

#[derive(Debug)]
pub struct UistV2 {
    orderbook: OrderBook,
    order_result_log: Vec<OrderResult>,
    //This is cleared on every tick
    order_buffer: Vec<Order>,
}

impl UistV2 {
    pub fn new() -> Self {
        Self {
            orderbook: OrderBook::default(),
            order_result_log: Vec::new(),
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

    pub fn tick(&mut self, quotes: &DateDepth, now: i64) -> (Vec<OrderResult>, Vec<InnerOrder>) {
        //To eliminate lookahead bias, we only insert new orders after we have executed any orders
        //that were on the stack first
        let executed_trades = self.orderbook.execute_orders(quotes, now);
        for executed_trade in &executed_trades {
            self.order_result_log.push(executed_trade.clone());
        }
        let mut inserted_orders = Vec::new();

        self.sort_order_buffer();
        //TODO: remove this overhead, shouldn't need a clone here
        for order in self.order_buffer.iter() {
            let inner_order = self.orderbook.insert_order(order.clone(), now);
            inserted_orders.push(inner_order);
        }

        self.order_buffer.clear();
        (executed_trades, inserted_orders)
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
    pub order_id_ref: Option<OrderId>,
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
    inner: BTreeMap<OrderId, InnerOrder>,
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
            inner: BTreeMap::new(),
            latency: LatencyModel::None,
            last_order_id: 0,
        }
    }

    //Used for testing
    pub fn get_total_order_qty_by_symbol(&self, symbol: &str) -> f64 {
        let mut total = 0.0;
        for order in self.inner.values() {
            if order.symbol == symbol {
                total += order.qty
            }
        }
        total
    }

    pub fn with_latency(latency: i64) -> Self {
        Self {
            inner: BTreeMap::new(),
            latency: LatencyModel::FixedPeriod(latency),
            last_order_id: 0,
        }
    }

    pub fn insert_order(&mut self, order: Order, now: i64) -> InnerOrder {
        let inner_order = InnerOrder {
            recieved_timestamp: now,
            order_id: self.last_order_id,
            order_type: order.order_type,
            symbol: order.symbol.clone(),
            qty: order.qty,
            price: order.price,
            order_id_ref: order.order_id_ref,
        };

        self.inner.insert(self.last_order_id, inner_order.clone());
        self.last_order_id += 1;
        inner_order
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    // Only returns a single `OrderResult` but we return a `Vec` for empty condition
    fn cancel_order(
        now: i64,
        order_to_cancel: &InnerOrder,
        orderbook: &mut BTreeMap<OrderId, InnerOrder>,
    ) -> Vec<OrderResult> {
        let mut res = Vec::new();
        if orderbook
            .remove(&order_to_cancel.order_id_ref.unwrap())
            .is_some()
        {
            let order_result = OrderResult {
                symbol: order_to_cancel.symbol.clone(),
                value: 0.0,
                quantity: 0.0,
                date: now,
                typ: OrderResultType::Cancel,
                order_id: order_to_cancel.order_id,
            };
            res.push(order_result);
        }

        res
    }

    // Only returns a single `OrderResult` but we return a `Vec` for empty condition
    fn modify_order(
        now: i64,
        order_to_modify: &InnerOrder,
        orderbook: &mut BTreeMap<OrderId, InnerOrder>,
    ) -> Vec<OrderResult> {
        let mut res = Vec::new();

        if let Some(order) = orderbook.get_mut(&order_to_modify.order_id_ref.unwrap()) {
            let qty_change = order_to_modify.qty;

            if qty_change > 0.0 {
                order.qty += qty_change;
            } else {
                let qty_left = order.qty + qty_change;
                if qty_left > 0.0 {
                    order.qty += qty_change;
                } else {
                    // we are trying to remove more than the total number of shares
                    // left on the order so will assume user wants to cancel
                    orderbook.remove(&order_to_modify.order_id);
                }
            }

            let order_result = OrderResult {
                symbol: order_to_modify.symbol.clone(),
                value: 0.0,
                quantity: 0.0,
                date: now,
                typ: OrderResultType::Modify,
                order_id: order_to_modify.order_id,
            };
            res.push(order_result);
        }
        res
    }

    fn fill_order(
        depth: &Depth,
        order: &InnerOrder,
        is_buy: bool,
        price_check: f64,
        filled: &mut FillTracker,
    ) -> Vec<OrderResult> {
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
                let trade = OrderResult {
                    symbol: order.symbol.clone(),
                    value: ask.price * order.qty,
                    quantity: qty,
                    date: depth.date,
                    typ: OrderResultType::Buy,
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
                let trade = OrderResult {
                    symbol: order.symbol.clone(),
                    value: bid.price * order.qty,
                    quantity: qty,
                    date: depth.date,
                    typ: OrderResultType::Sell,
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
    ) -> Vec<OrderResult> {
        //Tracks liquidity that has been used at each level
        let mut filled: FillTracker = FillTracker::new();

        let mut trade_results = Vec::new();
        if self.is_empty() {
            return trade_results;
        }

        // Split out cancel and modifies, and then implement on a copy of orderbook
        let mut cancel_and_modify: Vec<InnerOrder> = Vec::new();
        let mut orders: BTreeMap<OrderId, InnerOrder> = BTreeMap::new();
        while let Some((order_id, order)) = self.inner.pop_first() {
            match order.order_type {
                OrderType::Cancel | OrderType::Modify => {
                    cancel_and_modify.push(order);
                }
                _ => {
                    orders.insert(order_id, order);
                }
            }
        }

        for order in cancel_and_modify {
            match order.order_type {
                OrderType::Cancel => {
                    let mut res = Self::cancel_order(now, &order, &mut orders);
                    if !res.is_empty() {
                        trade_results.append(&mut res);
                    }
                }
                OrderType::Modify => {
                    let mut res = Self::modify_order(now, &order, &mut orders);
                    if !res.is_empty() {
                        trade_results.append(&mut res);
                    }
                }
                _ => {}
            }
        }

        //TODO: really bad should be able to take somewhere?
        let mut unexecuted_orders = BTreeMap::new();
        for (order_id, order) in orders.iter() {
            let security_id = &order.symbol;

            if !self.latency.cmp_order(now, order) {
                unexecuted_orders.insert(*order_id, order.clone());
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
                    // There shouldn't be any cancel or modifies by this point
                    _ => vec![],
                };

                if trades.is_empty() {
                    unexecuted_orders.insert(*order_id, order.clone());
                }

                trade_results.append(&mut trades)
            } else {
                unexecuted_orders.insert(*order_id, order.clone());
            }
        }
        self.inner = unexecuted_orders;
        trade_results
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        exchange::uist_v2::{Order, OrderBook},
        input::athena::{DateDepth, Depth, Level},
    };

    fn quotes() -> DateDepth {
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
        quotes
    }

    fn quotes1() -> DateDepth {
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
        quotes
    }

    #[test]
    fn test_that_nonexistent_buy_order_cancel_produces_empty_result() {
        let quotes = quotes();
        let mut orderbook = OrderBook::new();
        orderbook.insert_order(Order::cancel_order("ABC", 10), 100);
        let res = orderbook.execute_orders(&quotes, 100);
        assert!(res.is_empty())
    }

    #[test]
    fn test_that_nonexistent_buy_order_modify_throws_error() {
        let quotes = quotes();
        let mut orderbook = OrderBook::new();
        orderbook.insert_order(Order::modify_order("ABC", 10, 100.0), 100);
        let res = orderbook.execute_orders(&quotes, 100);
        assert!(res.is_empty())
    }

    #[test]
    fn test_that_buy_order_can_be_cancelled_and_modified() {
        let quotes = quotes();

        let mut orderbook = OrderBook::new();
        let oid = orderbook
            .insert_order(Order::limit_buy("ABC", 100.0, 1.0), 100)
            .order_id;

        orderbook.insert_order(Order::cancel_order("ABC", oid), 100);
        let res = orderbook.execute_orders(&quotes, 100);
        println!("{:?}", res);
        assert!(res.len() == 1);

        let oid1 = orderbook
            .insert_order(Order::limit_buy("ABC", 200.0, 1.0), 100)
            .order_id;
        orderbook.insert_order(Order::modify_order("ABC", oid1, 100.0), 100);
        let res = orderbook.execute_orders(&quotes, 100);
        assert!(res.len() == 1);
    }

    #[test]
    fn test_that_buy_order_will_lift_all_volume_when_order_is_equal_to_depth_size() {
        let quotes = quotes();

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
        let quotes = quotes();

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
        let quotes = quotes();
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
        let quotes = quotes1();
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
        let quotes = quotes1();
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
