use std::ops::Index;

pub mod record;

#[derive(Clone, Debug)]
pub struct Quote {
    pub bid: f64,
    pub ask: f64,
    pub date: i64,
    pub symbol: String,
}

#[derive(Clone, Debug)]
pub struct Trade {
    pub symbol: String,
    pub value: f64,
    pub quantity: f64,
}

#[derive(Clone)]

pub enum BrokerEvent {
    TradeSuccess(Trade),
    TradeFailure(Order),
    OrderCreated(Order),
    OrderFailure(Order),
    SuccessfulWithdraw(f64),
    CashTransactionSuccess(f64),
    InsufficientCash(f64),
}

pub trait CashManager {
    fn withdraw_cash(&mut self, cash: f64) -> BrokerEvent;
    fn deposit_cash(&mut self, cash: f64) -> BrokerEvent;
    fn debit(&mut self, value: f64) -> BrokerEvent;
    fn credit(&mut self, value: f64) -> BrokerEvent;
    fn get_cash_balance(&self) -> f64;
}

pub trait PositionInfo {
    fn get_position_qty(&self, symbol: &String) -> Option<f64>;
    fn get_position_value(&self, symbol: &String) -> Option<f64>;
    fn get_position_cost(&self, symbol: &String) -> Option<f64>;
    fn get_position_profit(&self, symbol: &String) -> Option<f64>;
}

pub trait PriceQuote {
    fn get_quote(&self, symbol: &String) -> Option<Quote>;
}

/* Returning trait object is needed to require some abstraction from
   whatever data structure is used to represent portfolio holdings.
   
   Used to implement a Holdings enum within broker that fixed the
   underlying datastructure, but it makes more sense to let clients
   implement this. The only specification we need here is that we
   can index the object for the number of units held.
 */
pub trait ClientControlled {
    fn update_holdings(&mut self, symbol: &String, change: &f64);
    fn get_holdings(&self) -> &(dyn Index<&String, Output = f64>);
    fn get(&self, symbol: &String) -> Option<&f64>;
}

pub trait TradeLedger {
    fn record(&mut self, trade: &Trade);
    fn cost_basis(&self, symbol: &String) -> Option<f64>;
}

pub trait PendingOrders {
    fn insert_order(&mut self, order: &Order);
    fn delete_order(&mut self, id: &u8);
}

pub trait PriceAPI {
    fn get_prices(&self, symbol: &String) -> Option<Quote>;
    fn get_all_prices(&self) -> Vec<Quote>;
}

#[derive(Clone, Copy, Debug)]
pub enum OrderType {
    MarketSell,
    MarketBuy,
    LimitSell,
    LimitBuy,
    StopSell,
    StopBuy,
}

#[derive(Clone, Debug)]
pub struct Order {
    order_type: OrderType,
    symbol: String,
    shares: f64,
    price: Option<f64>,
}

impl Order {
    pub fn get_symbol(&self) -> String {
        self.symbol.clone()
    }

    pub fn get_shares(&self) -> f64 {
        self.shares
    }

    pub fn get_price(&self) -> Option<f64> {
        self.price
    }

    pub fn get_order_type(&self) -> OrderType {
        self.order_type
    }

    pub fn new(order_type: OrderType, symbol: String, shares: f64, price: Option<f64> ) -> Self {
        Order {
            order_type,
            symbol,
            shares,
            price
        }
    }
}

pub trait OrderExecutor {
    fn execute_order(&mut self, order: &Order) -> BrokerEvent;
    fn execute_orders(&mut self, orders: Vec<Order>) -> Vec<BrokerEvent>;
}
