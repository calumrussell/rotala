use core::panic;
use std::collections::HashMap;


use crate::broker::book::SimOrderBook;
use crate::broker::execution::OrderExecutionRules;
use crate::broker::order::{Order, OrderExecutor, OrderType};
use crate::broker::record::TradeRecord;
use crate::broker::{
    BrokerEvent, CashManager, ClientControlled, Holdings, PendingOrders, PositionInfo, PriceQuote,
    Quote, Trade, TradeLedger,
};
use crate::data::{DataSource, SimSource};

#[derive(Clone)]
pub struct SimulatedBroker {
    pub holdings: Holdings,
    simapi: BrokerSimAPI,
    pub orderbook: SimOrderBook,
    pub cash: f64,
    pub ledger: TradeRecord,
}

impl CashManager for SimulatedBroker {
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

    fn credit(&mut self, value: f64) -> BrokerEvent {
        self.cash += value;
        BrokerEvent::CashTransactionSuccess(value)
    }

    fn debit(&mut self, value: f64) -> BrokerEvent {
        if value > self.cash {
            return BrokerEvent::InsufficientCash(value);
        }
        self.cash -= value;
        BrokerEvent::CashTransactionSuccess(value)
    }

    fn get_cash_balance(&self) -> f64 {
        self.cash
    }
}

impl PositionInfo for SimulatedBroker {
    fn get_position_cost(&self, symbol: &String) -> Option<f64> {
        self.ledger.cost_basis(symbol)
    }

    fn get_position_profit(&self, symbol: &String) -> Option<f64> {
        let cost = self.ledger.cost_basis(symbol);
        let price = self.get_quote(symbol);
        if cost.is_some() && price.is_some() {
            let qty = self.get_position_qty(symbol).unwrap();
            if qty > 0.0 {
                let profit = price.unwrap().bid - cost.unwrap();
                return Some(profit * qty);
            } else {
                let profit = price.unwrap().ask - cost.unwrap();
                return Some(profit * qty);
            }
        }
        None
    }

    fn get_position_qty(&self, symbol: &String) -> Option<f64> {
        let pos = self.holdings.0.get(symbol);
        match pos {
            Some(p) => Some(p.clone()),
            _ => None,
        }
    }

