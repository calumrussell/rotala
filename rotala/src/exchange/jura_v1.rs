use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::input::penelope::{PenelopeQuote, PenelopeQuoteByDate};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct JuraQuote {
    bid: f64,
    ask: f64,
    date: i64,
    symbol: String,
}

impl From<PenelopeQuote> for JuraQuote {
    fn from(value: PenelopeQuote) -> Self {
        Self {
            bid: value.bid,
            ask: value.ask,
            date: value.date,
            symbol: value.symbol,
        }
    }
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
///   be set to false
/// * crossed, this is unclear and may relate to margin or the execution of previous trades, this
///   will always be set to false
/// * hash, will always be an empty string, as HL is on-chain a transaction hash is produced but
///   won't be in a test env, always set to false
/// * start_position, unimplemented as this relates to overall position which is untracked, will
///   always be set to false
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Fill {
    pub closed_pnl: String,
    pub coin: String,
    pub crossed: bool,
    pub dir: bool,
    pub hash: bool,
    pub oid: u64,
    pub px: String,
    pub side: String,
    pub start_position: bool,
    pub sz: String,
    pub time: i64,
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

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub struct JuraV1 {
    orderbook: OrderBook,
    trade_log: Vec<Fill>,
    //This is cleared on every tick
    order_buffer: Vec<Order>,
}

impl JuraV1 {
    pub fn new() -> Self {
        Self {
            orderbook: OrderBook::default(),
            trade_log: Vec::new(),
            order_buffer: Vec::new(),
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

    pub fn tick(&mut self, quotes: &PenelopeQuoteByDate) -> (Vec<Fill>, Vec<Order>, Vec<u64>) {
        //To eliminate lookahead bias, we only insert new orders after we have executed any orders
        //that were on the stack first
        let (fills, triggered_order_ids) = self.orderbook.execute_orders(quotes);
        for fill in &fills {
            self.trade_log.push(fill.clone());
        }

        self.sort_order_buffer();
        for order in self.order_buffer.iter_mut() {
            self.orderbook.insert_order(order.clone());
        }

        println!("{:?}", self.orderbook);

        let inserted_orders = std::mem::take(&mut self.order_buffer);
        (fills, inserted_orders, triggered_order_ids)
    }
}

impl Default for JuraV1 {
    fn default() -> Self {
        Self::new()
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
#[derive(Clone, Debug)]
struct OrderBook {
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
    pub fn insert_order(&mut self, order: Order) -> OrderId {
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
        let trade_price = quote.ask;
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
        let trade_price = quote.bid;
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

    pub fn execute_orders(&mut self, quotes: &PenelopeQuoteByDate) -> (Vec<Fill>, Vec<OrderId>) {
        let mut fills: Vec<Fill> = Vec::new();
        let mut should_delete: Vec<(u64, u64)> = Vec::new();
        // HyperLiquid execution can trigger more orders, we don't execute these immediately.
        let mut should_insert: Vec<Order> = Vec::new();

        // We have to have a mutable reference so we can update attempted_execution
        for order in self.inner.iter_mut() {
            let symbol = order.order.asset.to_string();
            if let Some(quote) = quotes.get(&symbol) {
                let quote_copy: JuraQuote = quote.clone().into();
                let date = quote_copy.date;
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
                                        if price * (1.0 + self.slippage) >= quote_copy.ask {
                                            should_delete.push((order.order.asset, order.order_id));
                                            Some(Self::execute_buy(quote_copy, order, date))
                                        } else {
                                            None
                                        }
                                    } else if price * (1.0 - self.slippage) <= quote_copy.bid {
                                        should_delete.push((order.order.asset, order.order_id));
                                        Some(Self::execute_sell(quote_copy, order, date))
                                    } else {
                                        None
                                    }
                                }
                            }
                            TimeInForce::Gtc => {
                                let price = str::parse::<f64>(&order.order.limit_px).unwrap();
                                if order.order.is_buy {
                                    if price >= quote_copy.ask {
                                        should_delete.push((order.order.asset, order.order_id));
                                        Some(Self::execute_buy(quote_copy, order, date))
                                    } else {
                                        None
                                    }
                                } else if price <= quote_copy.bid {
                                    should_delete.push((order.order.asset, order.order_id));
                                    Some(Self::execute_sell(quote_copy, order, date))
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
                                    if quote_copy.ask >= trigger.trigger_px {
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
                                    if quote_copy.bid >= trigger.trigger_px {
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
                                    if quote_copy.ask <= trigger.trigger_px {
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
                                    if quote_copy.bid <= trigger.trigger_px {
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
            triggered_order_ids.push(self.insert_order(order));
        }
        (fills, triggered_order_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::{JuraV1, Order};
    use crate::input::penelope::Penelope;

    fn setup() -> (Penelope, JuraV1) {
        let mut source = Penelope::new();
        source.add_quote(101.00, 102.00, 100, "0".to_owned());
        source.add_quote(102.00, 103.00, 101, "0".to_owned());
        source.add_quote(105.00, 106.00, 102, "0".to_owned());

        let exchange = JuraV1::new();

        (source, exchange)
    }

    #[test]
    fn test_that_buy_market_executes_incrementing_trade_log() {
        let (source, mut exchange) = setup();

        exchange.insert_order(Order::market_buy(0_u64, "100.0", "102.00"));
        exchange.tick(source.get_quotes_unchecked(&100));
        exchange.tick(source.get_quotes_unchecked(&101));

        //TODO: no abstraction!
        assert_eq!(exchange.trade_log.len(), 1);
    }

    #[test]
    fn test_that_multiple_orders_are_executed_on_same_tick() {
        let (source, mut exchange) = setup();

        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));

        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));
        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));
        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));

        exchange.tick(source.get_quotes_unchecked(&100));
        exchange.tick(source.get_quotes_unchecked(&101));
        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_multiple_orders_are_executed_on_consecutive_tick() {
        let (source, mut exchange) = setup();
        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));
        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));
        exchange.tick(source.get_quotes_unchecked(&100));

        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));
        exchange.insert_order(Order::market_buy(0_u64, "25.0", "102.00"));
        exchange.tick(source.get_quotes_unchecked(&101));
        exchange.tick(source.get_quotes_unchecked(&102));

        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_buy_market_executes_on_next_tick() {
        //Verifies that trades do not execute instaneously removing lookahead bias
        let (source, mut exchange) = setup();

        exchange.insert_order(Order::market_buy(0_u64, "100.0", "102.00"));
        exchange.tick(source.get_quotes_unchecked(&100));
        exchange.tick(source.get_quotes_unchecked(&101));

        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 103
        assert_eq!(trade.px, "103");
        assert_eq!(trade.time, 101);
    }

    #[test]
    fn test_that_sell_market_executes_on_next_tick() {
        //Verifies that trades do not execute instaneously removing lookahead bias
        let (source, mut exchange) = setup();

        exchange.insert_order(Order::market_sell(0_u64, "100.0", "101.00"));
        exchange.tick(source.get_quotes_unchecked(&100));
        exchange.tick(source.get_quotes_unchecked(&101));

        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 102
        assert_eq!(trade.px, "102");
        assert_eq!(trade.time, 101);
    }

    #[test]
    fn test_that_order_for_nonexistent_stock_fails_silently() {
        let (source, mut exchange) = setup();

        exchange.insert_order(Order::market_buy(99_u64, "100.0", "102.00"));
        exchange.tick(source.get_quotes_unchecked(&100));

        assert_eq!(exchange.trade_log.len(), 0);
    }

    #[test]
    fn test_that_order_buffer_clears() {
        //Sounds redundant but accidentally removing the clear could cause unusual errors elsewhere
        let (source, mut exchange) = setup();

        exchange.insert_order(Order::market_buy(0_u64, "100.0", "102.00"));
        exchange.tick(source.get_quotes_unchecked(&100));

        assert!(exchange.order_buffer.is_empty());
    }

    #[test]
    fn test_that_order_with_missing_price_executes_later() {
        let mut source = Penelope::new();
        source.add_quote(101.00, 102.00, 100, "0".to_owned());
        source.add_quote(105.00, 106.00, 102, "0".to_owned());

        let mut exchange = JuraV1::new();

        exchange.insert_order(Order::market_buy(0_u64, "100.0", "102.00"));
        exchange.tick(source.get_quotes_unchecked(&100));
        //Orderbook should have one order and trade log has no executed trades
        assert_eq!(exchange.trade_log.len(), 0);

        exchange.tick(source.get_quotes_unchecked(&102));
        //Order should execute now
        assert_eq!(exchange.trade_log.len(), 1);
    }

    #[test]
    fn test_that_sells_are_executed_before_buy() {
        let (source, mut exchange) = setup();

        exchange.insert_order(Order::market_buy(0_u64, "100.0", "102.00"));
        exchange.insert_order(Order::market_buy(0_u64, "100.0", "102.00"));
        exchange.insert_order(Order::market_sell(0_u64, "100.0", "102.00"));
        let res = exchange.tick(source.get_quotes_unchecked(&100));

        assert_eq!(res.1.len(), 3);
        assert!(!(res.1.first().unwrap().is_buy));
    }
}
