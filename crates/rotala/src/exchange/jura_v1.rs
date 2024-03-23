use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::{
    clock::Clock,
    input::penelope::{Penelope, PenelopeQuote},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct JuraQuote {
    bid: f64,
    ask: f64,
    date: i64,
    symbol: String,
}

impl PenelopeQuote for JuraQuote {
    fn get_ask(&self) -> f64 {
        self.ask
    }

    fn get_bid(&self) -> f64 {
        self.bid
    }

    fn get_date(&self) -> i64 {
        self.date
    }

    fn get_symbol(&self) -> String {
        self.symbol.clone()
    }

    fn create(bid: f64, ask: f64, date: i64, symbol: String) -> Self {
        Self {
            bid,
            ask,
            date,
            symbol,
        }
    }
}

pub trait JuraSource {
    fn get_quote(&self, date: &i64, security: &u64) -> Option<JuraQuote>;
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Side {
    Ask,
    Bid,
}

impl From<Side> for String {
    fn from(value: Side) -> Self {
        match value {
            Side::Bid => "B".to_string(),
            Side::Ask => "A".to_string(),
        }
    }
}

/// HL is a future exchanges so assumes some of the functions of a broker, this means that
/// functions that report on the client's overall position won't be implemented at this stage.
/// * closed_pnl, unimplemented because the exchange does not keep track of client pnl
/// * dir, unimplemented as this appears to track the overall position in a coin, will always
/// be set to false
/// * crossed, this is unclear and may relate to margin or the execution of previous trades, this
/// will always be set to false
/// * hash, will always be an empty string, as HL is on-chain a transaction hash is produced but
/// won't be in a test env, always set to false
/// * start_position, unimplemented as this relates to overall position which is untracked, will
/// always be set to false
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Fill {
    closed_pnl: String,
    coin: String,
    crossed: bool,
    dir: bool,
    hash: bool,
    oid: u64,
    px: String,
    side: String,
    start_position: bool,
    sz: String,
    time: i64,
}

pub type OrderId = u64;

#[derive(Clone, Debug, Deserialize, Serialize)]
enum TimeInForce {
    Alo,
    Ioc,
    Gtc,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LimitOrder {
    tif: TimeInForce,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum TriggerType {
    Tp,
    Sl,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TriggerOrder {
    // Differs from limit_px as trigger_px is the price that triggers the order, limit_px is the
    // price that the user wants to trade at but is subject to the same slippage limitations
    // For some reason this is a number but limit_px is a String?
    trigger_px: f64,
    // If this is true then the order will execute immediately with max slippage of 10%
    is_market: bool,
    tpsl: TriggerType,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum OrderType {
    Limit(LimitOrder),
    Trigger(TriggerOrder),
}

/// The assumed function of the Hyperliquid API is followed as far as possible. A major area of
/// uncertainty in the API docs concerned what the exchange does when it receives an order that
/// has some properties set like a market order and some set like a limit order. The assumption
/// made throughout the implementation is that the is_market field, set on [TriggerOrder]
/// , determines fully whether an order is a market order and everything else is limit.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Order {
    asset: u64,
    is_buy: bool,
    limit_px: String,
    sz: String,
    reduce_only: bool,
    //This is client order id, need to check whether test impl should reasonably use this.
    cloid: Option<String>,
    order_type: OrderType,
}

impl Order {
    pub fn market_buy(asset: u64, sz: &str, price: &str) -> Self {
        Self {
            asset,
            is_buy: true,
            limit_px: price.to_string(),
            sz: sz.to_string(),
            reduce_only: false,
            cloid: None,
            order_type: OrderType::Limit(LimitOrder {
                tif: TimeInForce::Ioc,
            }),
        }
    }

    pub fn market_sell(
        asset: impl Into<u64>,
        sz: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        Self {
            asset: asset.into(),
            is_buy: false,
            limit_px: price.into(),
            sz: sz.into(),
            reduce_only: false,
            cloid: None,
            order_type: OrderType::Limit(LimitOrder {
                tif: TimeInForce::Ioc,
            }),
        }
    }

    pub fn limit_buy(
        asset: impl Into<u64>,
        sz: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        Self {
            asset: asset.into(),
            is_buy: true,
            limit_px: price.into(),
            sz: sz.into(),
            reduce_only: false,
            cloid: None,
            order_type: OrderType::Limit(LimitOrder {
                tif: TimeInForce::Gtc,
            }),
        }
    }

    pub fn limit_sell(
        asset: impl Into<u64>,
        sz: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        Self {
            asset: asset.into(),
            is_buy: false,
            limit_px: price.into(),
            sz: sz.into(),
            reduce_only: false,
            cloid: None,
            order_type: OrderType::Limit(LimitOrder {
                tif: TimeInForce::Gtc,
            }),
        }
    }

    pub fn stop_buy(
        asset: impl Into<u64>,
        sz: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        // It is possible for a stop to use a different trigger price, we guard against this with
        // the default order type because it is unexpected behaviour in most applications.
        let copy = price.into();
        let to_f64 = copy.parse::<f64>().unwrap();
        Self {
            asset: asset.into(),
            is_buy: true,
            limit_px: copy.clone(),
            sz: sz.into(),
            reduce_only: false,
            cloid: None,
            order_type: OrderType::Trigger(TriggerOrder {
                trigger_px: to_f64,
                is_market: true,
                tpsl: TriggerType::Sl,
            }),
        }
    }

    pub fn stop_sell(
        asset: impl Into<u64>,
        sz: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        // It is possible for a stop to use a different trigger price, we guard against this with
        // the default order type because it is unexpected behaviour in most applications.
        let copy = price.into();
        let to_f64 = copy.parse::<f64>().unwrap();
        Self {
            asset: asset.into(),
            is_buy: false,
            limit_px: copy.clone(),
            sz: sz.into(),
            reduce_only: false,
            cloid: None,
            order_type: OrderType::Trigger(TriggerOrder {
                trigger_px: to_f64,
                is_market: true,
                tpsl: TriggerType::Sl,
            }),
        }
    }

    pub fn takeprofit_buy(
        asset: impl Into<u64>,
        sz: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        // It is possible for a stop to use a different trigger price, we guard against this with
        // the default order type because it is unexpected behaviour in most applications.
        let copy = price.into();
        let to_f64 = copy.parse::<f64>().unwrap();
        Self {
            asset: asset.into(),
            is_buy: true,
            limit_px: copy.clone(),
            sz: sz.into(),
            reduce_only: false,
            cloid: None,
            order_type: OrderType::Trigger(TriggerOrder {
                trigger_px: to_f64,
                is_market: true,
                tpsl: TriggerType::Tp,
            }),
        }
    }

    pub fn takeprofit_sell(
        asset: impl Into<u64>,
        sz: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        // It is possible for a stop to use a different trigger price, we guard against this with
        // the default order type because it is unexpected behaviour in most applications.
        let copy = price.into();
        let to_f64 = copy.parse::<f64>().unwrap();
        Self {
            asset: asset.into(),
            is_buy: false,
            limit_px: copy.clone(),
            sz: sz.into(),
            reduce_only: false,
            cloid: None,
            order_type: OrderType::Trigger(TriggerOrder {
                trigger_px: to_f64,
                is_market: true,
                tpsl: TriggerType::Tp,
            }),
        }
    }
}

#[derive(Debug)]
struct InnerOrder {
    pub order_id: OrderId,
    pub order: Order,
    pub attempted_execution: bool,
}

impl InnerOrder {
    pub fn get_shares(&self) -> f64 {
        // We unwrap immediately because we can't continue if the client is passing incorrectly
        // sized orders
        str::parse::<f64>(&self.order.sz).unwrap()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InitMessage {
    pub start: i64,
    pub frequency: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InfoMessage {
    pub version: String,
    pub dataset: String,
}

impl InfoMessage {
    fn v1(dataset: String) -> InfoMessage {
        InfoMessage {
            version: "1.0".to_string(),
            dataset,
        }
    }
}

#[derive(Debug)]
pub struct JuraV1 {
    dataset: String,
    clock: Clock,
    price_source: Penelope<JuraQuote>,
    orderbook: OrderBook,
    trade_log: Vec<Fill>,
    //This is cleared on every tick
    order_buffer: Vec<Order>,
}

impl JuraV1 {
    pub fn from_binance() -> Self {
        let (penelope, clock) = Penelope::from_binance();
        Self::new(clock, penelope, "BINANCE")
    }

    pub fn new(clock: Clock, price_source: Penelope<JuraQuote>, dataset: &str) -> Self {
        Self {
            dataset: dataset.into(),
            clock,
            price_source,
            orderbook: OrderBook::default(),
            trade_log: Vec::new(),
            order_buffer: Vec::new(),
        }
    }

    pub fn info(&self) -> InfoMessage {
        InfoMessage::v1(self.dataset.clone())
    }

    pub fn init(&self) -> InitMessage {
        InitMessage {
            start: *self.clock.now(),
            frequency: self.clock.frequency().clone().into(),
        }
    }

    fn sort_order_buffer(&mut self) {
        self.order_buffer.sort_by(|a, _b| {
            if a.is_buy {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            }
        })
    }

    pub fn fetch_quotes(&self) -> Vec<JuraQuote> {
        if let Some(quotes) = self.price_source.get_quotes(&self.clock.now()) {
            return quotes;
        }
        vec![]
    }

    pub fn insert_order(&mut self, order: Order) {
        // Orders are only inserted into the book when tick is called, this is to ensure proper
        // ordering of trades
        // This impacts order_id where an order X can come in before order X+1 but the latter can
        // have an order_id that is less than the former.
        self.order_buffer.push(order);
    }

    pub fn delete_order(&mut self, asset: u64, order_id: u64) {
        self.orderbook.delete_order(asset, order_id);
    }

    pub fn tick(&mut self) -> (bool, Vec<Fill>, Vec<Order>, Vec<u64>) {
        //To eliminate lookahead bias, we only start executing orders on the next
        //tick.
        self.clock.tick();
        let now = self.clock.now();

        self.sort_order_buffer();
        for order in self.order_buffer.iter_mut() {
            self.orderbook.insert_order(now.into(), order.clone());
        }

        let (fills, triggered_order_ids) = self.orderbook.execute_orders(*now, &self.price_source);
        for fill in &fills {
            self.trade_log.push(fill.clone());
        }
        let inserted_orders = std::mem::take(&mut self.order_buffer);
        (
            self.clock.has_next(),
            fills,
            inserted_orders,
            triggered_order_ids,
        )
    }
}

/// OrderBook is an implementation of the Hyperliquid API running against a local server. This allows
/// testing of strategies using the same API/order types/etc.
///
/// Hyperliquid is a derivatives exchange. In order to simplify the implementation it is assumed
/// that everything is cash/no margin/no leverage.
///
/// Hyperliquid has two order types: limit and trigger.
///
/// Limit orders have various [TimeInForce] settings, we currently only support [TimeInForce::Ioc]
/// and [TimeInForce::Gtc]. This is roughly equivalent to a market order that will execute on the
/// next tick after entry with maximum slippage of 10%. Slippage is constant in this
/// implementation as this is the default setting in production. If this doesn't execute on the
/// next tick then it is cancelled.
///
/// Trigger orders are orders that turn into Limit orders when a trigger has been hit. The
/// trigger_px and limit_px are distinct so this works slightly differently to a normal TP/SL
/// order. If the trigger_px and limit_px are the same, it is possible for price to gap down
/// through your order such that it is never executed.
///
/// The latency paid by this order in production is unclear. The assumption made in this
/// implementation is that it is impossible for orders to be queued instanteously on a on-chain
/// exchange. So when an order triggers, the triggered orders are queued onto the next tick.
/// These are, however, added onto the front of the queue so will have a queue advantage over
/// order inserted onto the next_tick.
///
/// When an order is triggered, is_market is used to determine whether the [TimeInForce] is
/// [TimeInForce::Ioc] (if true) or [TimeInForce::Gtc] (if false).
///
/// After a trade executes a fill is returned to the user, the data returned is substantially
/// different to the Hyperliquid API due to Hyperliquid performing functions like margin.
/// The differences are documented in [Fill].
#[derive(Debug)]
pub struct OrderBook {
    inner: VecDeque<InnerOrder>,
    last_inserted: u64,
    slippage: f64,
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
            last_inserted: 0,
            slippage: 0.1,
        }
    }

    /// This method runs in O(N) because the underlying representation is [VecDeque]. Clients
    /// should be performing synchronization themselves so if you are calling this within main
    /// trade loop then you are doing something wrong. This is included for testing.
    pub fn get_order(&self, order_id: OrderId) -> Option<Order> {
        for order in self.inner.iter() {
            if order_id.eq(&order.order_id) {
                return Some(order.order.clone());
            }
        }
        None
    }

    // Hyperliquid returns an error if we try to cancel a non-existent order
    pub fn delete_order(&mut self, asset: u64, order_id: u64) -> bool {
        let mut delete_position: Option<usize> = None;
        for (position, order) in self.inner.iter().enumerate() {
            if order_id == order.order_id && asset == order.order.asset {
                delete_position = Some(position);
                break;
            }
        }
        if let Some(position) = delete_position {
            self.inner.remove(position);
            return true;
        }
        false
    }

    // Hyperliquid immediately returns an oid to the user whether the order is resting or filled on
    // the next tick. Because we need to guard against lookahead bias, we cannot execute immediately
    // but we have to return order id here.
    pub fn insert_order(&mut self, _date: i64, order: Order) -> OrderId {
        let order_id = self.last_inserted;
        // We assume that orders are received instaneously.
        // Latency can be added here when this is implemented.
        let inner_order = InnerOrder {
            order_id,
            order,
            attempted_execution: false,
        };
        self.inner.push_back(inner_order);
        self.last_inserted += 1;
        order_id
    }

    fn execute_buy(quote: JuraQuote, order: &InnerOrder, date: i64) -> Fill {
        let trade_price = quote.get_ask();
        Fill {
            closed_pnl: "0.0".to_string(),
            coin: order.order.asset.to_string(),
            crossed: false,
            dir: false,
            hash: false,
            oid: order.order_id,
            px: trade_price.to_string(),
            side: Side::Ask.into(),
            start_position: false,
            sz: order.get_shares().to_string(),
            time: date,
        }
    }

    fn execute_sell(quote: JuraQuote, order: &InnerOrder, date: i64) -> Fill {
        let trade_price = quote.get_bid();
        Fill {
            closed_pnl: "0.0".to_string(),
            coin: order.order.asset.to_string(),
            crossed: false,
            dir: false,
            hash: false,
            oid: order.order_id,
            px: trade_price.to_string(),
            side: Side::Bid.into(),
            start_position: false,
            sz: order.get_shares().to_string(),
            time: date,
        }
    }

    fn create_trigger(order: &InnerOrder, tif: TimeInForce) -> Order {
        Order {
            asset: order.order.asset,
            is_buy: order.order.is_buy,
            limit_px: order.order.limit_px.clone(),
            sz: order.order.sz.clone(),
            reduce_only: order.order.reduce_only,
            cloid: order.order.cloid.clone(),
            order_type: OrderType::Limit(LimitOrder { tif }),
        }
    }

    fn create_gtc_trigger(order: &InnerOrder) -> Order {
        Self::create_trigger(order, TimeInForce::Gtc)
    }

    fn create_ioc_trigger(order: &InnerOrder) -> Order {
        Self::create_trigger(order, TimeInForce::Ioc)
    }

    pub fn execute_orders(
        &mut self,
        date: i64,
        source: &impl JuraSource,
    ) -> (Vec<Fill>, Vec<OrderId>) {
        let mut fills: Vec<Fill> = Vec::new();
        let mut should_delete: Vec<(u64, u64)> = Vec::new();
        // HyperLiquid execution can trigger more orders, we don't execute these immediately.
        let mut should_insert: Vec<Order> = Vec::new();

        // We have to have a mutable reference so we can update attempted_execution
        for order in self.inner.iter_mut() {
            let symbol = order.order.asset;
            if let Some(quote) = source.get_quote(&date, &symbol.clone()) {
                let result = match &order.order.order_type {
                    OrderType::Limit(limit) => {
                        // A market order is a limit order with Ioc time-in-force. The px parameter
                        // on the order is taken from the order and seems to be used to calculate
                        // maximum slippage tolerated on the order.
                        // Market order code in Python SDK:
                        // https://github.com/hyperliquid-dex/hyperliquid-python-sdk/blob/67864cf979d3bbea2e964a99ecc0a1effb7bb911/hyperliquid/exchange.py#L209
                        match limit.tif {
                            // Don't support Alo TimeInForce
                            TimeInForce::Ioc => {
                                // Market orders can only be executed on the next time step
                                if order.attempted_execution {
                                    // We have tried to execute this before, return nothing
                                    should_delete.push((order.order.asset, order.order_id));
                                    None
                                } else {
                                    // For a market order, the limit price represents the maximum amount
                                    // of slippage tolerated by the client

                                    // We unwrap here because if the client is sending us bad prices
                                    // then we need to stop execution
                                    let price = str::parse::<f64>(&order.order.limit_px).unwrap();
                                    order.attempted_execution = true;
                                    if order.order.is_buy {
                                        if price * (1.0 + self.slippage) >= quote.get_ask() {
                                            should_delete.push((order.order.asset, order.order_id));
                                            Some(Self::execute_buy(quote, order, date))
                                        } else {
                                            None
                                        }
                                    } else if price * (1.0 - self.slippage) <= quote.get_bid() {
                                        should_delete.push((order.order.asset, order.order_id));
                                        Some(Self::execute_sell(quote, order, date))
                                    } else {
                                        None
                                    }
                                }
                            }
                            TimeInForce::Gtc => {
                                let price = str::parse::<f64>(&order.order.limit_px).unwrap();
                                if order.order.is_buy {
                                    if price >= quote.get_ask() {
                                        should_delete.push((order.order.asset, order.order_id));
                                        Some(Self::execute_buy(quote, order, date))
                                    } else {
                                        None
                                    }
                                } else if price <= quote.get_bid() {
                                    should_delete.push((order.order.asset, order.order_id));
                                    Some(Self::execute_sell(quote, order, date))
                                } else {
                                    None
                                }
                            }
                            _ => unimplemented!(),
                        }
                    }
                    OrderType::Trigger(trigger) => {
                        // If we trigger a market order, execute it here. If the trigger is for a
                        // limit order then we create another order add it to the queue and return
                        // the order_id to the client, execution cannot be immediate.

                        // TP/SL market orders have slippage of 10%
                        // If the market price falls below trigger price of stop loss purchase then it
                        // is triggered
                        // https://hyperliquid.gitbook.io/hyperliquid-docs/trading/take-profit-and-stop-loss-orders-tp-sl
                        match trigger.tpsl {
                            TriggerType::Sl => {
                                // Closing a short as price goes up
                                if order.order.is_buy {
                                    if quote.get_ask() >= trigger.trigger_px {
                                        if trigger.is_market {
                                            should_insert.push(Self::create_ioc_trigger(order));
                                        } else {
                                            should_insert.push(Self::create_gtc_trigger(order));
                                        }
                                        should_delete.push((order.order.asset, order.order_id));
                                        None
                                    } else {
                                        None
                                    }
                                } else {
                                    // Closing a long as price goes down
                                    if quote.get_bid() >= trigger.trigger_px {
                                        if trigger.is_market {
                                            should_insert.push(Self::create_ioc_trigger(order));
                                        } else {
                                            should_insert.push(Self::create_gtc_trigger(order));
                                        }
                                        should_delete.push((order.order.asset, order.order_id));
                                        None
                                    } else {
                                        None
                                    }
                                }
                            }
                            TriggerType::Tp => {
                                // Closing a short as price goes down
                                if order.order.is_buy {
                                    if quote.get_ask() <= trigger.trigger_px {
                                        if trigger.is_market {
                                            should_insert.push(Self::create_ioc_trigger(order))
                                        } else {
                                            should_insert.push(Self::create_gtc_trigger(order))
                                        }
                                        should_delete.push((order.order.asset, order.order_id));
                                        None
                                    } else {
                                        None
                                    }
                                } else {
                                    // Closing a long as price goes up
                                    if quote.get_bid() <= trigger.trigger_px {
                                        if trigger.is_market {
                                            should_insert.push(Self::create_ioc_trigger(order))
                                        } else {
                                            should_insert.push(Self::create_gtc_trigger(order))
                                        }
                                        should_delete.push((order.order.asset, order.order_id));
                                        None
                                    } else {
                                        None
                                    }
                                }
                            }
                        }
                    }
                };

                if let Some(trade) = &result {
                    fills.push(trade.clone());
                }
            }
        }

        for (asset, order_id) in should_delete {
            self.delete_order(asset, order_id);
        }

        let mut triggered_order_ids = Vec::new();

        for order in should_insert {
            triggered_order_ids.push(self.insert_order(date, order));
        }
        (fills, triggered_order_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::{JuraQuote, JuraV1, Order, OrderBook};
    use crate::clock::{Clock, Frequency};
    use crate::exchange::jura_v1::Side;
    use crate::input::penelope::Penelope;
    use crate::input::penelope::PenelopeBuilder;

    fn setup_orderbook() -> (Clock, Penelope<JuraQuote>) {
        let mut price_source_builder = PenelopeBuilder::new();
        price_source_builder.add_quote(101.0, 102.00, 100, "0".to_string());
        price_source_builder.add_quote(102.0, 103.00, 101, "0".to_string());
        price_source_builder.add_quote(105.0, 106.00, 102, "0".to_string());
        price_source_builder.add_quote(99.0, 100.00, 103, "0".to_string());

        let (penelope, clock) = price_source_builder.build_with_frequency(Frequency::Second);
        (clock, penelope)
    }

    #[test]
    fn test_that_buy_market_ioc_executes() {
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let order = Order {
            asset: 0,
            is_buy: true,
            limit_px: "102.00".to_string(),
            sz: "100.0".to_string(),
            reduce_only: false,
            cloid: None,
            order_type: super::OrderType::Limit(super::LimitOrder {
                tif: super::TimeInForce::Ioc,
            }),
        };
        orderbook.insert_order(100, order);
        let mut executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.0.len(), 1);

        let trade = executed.0.pop().unwrap();
        //Trade executes at 100 so trade price should be 102
        assert_eq!(trade.px, "102".to_string());
        assert_eq!(trade.time, 100);
    }

    #[test]
    fn test_that_buy_market_gtc_executes() {
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let order = Order {
            asset: 0,
            is_buy: true,
            limit_px: "105.00".to_string(),
            sz: "100.0".to_string(),
            reduce_only: false,
            cloid: None,
            order_type: super::OrderType::Limit(super::LimitOrder {
                tif: super::TimeInForce::Gtc,
            }),
        };
        orderbook.insert_order(100, order);
        let mut executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.0.len(), 1);

        let trade = executed.0.pop().unwrap();
        //Trade executes at 100 so trade price should be 102
        assert_eq!(trade.px, "102".to_string());
        assert_eq!(trade.time, 100);
    }

    #[test]
    fn test_that_sell_market_gtc_executes() {
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let order = Order {
            asset: 0,
            is_buy: false,
            limit_px: "95.00".to_string(),
            sz: "100.0".to_string(),
            reduce_only: false,
            cloid: None,
            order_type: super::OrderType::Limit(super::LimitOrder {
                tif: super::TimeInForce::Gtc,
            }),
        };
        orderbook.insert_order(100, order);
        let mut executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.0.len(), 1);

        let trade = executed.0.pop().unwrap();
        //Trade executes at 100 so trade price should be 101
        assert_eq!(trade.px, "101".to_string());
        assert_eq!(trade.time, 100);
    }

    #[test]
    fn test_that_sell_market_ioc_executes() {
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let order = Order {
            asset: 0,
            is_buy: false,
            limit_px: "99.00".to_string(),
            sz: "100.0".to_string(),
            reduce_only: false,
            cloid: None,
            order_type: super::OrderType::Limit(super::LimitOrder {
                tif: super::TimeInForce::Ioc,
            }),
        };
        orderbook.insert_order(100, order);
        let mut executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.0.len(), 1);

        let trade = executed.0.pop().unwrap();
        //Trade executes at 99 so trade price should be 101
        assert_eq!(trade.px, "101".to_string());
        assert_eq!(trade.time, 100);
    }

    #[test]
    fn test_that_buy_market_cancels_itself_if_price_too_high() {
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let order = Order {
            asset: 0,
            is_buy: true,
            limit_px: "1.00".to_string(),
            sz: "100.0".to_string(),
            reduce_only: false,
            cloid: None,
            order_type: super::OrderType::Limit(super::LimitOrder {
                tif: super::TimeInForce::Ioc,
            }),
        };
        let oid = orderbook.insert_order(100, order);
        // Should try to execute market order and fail marking the order as attempted
        let executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.0.len(), 0);

        // Should see that the order has been attempted and delete
        let executed_again = orderbook.execute_orders(101.into(), &source);
        assert_eq!(executed_again.0.len(), 0);

        // Should be deleted
        assert!(orderbook.get_order(oid).is_none());
    }

    #[test]
    fn test_that_sell_market_cancels_itself_if_price_too_high() {
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let order = Order {
            asset: 0,
            is_buy: false,
            limit_px: "999.00".to_string(),
            sz: "100.0".to_string(),
            reduce_only: false,
            cloid: None,
            order_type: super::OrderType::Limit(super::LimitOrder {
                tif: super::TimeInForce::Ioc,
            }),
        };
        let oid = orderbook.insert_order(100, order);
        // Should try to execute market order and fail marking the order as attempted
        let executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.0.len(), 0);

        // Should see that the order has been attempted and delete
        let executed_again = orderbook.execute_orders(101.into(), &source);
        assert_eq!(executed_again.0.len(), 0);

        // Should be deleted
        assert!(orderbook.get_order(oid).is_none());
    }

    #[test]
    fn test_that_buy_market_executes_within_slippage() {
        // Default slippage param is 10%. Price on first tick is 102 so market orders will
        // execute when limit_px*1.1 > price (not when price*0.9 > limit_px) so 93 is the lowest
        // price (roughly) that will trigger a market buy with ask of 102.

        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let order = Order {
            asset: 0,
            is_buy: true,
            limit_px: "93.00".to_string(),
            sz: "100.0".to_string(),
            reduce_only: false,
            cloid: None,
            order_type: super::OrderType::Limit(super::LimitOrder {
                tif: super::TimeInForce::Ioc,
            }),
        };
        orderbook.insert_order(100, order);
        let mut executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.0.len(), 1);

        let trade = executed.0.pop().unwrap();
        //Trade executes at 100 so trade price should be 102
        assert_eq!(trade.px, "102".to_string());
        assert_eq!(trade.time, 100);
    }

    #[test]
    fn test_that_sell_market_executes_within_slippage() {
        // Default slippage param is 10%. Price on first tick is 101 so market orders will
        // execute when limit_px*0.9 < price so 108 is the highest
        // price (roughly) that will trigger a market buy with bid of 101.

        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let order = Order {
            asset: 0,
            is_buy: false,
            limit_px: "108.00".to_string(),
            sz: "100.0".to_string(),
            reduce_only: false,
            cloid: None,
            order_type: super::OrderType::Limit(super::LimitOrder {
                tif: super::TimeInForce::Ioc,
            }),
        };
        orderbook.insert_order(100, order);
        let mut executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.0.len(), 1);

        let trade = executed.0.pop().unwrap();
        //Trade executes at 100 so trade price should be 101
        assert_eq!(trade.px, "101".to_string());
        assert_eq!(trade.time, 100);
    }

    #[test]
    fn test_that_trigger_order_triggers_stop_loss_long() {
        //Currently short, set stop loss to trigger immediately if 102 is hit. Trigger is same as
        //limit so this functions like a normal SL. Executs on next tick.
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let order = Order {
            asset: 0,
            is_buy: true,
            limit_px: "102.00".to_string(),
            sz: "100.0".to_string(),
            reduce_only: false,
            cloid: None,
            order_type: super::OrderType::Trigger(super::TriggerOrder {
                trigger_px: 102.0,
                is_market: true,
                tpsl: super::TriggerType::Sl,
            }),
        };
        orderbook.insert_order(100, order);
        let executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.0.len(), 0);
        assert_eq!(executed.1.len(), 1);

        let executed_next_tick = orderbook.execute_orders(101.into(), &source);
        assert_eq!(executed_next_tick.0.len(), 1);
        let trade = executed_next_tick.0.get(0).unwrap();

        assert_eq!(trade.px, "103".to_string());
        assert_eq!(trade.time, 101);
    }

    #[test]
    fn test_that_trigger_order_triggers_stop_loss_long_on_next_tick() {
        //Currently short, set stop loss to trigger immediately if 103 is hit. Trigger is same as
        //limit so this functions like a normal SL.
        //Order inserted on 100 and doesn't trigger as ask is 102, triggers on 101 as ask is 103,
        //executes on 103 when ask is 106.
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let order = Order {
            asset: 0,
            is_buy: true,
            limit_px: "103.00".to_string(),
            sz: "100.0".to_string(),
            reduce_only: false,
            cloid: None,
            order_type: super::OrderType::Trigger(super::TriggerOrder {
                trigger_px: 103.0,
                is_market: true,
                tpsl: super::TriggerType::Sl,
            }),
        };
        orderbook.insert_order(100, order);
        let executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.0.len(), 0);
        assert_eq!(executed.1.len(), 0);

        let executed_next_tick = orderbook.execute_orders(101.into(), &source);
        assert_eq!(executed_next_tick.0.len(), 0);
        assert_eq!(executed_next_tick.1.len(), 1);

        let executed_last_tick = orderbook.execute_orders(102.into(), &source);
        assert_eq!(executed_last_tick.0.len(), 1);
        let trade = executed_last_tick.0.get(0).unwrap();

        assert_eq!(trade.px, "106".to_string());
        assert_eq!(trade.time, 102);
    }

    #[test]
    fn test_that_trigger_order_triggers_stop_loss_short() {
        //Currently long, set stop loss to trigger immediately if 101 is hit. Trigger is same as
        //limit so this functions like a normal SL. Executes on next tick.
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let order = Order {
            asset: 0,
            is_buy: false,
            limit_px: "101.00".to_string(),
            sz: "100.0".to_string(),
            reduce_only: false,
            cloid: None,
            order_type: super::OrderType::Trigger(super::TriggerOrder {
                trigger_px: 101.0,
                is_market: true,
                tpsl: super::TriggerType::Sl,
            }),
        };
        orderbook.insert_order(100, order);
        let executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.0.len(), 0);
        assert_eq!(executed.1.len(), 1);

        let executed_next_tick = orderbook.execute_orders(101.into(), &source);
        assert_eq!(executed_next_tick.0.len(), 1);
        let trade = executed_next_tick.0.get(0).unwrap();

        assert_eq!(trade.px, "102".to_string());
        assert_eq!(trade.time, 101);
    }

    #[test]
    fn test_that_trigger_order_triggers_take_profit_long() {
        //Current short , set take profit to trigger immediately if 102 is hit. Trigger is same as
        //limit so this functions like a normal TP. Executes on next tick.
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let order = Order {
            asset: 0,
            is_buy: false,
            limit_px: "102.00".to_string(),
            sz: "100.0".to_string(),
            reduce_only: false,
            cloid: None,
            order_type: super::OrderType::Trigger(super::TriggerOrder {
                trigger_px: 102.0,
                is_market: true,
                tpsl: super::TriggerType::Tp,
            }),
        };
        orderbook.insert_order(100, order);
        let executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.0.len(), 0);
        assert_eq!(executed.1.len(), 1);

        let executed_next_tick = orderbook.execute_orders(101.into(), &source);
        assert_eq!(executed_next_tick.0.len(), 1);
        let trade = executed_next_tick.0.get(0).unwrap();

        assert_eq!(trade.px, "102".to_string());
        assert_eq!(trade.time, 101);
    }

    #[test]
    fn test_that_trigger_order_triggers_take_profit_short() {
        //Current long, set take profit to trigger immediately if 101 is hit. Trigger is same as
        //limit so this functions like a normal TP.
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let order = Order {
            asset: 0,
            is_buy: false,
            limit_px: "101.00".to_string(),
            sz: "100.0".to_string(),
            reduce_only: false,
            cloid: None,
            order_type: super::OrderType::Trigger(super::TriggerOrder {
                trigger_px: 101.0,
                is_market: true,
                tpsl: super::TriggerType::Tp,
            }),
        };
        orderbook.insert_order(100, order);
        let executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.0.len(), 0);
        assert_eq!(executed.1.len(), 1);

        let executed_next_tick = orderbook.execute_orders(101.into(), &source);
        assert_eq!(executed_next_tick.0.len(), 1);
        let trade = executed_next_tick.0.get(0).unwrap();

        assert_eq!(trade.px, "102".to_string());
        assert_eq!(trade.time, 101);
    }

    fn setup() -> JuraV1 {
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(101.00, 102.00, 100, "0".to_owned());
        source_builder.add_quote(102.00, 103.00, 101, "0".to_owned());
        source_builder.add_quote(105.00, 106.00, 102, "0".to_owned());

        let (source, clock) = source_builder.build_with_frequency(crate::clock::Frequency::Second);

        let exchange = JuraV1::new(clock, source, "FAKE");
        exchange
    }

    #[test]
    fn test_that_buy_market_executes_incrementing_trade_log() {
        let mut exchange = setup();

        exchange.insert_order(Order::market_buy(0_u64, "100.0", "102.00"));
        exchange.tick();

        //TODO: no abstraction!
        assert_eq!(exchange.trade_log.len(), 1);
    }

    #[test]
    fn test_that_multiple_orders_are_executed_on_same_tick() {
        let mut exchange = setup();

        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));
        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));
        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));
        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));

        exchange.tick();
        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_multiple_orders_are_executed_on_consecutive_tick() {
        let mut exchange = setup();
        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));
        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));
        exchange.tick();

        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));
        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));
        exchange.tick();

        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_buy_market_executes_on_next_tick() {
        //Verifies that trades do not execute instaneously removing lookahead bias
        let mut exchange = setup();

        exchange.insert_order(Order::market_buy(0_u64, "100.0", "102.00"));
        exchange.tick();

        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 103
        assert_eq!(trade.px, "103");
        assert_eq!(trade.time, 101);
    }

    #[test]
    fn test_that_sell_market_executes_on_next_tick() {
        //Verifies that trades do not execute instaneously removing lookahead bias
        let mut exchange = setup();

        exchange.insert_order(Order::market_sell(0_u64, "100.0", "101.00"));
        exchange.tick();

        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 102
        assert_eq!(trade.px, "102");
        assert_eq!(trade.time, 101);
    }

    #[test]
    fn test_that_order_for_nonexistent_stock_fails_silently() {
        let mut exchange = setup();

        exchange.insert_order(Order::market_buy(99_u64, "100.0", "102.00"));
        exchange.tick();

        assert_eq!(exchange.trade_log.len(), 0);
    }

    #[test]
    fn test_that_order_buffer_clears() {
        //Sounds redundant but accidentally removing the clear could cause unusual errors elsewhere
        let mut exchange = setup();

        exchange.insert_order(Order::market_buy(0_u64, "100.0", "102.00"));
        exchange.tick();

        assert!(exchange.order_buffer.is_empty());
    }

    #[test]
    fn test_that_order_with_missing_price_executes_later() {
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(101.00, 102.00, 100, "0".to_owned());
        source_builder.add_quote(105.00, 106.00, 102, "0".to_owned());

        let (source, clock) = source_builder.build_with_frequency(crate::clock::Frequency::Second);

        let mut exchange = JuraV1::new(clock, source, "FAKE");

        exchange.insert_order(Order::market_buy(0_u64, "100.0", "102.00"));
        exchange.tick();
        //Orderbook should have one order and trade log has no executed trades
        assert_eq!(exchange.trade_log.len(), 0);

        exchange.tick();
        //Order should execute now
        assert_eq!(exchange.trade_log.len(), 1);
    }

    #[test]
    fn test_that_sells_are_executed_before_buy() {
        let mut exchange = setup();

        exchange.insert_order(Order::market_buy(0_u64, "100.0", "102.00"));
        exchange.insert_order(Order::market_buy(0_u64, "100.0", "102.00"));
        exchange.insert_order(Order::market_sell(0_u64, "100.0", "102.00"));
        let res = exchange.tick();

        assert_eq!(res.1.len(), 3);
        assert_eq!(res.1.get(0).unwrap().side, String::from(Side::Bid))
    }
}
