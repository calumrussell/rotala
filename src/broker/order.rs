use itertools::Itertools;
use std::collections::HashMap;

use super::{BrokerEvent, CashManager, ClientControlled, Quote, Trade, TradeLedger};

#[derive(Clone, Copy)]
pub enum OrderType {
    MarketSell,
    MarketBuy,
    LimitSell,
    LimitBuy,
    StopSell,
    StopBuy,
}

#[derive(Clone)]
pub struct Order {
    pub order_type: OrderType,
    pub symbol: String,
    pub shares: f64,
    pub price: Option<f64>,
}

pub struct OrderExecutionRules;

impl OrderExecutionRules {
    pub fn client_has_sufficient_cash(
        order: &Order,
        price: &f64,
        brkr: &impl CashManager,
    ) -> Result<bool, f64> {
        match order.order_type {
            OrderType::MarketBuy => {
                let value = price * order.shares as f64;
                if brkr.get_cash_balance() > value {
                    return Ok(true);
                }
                Err(value)
            }
            _ => Ok(true),
        }
    }

    pub fn trade_logic(
        order: &Order,
        price: &f64,
        brkr: &mut (impl CashManager + ClientControlled + TradeLedger),
    ) -> Trade {
        let value = price * order.shares;
        //Update holdings
        let curr = brkr.get(&order.symbol).unwrap_or(&0.0);
        let updated = match order.order_type {
            OrderType::MarketBuy => curr + order.shares as f64,
            OrderType::MarketSell => curr - order.shares as f64,
            _ => panic!("Cannot call trade_logic with a non-market order"),
        };
        brkr.update_holdings(&order.symbol, &updated);

        //Update cash
        brkr.debit(value);

        let t = Trade {
            symbol: order.symbol.clone(),
            value,
            quantity: order.shares.clone() as f64,
        };

        //Update trade ledger
        brkr.record(&t);
        t
    }

    pub fn run_all<'a>(
        order: &'a Order,
        price: &'a f64,
        brkr: &'a mut (impl CashManager + ClientControlled + TradeLedger),
    ) -> Result<impl FnOnce() -> Trade + 'a, BrokerEvent> {
        let has_cash = OrderExecutionRules::client_has_sufficient_cash(order, price, brkr);
        if has_cash.is_err() {
            return Err(BrokerEvent::InsufficientCash(has_cash.unwrap_err()));
        }
        let trade = move || OrderExecutionRules::trade_logic(order, price, brkr);
        Ok(trade)
    }
}

pub trait OrderExecutor {
    fn execute_order(&mut self, order: &Order) -> BrokerEvent;
    fn execute_orders(&mut self, orders: Vec<Order>) -> Vec<BrokerEvent>;
}

pub struct SimOrderBook {
    orderbook: HashMap<u8, Order>,
    last: u8,
}

impl SimOrderBook {
    //We only check orders that all have the same symbol
    //so only need to test the price
    fn check_order(order: &Order, quote: &Quote) -> bool {
        //Only orders that have prices should be passed here
        let order_price = order.price.unwrap();

        match order.order_type {
            OrderType::LimitBuy => order_price < quote.ask,
            OrderType::LimitSell => order_price > quote.bid,
            OrderType::StopBuy => quote.ask > order_price,
            OrderType::StopSell => quote.bid < order_price,
            _ => false,
        }
    }

    //This has to return a new HashMap, not a reference to the underlying data structure,
    //as this method can mutate the state of the orderbook
    pub fn check_orders_by_symbol(&self, quote: &Quote) -> Option<HashMap<u8, Order>> {
        let mut res: HashMap<u8, Order> = HashMap::new();

        let symbol_ids = self.get_orders_by_symbol(&quote.symbol);
        if symbol_ids.len() == 0 {
            return None;
        }

        for id in symbol_ids {
            //Ids come from orderbook so will always have key
            let order = self.orderbook.get(&id).unwrap();
            let should_order_trigger = SimOrderBook::check_order(&order, &quote);

            if should_order_trigger {
                res.insert(id, order.clone());
            }
        }
        Some(res)
    }

    fn get_orders_by_symbol(&self, symbol: &String) -> Vec<u8> {
        self.orderbook
            .iter()
            .filter(|(_id, order)| order.symbol.eq(symbol))
            .map(|(id, _order)| id.clone())
            .collect_vec()
    }

    //Market orders are executed immediately so cannot
    //be stored, fail silently
    pub fn insert_order(&mut self, order: &Order) -> BrokerEvent {
        match order.order_type {
            OrderType::MarketBuy | OrderType::MarketSell => {
                BrokerEvent::OrderFailure(order.clone())
            }
            _ => {
                self.last += 1;
                self.orderbook.insert(self.last, order.clone());
                BrokerEvent::OrderCreated(order.clone())
            }
        }
    }

    pub fn delete_order(&mut self, order_id: &u8) {
        self.orderbook.remove(order_id);
    }