    fn get_position_value(&self, symbol: &String) -> Option<f64> {
        let quote = self.get_quote(symbol);
        //TODO: we need to introduce some kind of distinction between short and long
        //      positions.

        if quote.is_some() {
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

impl PriceQuote for SimulatedBroker {
    fn get_quote(&self, symbol: &String) -> Option<Quote> {
        self.simapi.get_prices(symbol)
    }
}

impl OrderExecutor for SimulatedBroker {
    fn execute_order(&mut self, order: &Order) -> BrokerEvent {
        if let OrderType::LimitBuy
        | OrderType::LimitSell
        | OrderType::StopBuy
        | OrderType::StopSell = order.order_type
        {
            panic!("Can only call execute order with market orders")
        };

        let quote = self.get_quote(&order.symbol);
        if quote.is_none() {
            return BrokerEvent::TradeFailure(order.clone());
        }

        let price = match order.order_type {
            OrderType::MarketBuy => quote.unwrap().ask,
            OrderType::MarketSell => quote.unwrap().bid,
            _ => unreachable!("Can only get here with market orders"),
        };

        //OrderExecutionRules returns a closure with the execution logic over the
        //result, precaution as the actual execution logic should be run from here
        let res = OrderExecutionRules::run_all(order, &price, self);
        match res {
            Ok(trade_func) => {
                let t = trade_func();
                return BrokerEvent::TradeSuccess(t);
            }
            Err(e) => {
                return e;
            }
        }
    }

    fn execute_orders(&mut self, orders: Vec<Order>) -> Vec<BrokerEvent> {
        let mut res = Vec::new();
        for o in orders {
            let trade = self.execute_order(&o);
            res.push(trade);
        }
        res
    }
}

impl PendingOrders for SimulatedBroker {
    fn insert_order(&mut self, order: &Order) {
        self.orderbook.insert_order(order);
    }

    fn delete_order(&mut self, order_id: &u8) {
        self.orderbook.delete_order(order_id)
    }
}

impl ClientControlled for SimulatedBroker {
    fn get_holdings(&self) -> &Holdings {
        &self.holdings
    }

    fn get(&self, symbol: &String) -> Option<&f64> {
        self.holdings.0.get(symbol)
    }

    fn update_holdings(&mut self, symbol: &String, change: &f64) {
        self.holdings.0.insert(symbol.clone(), *change);
    }
}

impl TradeLedger for SimulatedBroker {
    fn record(&mut self, trade: &Trade) {
        self.ledger.record(trade);
    }

    fn cost_basis(&self, symbol: &String) -> Option<f64> {
        self.ledger.cost_basis(symbol)
    }
}

impl SimulatedBroker {
    fn check_orderbook(&mut self) {
        //Should always return because we are running after we set a new date
        let quotes = self.simapi.get_all_prices();
        for quote in quotes {
            let pending_orders = self.orderbook.check_orders_by_symbol(&quote);
            if pending_orders.is_some() {
                let active_orders = pending_orders.unwrap();
                for (order_id, order) in active_orders {
                    let order = match order.order_type {
                        OrderType::LimitBuy | OrderType::StopBuy => Order {
                            order_type: OrderType::MarketBuy,
                            symbol: quote.symbol.clone(),
                            shares: order.shares,
                            price: None,
                        },
                        OrderType::LimitSell | OrderType::StopSell => Order {
                            order_type: OrderType::MarketSell,
                            symbol: quote.symbol.clone(),
                            shares: order.shares,
                            price: None,
                        },
                        _ => panic!("Orderbook should have only non-market orders"),
                    };
                    let order_result = self.execute_order(&order);
                    //TODO: orders fail silently if the market order can't be executed
                    if let BrokerEvent::TradeSuccess(_t) = order_result {
                        self.orderbook.delete_order(&order_id);
                    }
                }
            }
        }
    }

    pub fn set_date(&mut self, new_date: &i64) {
        self.simapi.set_date(new_date);
        self.check_orderbook();
    }

    pub fn new(raw_data: DataSource) -> SimulatedBroker {
        let holdings_data: HashMap<String, f64> = HashMap::new();
        let holdings = Holdings(holdings_data);
        let orderbook = SimOrderBook::new();

        let ledger = TradeRecord::new();
        let simapi = BrokerSimAPI::new(raw_data);

        SimulatedBroker {
            simapi,
            holdings,
            orderbook,
            cash: 0.0,
            ledger,
        }
    }
}

trait Prices {
    fn get_prices(&self, symbol: &String) -> Option<Quote>;
    fn get_all_prices(&self) -> Vec<Quote>;
}

#[derive(Clone)]
struct BrokerSimAPI {
    raw_data: DataSource,
    date: i64,
}

impl Prices for BrokerSimAPI {
    fn get_prices(&self, symbol: &String) -> Option<Quote> {
        let quote = self.raw_data.get_date_symbol(&self.date, symbol);
        match quote {
            Ok(q) => Some(q),
            _ => None,
        }
    }

    //Returns a copy so that we don't need a mutable reference to the underlying data
    fn get_all_prices(&self) -> Vec<Quote> {
        let mut res: Vec<Quote> = Vec::new();
        let prices = self.raw_data.get_date(&self.date);
        if prices.is_some() {
            for price in prices.unwrap() {
                res.push(price.clone());
            }
        }
        res
    }
}

impl BrokerSimAPI {
    pub fn set_date(&mut self, date: &i64) {
        self.date = date.clone();
    }

    pub fn new(raw_data: DataSource) -> Self {
        BrokerSimAPI { raw_data, date: -1 }
    }
}

#[cfg(test)]
mod tests {

    use super::{PendingOrders, SimulatedBroker};
    use crate::broker::order::{Order, OrderExecutor, OrderType};
    use crate::broker::{BrokerEvent, CashManager, PositionInfo, Quote, TradeLedger};
    use crate::data::DataSource;

    use std::collections::HashMap;

    fn setup() -> (SimulatedBroker, i64) {
        let mut prices: HashMap<i64, Vec<Quote>> = HashMap::new();

        let mut price_row: Vec<Quote> = Vec::new();
        let mut price_row1: Vec<Quote> = Vec::new();
        let mut price_row2: Vec<Quote> = Vec::new();
        let quote = Quote {
            bid: 100.00,
            ask: 101.00,
            date: 100,
            symbol: String::from("ABC"),
        };
        let quote1 = Quote {
            bid: 10.00,
            ask: 11.00,
            date: 100,
            symbol: String::from("BCD"),
        };
        let quote2 = Quote {
            bid: 104.00,
            ask: 105.00,
            date: 101,
            symbol: String::from("ABC"),
        };
        let quote3 = Quote {
            bid: 14.00,
            ask: 15.00,
            date: 101,
            symbol: String::from("BCD"),
        };
        let quote4 = Quote {
            bid: 95.00,
            ask: 96.00,
            date: 101,
            symbol: String::from("ABC"),
        };
        let quote5 = Quote {
            bid: 10.00,
            ask: 11.00,
            date: 101,
            symbol: String::from("BCD"),
        };

        price_row.push(quote);
        price_row.push(quote1);
        price_row1.push(quote2);
        price_row1.push(quote3);
        price_row2.push(quote4);
        price_row2.push(quote5);

        prices.insert(100, price_row);
        prices.insert(101, price_row1);
        prices.insert(102, price_row2);

        let source = DataSource::from_hashmap(prices);
        let brkr = SimulatedBroker::new(source);
        (brkr, 10)
    }

    #[test]
    fn test_that_successful_market_buy_order_reduces_cash() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.00);
        brkr.set_date(&100);

        let order = Order {
            order_type: OrderType::MarketBuy,
            symbol: String::from("ABC"),
            shares: 495.00,
            price: None,
        };

        let _res = brkr.execute_order(&order);

        let cash = brkr.get_cash_balance();
        assert!(cash == 50_005.00);
    }

    #[test]
    fn test_that_order_fails_without_cash_bubbling_correct_error() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100.00);
        brkr.set_date(&100);

