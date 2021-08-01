use std::collections::HashMap;
use std::rc::Rc;

use crate::data::{DataSourceSim, SimSource};
use crate::types::{Order, OrderType};

#[derive(Clone)]
pub struct Quote {
    pub bid: f64,
    pub ask: f64,
    pub date: i64,
    pub symbol: String,
}

pub struct Trade {
    symbol: String,
    value: f64,
}

pub enum BrokerEvent {
    TradeSuccess(Trade),
    TradeFailure(Order),
    SuccessfulWithdraw(f64),
    InsufficientCash(f64),
}

pub trait CashManager {
    fn withdraw_cash(&mut self, cash: f64) -> BrokerEvent;
    fn deposit_cash(&mut self, cash: f64) -> BrokerEvent;
}

pub trait OrderExecutor {
    fn execute_order(&mut self, order: &Order) -> BrokerEvent;
    fn execute_orders(&mut self, orders: Vec<Order>) -> Vec<BrokerEvent>;
    fn trade_logic(&mut self, value: &f64, order: &Order) -> BrokerEvent;
}

pub trait PositionInfo {
    fn get_position_qty(&self, symbol: &String) -> Option<f64>;
    fn get_position_value(&self, symbol: &String) -> Option<f64>;
}

pub trait PriceQuote {
    fn get_quote(&self, symbol: &String) -> Option<Quote>;
}

pub struct SimulatedBroker<T>
where
    T: SimSource,
{
    holdings: HashMap<String, f64>,
    raw_data: Rc<DataSourceSim<T>>,
    date: i64,
    cash: f64,
}

impl<T> CashManager for SimulatedBroker<T>
where
    T: SimSource,
{
    fn withdraw_cash(&mut self, cash: f64) -> BrokerEvent {
        if cash > self.cash {
            return BrokerEvent::InsufficientCash(cash);
        }
        self.cash -= cash;
        BrokerEvent::SuccessfulWithdraw(cash)
    }

    fn deposit_cash(&mut self, cash: f64) -> BrokerEvent {
        self.cash += cash.clone();
        BrokerEvent::SuccessfulWithdraw(cash)
    }
}

impl<T> OrderExecutor for SimulatedBroker<T>
where
    T: SimSource,
{
    fn execute_order(&mut self, order: &Order) -> BrokerEvent {
        let quote = self
            .raw_data
            .source
            .get_date_symbol(&self.date, &order.symbol);

        if quote.is_err() {
            return BrokerEvent::TradeFailure(order.clone());
        }

        let mut price = 0.0;
        match order.order_type {
            OrderType::MarketBuy => price = quote.unwrap().ask,
            OrderType::MarketSell => price = quote.unwrap().bid,
        }
        let value = price * order.shares as f64;
        self.trade_logic(&value, order)
    }

    fn execute_orders(&mut self, orders: Vec<Order>) -> Vec<BrokerEvent> {
        let mut res = Vec::new();
        for o in orders {
            let trade = self.execute_order(&o);
            res.push(trade);
        }
        res
    }

    fn trade_logic(&mut self, value: &f64, order: &Order) -> BrokerEvent {
        if self.cash > *value {
            return BrokerEvent::TradeFailure(order.clone());
        }

        //Update holdings
        let curr = self.holdings.get(&order.symbol).unwrap_or(&0.0);
        let updated = curr + order.shares as f64;
        self.holdings.insert(order.symbol.clone(), updated.clone());

        //Update cash
        self.cash -= *value as f64;

        let t = Trade {
            symbol: order.symbol.clone(),
            value: *value,
        };
        BrokerEvent::TradeSuccess(t)
    }
}

impl<T> PositionInfo for SimulatedBroker<T>
where
    T: SimSource,
{
    fn get_position_qty(&self, symbol: &String) -> Option<f64> {
        let pos = self.holdings.get(symbol);
        match pos {
            Some(p) => Some(p.clone()),
            _ => None,
        }
    }

    fn get_position_value(&self, symbol: &String) -> Option<f64> {
        let quote = self.raw_data.source.get_date_symbol(&self.date, symbol);
        //TODO: we need to introduce some kind of distinction between short and long
        //      positions.

        if quote.is_ok() {
            let price = quote.unwrap().ask;
            let qty = self.get_position_qty(symbol);
            if qty.is_some() {
                return Some(price * qty.unwrap() as f64);
            }
            return None;
        }
        None
    }
}

impl<T> PriceQuote for SimulatedBroker<T>
where
    T: SimSource,
{
    fn get_quote(&self, symbol: &String) -> Option<Quote> {
        let quote = self.raw_data.source.get_date_symbol(&self.date, symbol);
        match quote {
            Ok(q) => Some(q),
            _ => None,
        }
    }
}

impl<T> SimulatedBroker<T>
where
    T: SimSource,
{
    pub fn set_date(&mut self, date: i64) {
        self.date = date;
    }

    pub fn new(raw_data: Rc<DataSourceSim<T>>) -> SimulatedBroker<T> {
        let holdings: HashMap<String, f64> = HashMap::new();
        SimulatedBroker {
            holdings,
            raw_data,
            date: -1,
            cash: 0.0,
        }
    }
}
