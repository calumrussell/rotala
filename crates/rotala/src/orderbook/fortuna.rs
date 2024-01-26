use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

pub trait FortunaSource {
    fn get_quote(&self, date: &i64, security: &u64) -> Option<impl FortunaQuote>;
}

pub trait FortunaQuote {
    fn get_ask(&self) -> f64;
    fn get_bid(&self) -> f64;
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum FortunaSide {
    Ask,
    Bid,
}

impl From<FortunaSide> for String {
    fn from(value: FortunaSide) -> Self {
        match value {
            FortunaSide::Bid => "B".to_string(),
            FortunaSide::Ask => "A".to_string(),
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
pub struct FortunaFill {
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

pub type FortunaOrderId = u64;

#[derive(Clone, Debug, Deserialize, Serialize)]
enum TimeInForce {
    Alo,
    Ioc,
    Gtc,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FortunaLimitOrder {
    tif: TimeInForce,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum TriggerType {
    Tp,
    Sl,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FortunaTriggerOrder {
    // Differs from limit_px as trigger_px is the price that triggers the order, limit_px is the
    // price that the user wants to trade at but is subject to the same slippage limitations
    // For some reason this is a number but limit_px is a String?
    trigger_px: f64,
    // If this is true then the order will execute immediately with max slippage of 10%
    is_market: bool,
    tpsl: TriggerType,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum FortunaOrderType {
    Limit(FortunaLimitOrder),
    Trigger(FortunaTriggerOrder),
}

/// The assumed function of the Hyperliquid API is followed as far as possible. A major area of
/// uncertainty in the API docs concerned what the exchange does when it receives an order that
/// has some properties set like a market order and some set like a limit order. The assumption
/// made throughout the implementation is that the is_market field, set on [FortunaTriggerOrder]
/// , determines fully whether an order is a market order and everything else is limit.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FortunaOrder {
    asset: u64,
    is_buy: bool,
    // What if limit_px has value but is_market is true?
    limit_px: String,
    sz: String,
    reduce_only: bool,
    //This is client order id, need to check whether test impl should reasonably use this.
    cloid: Option<String>,
    order_type: FortunaOrderType,
}

struct FortunaInnerOrder {
    pub order_id: FortunaOrderId,
    pub order_time_received: i64,
    pub order: FortunaOrder,
    pub attempted_execution: bool,
}

impl FortunaInnerOrder {
    pub fn get_shares(&self) -> f64 {
        // We unwrap immediately because we can't continue if the client is passing incorrectly
        // sized orders
        str::parse::<f64>(&self.order.sz).unwrap()
    }
}

/// Fortuna is an implementation of the Hyperliquid API running against a local server. This allows
/// testing of strategies using the same API/order types/etc.
///
/// Hyperliquid is a derivatives exchange. In order to simplify the implementation it is assumed
/// that everything is cash/no margin/no leverage.
/// 
/// Hyperliquid has two order types: limit and trigger. 
/// 
/// A limit order can be set to execute immediately, with [TimeInForce::Ioc], and will execute on
/// the next tick with slippage of 10%. Slippage is constant to the implementation as this appears
/// to be the default setting in prod. If this doesn't execute on the next tick then it is
/// cancelled.
/// 
/// A trigger order has distinct trigger_px and limit_px. The trigger_px is the price that triggers
/// the order to enter the book. Once this occurs, it is treated as a normal limit market order or
/// limit order that uses the limit_px to determine execution. This will be queued onto the same
/// tick.
/// 
/// After a trade executes a fill is returned to the user, this is substantially different to the
/// Hyperliquid API due to Hyperliquid performing functions like margin. The differences are
/// documented in [FortunaFill].
pub struct Fortuna {
    inner: VecDeque<FortunaInnerOrder>,
    last_inserted: u64,
    slippage: f64,
}

impl Default for Fortuna {
    fn default() -> Self {
        Self::new()
    }
}

impl Fortuna {
    pub fn new() -> Self {
        Self {
            inner: std::collections::VecDeque::new(),
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
    pub fn insert_order(&mut self, date: i64, order: FortunaOrder) -> FortunaOrderId {
        let order_id = self.last_inserted.clone();
        // We assume that orders are received instaneously.
        // Latency can be added here when this is implemented.
        let inner_order = FortunaInnerOrder {
            order_id,
            order,
            order_time_received: date,
            attempted_execution: false,
        };
        self.inner.push_back(inner_order);
        self.last_inserted += 1;
        order_id
    }

    fn execute_buy(quote: impl FortunaQuote, order: &FortunaInnerOrder, date: i64) -> FortunaFill {
        let trade_price = quote.get_ask();
        FortunaFill {
            closed_pnl: "0.0".to_string(),
            coin: order.order.asset.to_string(),
            crossed: false,
            dir: false,
            hash: false,
            oid: order.order_id,
            px: trade_price.to_string(),
            side: FortunaSide::Ask.into(),
            start_position: false,
            sz: order.get_shares().to_string(),
            time: date,
        }
    }

    fn execute_sell(quote: impl FortunaQuote, order: &FortunaInnerOrder, date: i64) -> FortunaFill {
        let trade_price = quote.get_bid();
        FortunaFill {
            closed_pnl: "0.0".to_string(),
            coin: order.order.asset.to_string(),
            crossed: false,
            dir: false,
            hash: false,
            oid: order.order_id,
            px: trade_price.to_string(),
            side: FortunaSide::Bid.into(),
            start_position: false,
            sz: order.get_shares().to_string(),
            time: date,
        }
    }

    pub fn execute_orders(&mut self, date: i64, source: impl FortunaSource) -> Vec<FortunaFill> {
        let mut fills: Vec<FortunaFill> = Vec::new();
        let mut should_delete: Vec<(u64, u64)> = Vec::new();

        // We have to have a mutable reference so we can update attempted_execution
        for order in self.inner.iter_mut() {
            let symbol = order.order.asset;
            if let Some(quote) = source.get_quote(&date, &symbol.clone()) {
                let result = match &order.order.order_type {
                    FortunaOrderType::Limit(limit) => {
                        // A market order is a limit order with Ioc time-in-force. The px parameter
                        // on the order is taken from the order and seems to be used to calculate
                        // maximum slippage tolerated on the order.
                        // Market order code in Python SDK:
                        // https://github.com/hyperliquid-dex/hyperliquid-python-sdk/blob/67864cf979d3bbea2e964a99ecc0a1effb7bb911/hyperliquid/exchange.py#L209
                        match limit.tif {
                            // Don't support Alo TimeInForce
                            TimeInForce::Ioc => {
                                // Market orders can only be executed on the next time step
                                if order.attempted_execution == false {
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
                                    } else {
                                        if price * (1.0 - self.slippage) <= quote.get_bid() {
                                            should_delete.push((order.order.asset, order.order_id));
                                            Some(Self::execute_sell(quote, order, date))
                                        } else {
                                            None
                                        }
                                    }
                                }
                            }
                            _ => unimplemented!(),
                        }
                    }
                    FortunaOrderType::Trigger(trigger) => {
                        // If we trigger a market order, execute it here. If the trigger is for a
                        // limit order then we create another order add it to the queue and return
                        // the order_id to the client

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
                                            let triggered_order = FortunaOrder {
                                                asset: order.order.asset,
                                                is_buy: order.order.is_buy,
                                                limit_px: order.order.limit_px.clone(),
                                                sz: order.order.sz.clone(),
                                                reduce_only: order.order.reduce_only,
                                                cloid: order.order.cloid.clone(),
                                                order_type: FortunaOrderType::Limit(
                                                    FortunaLimitOrder {
                                                        tif: TimeInForce::Ioc,
                                                    }
                                                )
                                            };
                                            self.insert_order(date, triggered_order);
                                        } else {

                                        }

                                        should_delete.push((order.order.asset, order.order_id));
                                        Some(Self::execute_buy(quote, order, date))
                                    } else {
                                        None
                                    } 
                                } else {
                                    // Closing a long as price goes down
                                    if quote.get_bid() >= trigger.trigger_px {
                                        should_delete.push((order.order.asset, order.order_id));
                                        Some(Self::execute_sell(quote, order, date))
                                    }  else {
                                        None
                                    }
                                }
                            },
                            TriggerType::Tp => {
                                // Closing a short as price goes down
                                if order.order.is_buy {
                                    if quote.get_ask() <= trigger.trigger_px {
                                        should_delete.push((order.order.asset, order.order_id));
                                        Some(Self::execute_buy(quote, order, date))
                                    } else {
                                        None
                                    }
                                } else {
                                    // Closing a long as price goes up
                                    if quote.get_bid() <= trigger.trigger_px {
                                        should_delete.push((order.order.asset, order.order_id));
                                        Some(Self::execute_sell(quote, order, date))
                                    }  else {
                                        None
                                    }
                                }
                            },
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

        fills
    }
}