        let order = Order {
            order_type: OrderType::MarketBuy,
            symbol: String::from("ABC"),
            shares: 495.00,
            price: None,
        };

        let res = brkr.execute_order(&order);

        let cash = brkr.get_cash_balance();
        assert!(cash == 100.00);
        assert!(matches!(res, BrokerEvent::InsufficientCash(..)));
    }

    #[test]
    fn test_that_market_buy_increases_holdings() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.00);
        brkr.set_date(&100);

        let order = Order {
            order_type: OrderType::MarketBuy,
            symbol: String::from("ABC"),
            shares: 495.00,
            price: None,
        };

        let _res = brkr.execute_order(&order);

        let qty = brkr.get_position_qty(&String::from("ABC")).unwrap();
        assert!(qty == 495.00);
    }

    #[test]
    fn test_that_market_sell_decreases_holdings() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.00);
        brkr.set_date(&100);

        let order = Order {
            order_type: OrderType::MarketBuy,
            symbol: String::from("ABC"),
            shares: 495.00,
            price: None,
        };

        let _res = brkr.execute_order(&order);

        let order1 = Order {
            order_type: OrderType::MarketSell,
            symbol: String::from("ABC"),
            shares: 295.00,
            price: None,
        };
        let _res1 = brkr.execute_order(&order1);

        let qty = brkr.get_position_qty(&String::from("ABC")).unwrap();
        assert!(qty == 200.00);
    }

    #[test]
    fn test_that_limit_order_increases_holdings_when_price_hits() {
        //This shouldn't just trigger but we must check that the
        //order executes at the market price, not the price of the limit
        //order

        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.00);
        brkr.set_date(&100);

        let order = Order {
            order_type: OrderType::LimitBuy,
            symbol: String::from("ABC"),
            shares: 495.00,
            price: Some(102.00),
        };

        let _res = brkr.insert_order(&order);

        brkr.set_date(&101);

        let qty = brkr.get_position_qty(&String::from("ABC")).unwrap();
        let cost = brkr.cost_basis(&String::from("ABC")).unwrap();
        assert!(qty == 495.00);
        assert!(cost == 105.00);
    }

    #[test]
    fn test_that_stop_order_decreases_holdings_when_price_hits() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.00);
        brkr.set_date(&100);

        let entry_order = Order {
            order_type: OrderType::MarketBuy,
            symbol: String::from("ABC"),
            shares: 500.0,
            price: None,
        };

        let _res = brkr.execute_order(&entry_order);

        let stop_order = Order {
            order_type: OrderType::StopSell,
            symbol: String::from("ABC"),
            shares: 500.0,
            price: Some(98.0),
        };

        let _res1 = brkr.insert_order(&stop_order);
        brkr.set_date(&101);
        brkr.set_date(&102);
        brkr.set_date(&103);

        let qty = brkr.get_position_qty(&String::from("ABC")).unwrap();
        assert!(qty == 0.0);
    }

    #[test]
    fn test_that_valuation_updates_in_next_period() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.00);
        brkr.set_date(&100);

        let order = Order {
            order_type: OrderType::MarketBuy,
            symbol: String::from("ABC"),
            shares: 495.00,
            price: None,
        };

        let _res = brkr.execute_order(&order);

        let val = brkr.get_position_value(&String::from("ABC")).unwrap();
        brkr.set_date(&101);
        let val1 = brkr.get_position_value(&String::from("ABC")).unwrap();
        assert_ne!(val, val1);
    }

    #[test]
    fn test_that_profit_calculation_is_accurate() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.00);
        brkr.set_date(&100);

        let order = Order {
            order_type: OrderType::MarketBuy,
            symbol: String::from("ABC"),
            shares: 495.00,
            price: None,
        };

        let _res = brkr.execute_order(&order);

        brkr.set_date(&101);
        let profit = brkr.get_position_profit(&String::from("ABC")).unwrap();
        assert!(profit == 1485.00);
    }
}