    pub fn new() -> Self {
        let orderbook: HashMap<u8, Order> = HashMap::new();
        SimOrderBook { orderbook, last: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::{BrokerEvent, Order, OrderType, SimOrderBook};
    use crate::broker::Quote;

    fn setup() -> (SimOrderBook, Quote) {
        let quote = Quote {
            bid: 101.00,
            ask: 102.00,
            date: 100,
            symbol: String::from("ABC"),
        };

        (SimOrderBook::new(), quote.clone())
    }

    #[test]
    fn test_that_orderbook_with_buy_limit_triggers_correctly() {
        let order = Order {
            order_type: OrderType::LimitBuy,
            symbol: String::from("ABC"),
            shares: 100.0,
            price: Some(100.00),
        };

        let order1 = Order {
            order_type: OrderType::LimitBuy,
            symbol: String::from("ABC"),
            shares: 100.0,
            price: Some(105.00),
        };

        let (mut orderbook, quote) = setup();
        orderbook.insert_order(&order);
        orderbook.insert_order(&order1);

        let res = orderbook.check_orders_by_symbol(&quote).unwrap();
        assert!(res.len().eq(&1));
    }

    #[test]
    fn test_that_orderbook_with_sell_limit_triggers_correctly() {
        let order = Order {
            order_type: OrderType::LimitSell,
            symbol: String::from("ABC"),
            shares: 100.0,
            price: Some(100.00),
        };

        let order1 = Order {
            order_type: OrderType::LimitSell,
            symbol: String::from("ABC"),
            shares: 100.0,
            price: Some(105.00),
        };

        let (mut orderbook, quote) = setup();
        orderbook.insert_order(&order);
        orderbook.insert_order(&order1);

        let res = orderbook.check_orders_by_symbol(&quote).unwrap();
        assert!(res.len().eq(&1));
    }

    #[test]
    fn test_that_orderbook_with_buy_stop_triggers_correctly() {
        //We are short from 90, and we put a StopBuy of 100 & 105 to take
        //off the position. If we are quoted 101/102 then our 100 order
        //should be executed.
        let order = Order {
            order_type: OrderType::StopBuy,
            symbol: String::from("ABC"),
            shares: 100.0,
            price: Some(100.00),
        };

        let order1 = Order {
            order_type: OrderType::StopBuy,
            symbol: String::from("ABC"),
            shares: 100.0,
            price: Some(105.00),
        };

        let (mut orderbook, quote) = setup();
        orderbook.insert_order(&order);
        orderbook.insert_order(&order1);

        let res = orderbook.check_orders_by_symbol(&quote).unwrap();
        assert!(res.len().eq(&1));
    }

    #[test]
    fn test_that_orderbook_with_sell_stop_triggers_correctly() {
        //Long from 110, we place orders to exit at 100 and 105.
        //If we are quoted 101/102 then our 105 StopSell is executed.
        let order = Order {
            order_type: OrderType::StopSell,
            symbol: String::from("ABC"),
            shares: 100.0,
            price: Some(100.00),
        };

        let order1 = Order {
            order_type: OrderType::StopSell,
            symbol: String::from("ABC"),
            shares: 100.0,
            price: Some(105.00),
        };

        let (mut orderbook, quote) = setup();
        orderbook.insert_order(&order);
        orderbook.insert_order(&order1);

        let res = orderbook.check_orders_by_symbol(&quote).unwrap();
        assert!(res.len().eq(&1));
    }

    #[test]
    fn test_that_orderbook_doesnt_load_market_orders() {
        let order = Order {
            order_type: OrderType::MarketBuy,
            symbol: String::from("ABC"),
            shares: 100.0,
            price: None,
        };

        let order1 = Order {
            order_type: OrderType::MarketSell,
            symbol: String::from("ABC"),
            shares: 100.0,
            price: None,
        };

        let (mut orderbook, _quote) = setup();
        orderbook.insert_order(&order);
        orderbook.insert_order(&order1);

        let res = orderbook.get_orders_by_symbol(&String::from("ABC"));
        assert!(res.len().eq(&0));
    }

    #[test]
    fn test_that_delete_and_insert_dont_conflict() {
        let order = Order {
            order_type: OrderType::LimitBuy,
            symbol: String::from("ABC"),
            shares: 100.0,
            price: Some(101.00),
        };

        let order1 = Order {
            order_type: OrderType::LimitBuy,
            symbol: String::from("ABC"),
            shares: 100.0,
            price: Some(105.00),
        };

        let (mut orderbook, _quote) = setup();
        orderbook.insert_order(&order);
        orderbook.delete_order(&1);
        orderbook.insert_order(&order1);

        let res = orderbook.get_orders_by_symbol(&String::from("ABC"));
        assert!(res.len().eq(&1));
    }

    #[test]
    fn test_that_orderbook_returns_order_creation_event_on_creating_good_order() {
        let order = Order {
            order_type: OrderType::LimitBuy,
            symbol: String::from("ABC"),
            shares: 100.0,
            price: Some(101.00),
        };

        let (mut orderbook, _quote) = setup();
        let res = orderbook.insert_order(&order);
        assert!(matches!(res, BrokerEvent::OrderCreated(..)));
    }

    #[test]
    fn test_that_orderbook_returns_order_failure_event_on_creating_bad_order() {
        let order = Order {
            order_type: OrderType::MarketBuy,
            symbol: String::from("ABC"),
            shares: 100.0,
            price: None,
        };

        let (mut orderbook, _quote) = setup();
        let res = orderbook.insert_order(&order);
        assert!(matches!(res, BrokerEvent::OrderFailure(..)));
    }
}