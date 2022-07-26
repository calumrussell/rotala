use core::panic;
use log::info;

use super::orderbook::SimOrderBook;
use crate::broker::record::BrokerLog;
use crate::broker::rules::OrderExecutionRules;
use crate::broker::{
    BrokerCost, BrokerEvent, CanUpdate, DividendPayment, EventLog, GetsQuote, PayDividend,
    PendingOrder, PositionInfo, Quote, Trade, TradeCost, TransferCash,
};
use crate::broker::{ExecutesOrder, Order, OrderType};
use crate::input::DataSource;
use crate::types::PortfolioValues;
use crate::types::{CashValue, DateTime, PortfolioHoldings, PortfolioQty, Price};

pub struct SimulatedBrokerBuilder<T: DataSource> {
    //Cannot run without data but can run with empty trade_costs
    data: Option<T>,
    trade_costs: Vec<BrokerCost>,
}

impl<T: DataSource> SimulatedBrokerBuilder<T> {
    pub fn build(&self) -> SimulatedBroker<T> {
        if self.data.is_none() {
            panic!("Cannot build broker without data");
        }
        let holdings = PortfolioHoldings::new();
        let orderbook = SimOrderBook::new();
        let log = BrokerLog::new();

        SimulatedBroker {
            data: self.data.as_ref().unwrap().clone(),
            //Intialised as invalid so errors throw if client tries to run before init
            holdings,
            orderbook,
            cash: CashValue::from(0.0),
            log,
            trade_costs: self.trade_costs.clone(),
        }
    }

    pub fn with_data(&mut self, data: T) -> &mut Self {
        self.data = Some(data);
        self
    }

    pub fn with_trade_costs(&mut self, trade_costs: Vec<BrokerCost>) -> &mut Self {
        self.trade_costs = trade_costs;
        self
    }

    pub fn new() -> Self {
        SimulatedBrokerBuilder {
            data: None,
            trade_costs: Vec::new(),
        }
    }
}

impl<T: DataSource> Default for SimulatedBrokerBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct SimulatedBroker<T: DataSource> {
    data: T,
    holdings: PortfolioHoldings,
    orderbook: SimOrderBook,
    cash: CashValue,
    log: BrokerLog,
    trade_costs: Vec<BrokerCost>,
}

impl<T: DataSource> SimulatedBroker<T> {
    pub fn cost_basis(&self, symbol: &str) -> Option<Price> {
        self.log.cost_basis(symbol)
    }

    fn check_orderbook(&mut self) {
        //Should always return because we are running after we set a new date
        info!("BROKER: Checking orderbook");
        if let Some(quotes) = self.get_quotes().cloned() {
            for quote in quotes {
                if let Some(active_orders) = self.orderbook.check_orders_by_symbol(&quote) {
                    for (order_id, order) in active_orders {
                        let order = match order.get_order_type() {
                            OrderType::LimitBuy | OrderType::StopBuy => Order::new(
                                OrderType::MarketBuy,
                                quote.symbol.clone(),
                                order.get_shares(),
                                None,
                            ),
                            OrderType::LimitSell | OrderType::StopSell => Order::new(
                                OrderType::MarketSell,
                                quote.symbol.clone(),
                                order.get_shares(),
                                None,
                            ),
                            _ => panic!("Orderbook should have only non-market orders"),
                        };
                        info!(
                            "BROKER: Checked orderbook attempting to execute order with id: {}",
                            &order_id
                        );
                        let order_result = self.execute_order(&order);
                        //TODO: orders fail silently if the market order can't be executed
                        if let BrokerEvent::TradeSuccess(_t) = order_result {
                            info!("BROKER: Orderbook order executed successfully, deleting order with id: {}", &order_id);
                            self.orderbook.delete_order(&order_id);
                        }
                    }
                }
            }
        }
    }

    //Contains tasks that should be run on every iteration of the simulation irregardless of the
    //state on the client.
    //Right now, this largely consists of actions that the broker needs to perform i.e. checking if
    //an order has been triggered.
    pub fn check(&mut self) {
        self.check_orderbook();
        self.pay_dividends();
    }
}

impl<T: DataSource> TransferCash for SimulatedBroker<T> {
    fn withdraw_cash(&mut self, cash: CashValue) -> BrokerEvent {
        if cash > self.cash {
            info!(
                "BROKER: Attempted cash withdraw of {:?} but only have {:?}",
                cash, self.cash
            );
            return BrokerEvent::WithdrawFailure(cash);
        }
        info!(
            "BROKER: Successful cash withdraw of {:?}, {:?} left in cash",
            cash, self.cash
        );
        self.cash -= cash;
        BrokerEvent::WithdrawSuccess(cash)
    }

    fn deposit_cash(&mut self, cash: CashValue) -> BrokerEvent {
        info!(
            "BROKER: Deposited {:?} cash, current balance of {:?}",
            cash, self.cash
        );
        self.cash += cash;
        BrokerEvent::DepositSuccess(cash)
    }

    //Identical to deposit_cash but is seperated to distinguish internal cash
    //transactions from external with no value returned to client
    fn credit(&mut self, value: CashValue) -> BrokerEvent {
        info!(
            "BROKER: Credited {:?} cash, current balance of {:?}",
            value, self.cash
        );
        self.cash += value;
        BrokerEvent::TransactionSuccess
    }

    //Looks similar to withdraw_cash but distinguished because it represents
    //failure of an internal transaction with no value returned to clients
    fn debit(&mut self, value: CashValue) -> BrokerEvent {
        if value > self.cash {
            info!(
                "BROKER: Debit failed of {:?} cash, current balance of {:?}",
                value, self.cash
            );
            return BrokerEvent::TransactionFailure;
        }
        info!(
            "BROKER: Debited {:?} cash, current balance of {:?}",
            value, self.cash
        );
        self.cash -= value;
        BrokerEvent::TransactionSuccess
    }

    fn get_cash_balance(&self) -> CashValue {
        self.cash
    }
}

impl<T: DataSource> PositionInfo for SimulatedBroker<T> {
    fn get_position_cost(&self, symbol: &str) -> Option<Price> {
        self.log.cost_basis(symbol)
    }

    fn get_position_profit(&self, symbol: &str) -> Option<CashValue> {
        if let Some(cost) = self.log.cost_basis(symbol) {
            if let Some(price) = self.get_quote(symbol) {
                //Once we get to this point we can unwrap safely
                let qty = *self.get_position_qty(symbol).unwrap();
                let profit: Price = if qty > 0.0 {
                    price.bid - cost
                } else {
                    price.ask - cost
                };
                //Profit in CashValue
                return Some(profit * qty);
            }
        }
        None
    }

    fn get_position_qty(&self, symbol: &str) -> Option<&PortfolioQty> {
        self.holdings.get(symbol)
    }

    fn get_position_liquidation_value(&self, symbol: &str) -> Option<CashValue> {
        //TODO: we need to introduce some kind of distinction between short and long
        //      positions.
        if let Some(quote) = self.get_quote(symbol) {
            let price = quote.bid;
            if let Some(qty) = self.get_position_qty(symbol) {
                let position_value = price * *qty;
                let (value_after_costs, _price_after_costs) =
                    self.calc_trade_impact(&position_value, &price, false);
                return Some(value_after_costs);
            }
        }
        None
    }

    fn get_position_value(&self, symbol: &str) -> Option<CashValue> {
        //TODO: we need to introduce some kind of distinction between short and long
        //      positions.
        if let Some(quote) = self.get_quote(symbol) {
            let price = quote.bid;
            if let Some(qty) = self.get_position_qty(symbol) {
                return Some(price * *qty);
            }
        }
        None
    }

    fn get_total_value(&self) -> CashValue {
        let assets = self.get_positions();
        let mut value = self.get_cash_balance();
        for a in assets {
            if let Some(position_value) = self.get_position_value(&a) {
                value += position_value
            }
        }
        value
    }

    fn get_liquidation_value(&self) -> CashValue {
        let mut value = self.get_cash_balance();
        for asset in self.get_positions() {
            if let Some(asset_value) = self.get_position_liquidation_value(&asset) {
                value += asset_value
            }
        }
        value
    }

    fn get_positions(&self) -> Vec<String> {
        self.holdings.keys()
    }

    fn get_holdings(&self) -> PortfolioHoldings {
        self.holdings.clone()
    }

    fn get_values(&self) -> PortfolioValues {
        let mut holdings = PortfolioValues::new();
        let assets = self.get_positions();
        for a in assets {
            let value = self.get_position_value(&a);
            if let Some(v) = value {
                holdings.insert(&a, &v);
            }
        }
        holdings
    }
}

impl<T: DataSource> GetsQuote for SimulatedBroker<T> {
    fn get_quote(&self, symbol: &str) -> Option<Quote> {
        self.data.get_quote(symbol)
    }

    fn get_quotes(&self) -> Option<&Vec<Quote>> {
        self.data.get_quotes()
    }
}

impl<T: DataSource> ExecutesOrder for SimulatedBroker<T> {
    fn execute_order(&mut self, order: &Order) -> BrokerEvent {
        info!(
            "BROKER: Attempting to execute {:?} order for {:?} shares of {:?}",
            order.get_order_type(),
            order.get_shares(),
            order.get_symbol()
        );
        if let OrderType::LimitBuy
        | OrderType::LimitSell
        | OrderType::StopBuy
        | OrderType::StopSell = order.get_order_type()
        {
            panic!("Can only call execute order with market orders")
        };

        if let Some(quote) = self.get_quote(&order.get_symbol()) {
            let price = match order.get_order_type() {
                OrderType::MarketBuy => quote.ask,
                OrderType::MarketSell => quote.bid,
                _ => unreachable!("Can only get here with market orders"),
            };
            let date = quote.date;

            match OrderExecutionRules::run_all(order, &price, &date, self) {
                Ok(trade) => {
                    let price = trade.value / trade.quantity;
                    info!("BROKER: Successfully executed {:?} trade for {:?} shares at {:?} in {:?} for total of {:?}", trade.typ, trade.quantity, price, trade.symbol, trade.value);
                    self.log.record(trade.clone());
                    BrokerEvent::TradeSuccess(trade)
                }
                Err(e) => e,
            }
        } else {
            panic!(
                "BROKER: Attempted to execute {:?} trade, no quote for {:?}",
                order.get_order_type(),
                order.get_symbol()
            );
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

impl<T: DataSource> PendingOrder for SimulatedBroker<T> {
    fn insert_order(&mut self, order: &Order) {
        info!(
            "BROKER: Attempting to insert {:?} order for {:?} shares of {:?} into orderbook",
            order.get_order_type(),
            order.get_shares(),
            order.get_symbol()
        );
        self.orderbook.insert_order(order);
    }

    fn delete_order(&mut self, order_id: &u8) {
        info!("BROKER: Deleting order_id {:?} from orderbook", order_id);
        self.orderbook.delete_order(order_id)
    }
}

impl<T: DataSource> CanUpdate for SimulatedBroker<T> {
    fn update_holdings(&mut self, symbol: &str, change: &PortfolioQty) {
        info!(
            "BROKER: Incrementing holdings in {:?} by {:?}",
            symbol, change
        );
        self.holdings.insert(symbol, &*change);
    }
}

impl<T: DataSource> TradeCost for SimulatedBroker<T> {
    fn get_trade_costs(&self, trade: &Trade) -> CashValue {
        let mut cost = CashValue::default();
        for trade_cost in &self.trade_costs {
            cost += trade_cost.calc(trade);
        }
        cost
    }

    fn calc_trade_impact(
        &self,
        budget: &CashValue,
        price: &Price,
        is_buy: bool,
    ) -> (CashValue, Price) {
        BrokerCost::trade_impact_total(&self.trade_costs, budget, price, is_buy)
    }
}

impl<T: DataSource> PayDividend for SimulatedBroker<T> {
    fn pay_dividends(&mut self) {
        info!("BROKER: Checking dividends");
        if let Some(dividends) = self.data.get_dividends() {
            for dividend in dividends.clone() {
                //Our dataset can include dividends for stocks we don't own so we need to check
                //that we own the stock, not performant but can be changed later
                if let Some(qty) = self.get_position_qty(&dividend.symbol) {
                    info!(
                        "BROKER: Found dividend of {:?} for portfolio holding {:?}",
                        dividend.value, dividend.symbol
                    );
                    let cash_value = *qty * dividend.value;
                    self.credit(cash_value);
                    let dividend_paid = DividendPayment {
                        value: cash_value,
                        symbol: dividend.symbol.clone(),
                        date: dividend.date,
                    };
                    self.log.record(dividend_paid);
                }
            }
        }
    }
}

impl<T: DataSource> EventLog for SimulatedBroker<T> {
    fn trades_between(&self, start: &DateTime, end: &DateTime) -> Vec<Trade> {
        self.log.trades_between(start, end)
    }

    fn dividends_between(&self, start: &DateTime, end: &DateTime) -> Vec<DividendPayment> {
        self.log.dividends_between(start, end)
    }
}

#[cfg(test)]
mod tests {

    use super::{PendingOrder, SimulatedBroker, SimulatedBrokerBuilder};
    use crate::broker::{BrokerCost, BrokerEvent, Dividend, PositionInfo, Quote, TransferCash};
    use crate::broker::{ExecutesOrder, Order, OrderType};
    use crate::clock::{Clock, ClockBuilder};
    use crate::input::{HashMapInput, HashMapInputBuilder};
    use crate::types::DateTime;

    use std::collections::HashMap;
    use std::rc::Rc;

    fn setup() -> (SimulatedBroker<HashMapInput>, Clock) {
        let mut prices: HashMap<DateTime, Vec<Quote>> = HashMap::new();
        let mut dividends: HashMap<DateTime, Vec<Dividend>> = HashMap::new();
        let quote = Quote {
            bid: 100.00.into(),
            ask: 101.00.into(),
            date: 100.into(),
            symbol: String::from("ABC"),
        };
        let quote1 = Quote {
            bid: 10.00.into(),
            ask: 11.00.into(),
            date: 100.into(),
            symbol: String::from("BCD"),
        };
        let quote2 = Quote {
            bid: 104.00.into(),
            ask: 105.00.into(),
            date: 101.into(),
            symbol: String::from("ABC"),
        };
        let quote3 = Quote {
            bid: 14.00.into(),
            ask: 15.00.into(),
            date: 101.into(),
            symbol: String::from("BCD"),
        };
        let quote4 = Quote {
            bid: 95.00.into(),
            ask: 96.00.into(),
            date: 102.into(),
            symbol: String::from("ABC"),
        };
        let quote5 = Quote {
            bid: 10.00.into(),
            ask: 11.00.into(),
            date: 102.into(),
            symbol: String::from("BCD"),
        };

        prices.insert(100.into(), vec![quote, quote1]);
        prices.insert(101.into(), vec![quote2, quote3]);
        prices.insert(102.into(), vec![quote4, quote5]);

        let divi1 = Dividend {
            value: 5.0.into(),
            symbol: String::from("ABC"),
            date: 101.into(),
        };
        dividends.insert(101.into(), vec![divi1]);

        let clock = ClockBuilder::from_fixed(100.into(), 102.into()).every();

        let source = HashMapInputBuilder::new()
            .with_quotes(prices)
            .with_dividends(dividends)
            .with_clock(Rc::clone(&clock))
            .build();

        let brkr = SimulatedBrokerBuilder::new()
            .with_data(source)
            .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
            .build();
        (brkr, clock)
    }

    #[test]
    fn test_cash_deposit_withdraw() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(100.0.into());
        clock.borrow_mut().tick();

        //Test cash
        assert!(matches!(
            brkr.withdraw_cash(50.0.into()),
            BrokerEvent::WithdrawSuccess(..)
        ));
        assert!(matches!(
            brkr.withdraw_cash(51.0.into()),
            BrokerEvent::WithdrawFailure(..)
        ));
        assert!(matches!(
            brkr.deposit_cash(50.0.into()),
            BrokerEvent::DepositSuccess(..)
        ));

        //Test transactions
        assert!(matches!(
            brkr.debit(50.0.into()),
            BrokerEvent::TransactionSuccess
        ));
        assert!(matches!(
            brkr.debit(51.0.into()),
            BrokerEvent::TransactionFailure
        ));
        assert!(matches!(
            brkr.credit(50.0.into()),
            BrokerEvent::TransactionSuccess
        ));
    }

    #[test]
    fn test_that_successful_market_buy_order_reduces_cash() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(100_000.0.into());
        clock.borrow_mut().tick();

        let order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            495.00.into(),
            None,
        );
        let _res = brkr.execute_order(&order);

        let cash = brkr.get_cash_balance();
        assert!(cash < 100_000.0);
    }

    #[test]
    fn test_that_buy_order_larger_than_cash_fails_with_error_returned_without_panic() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(100.0.into());
        clock.borrow_mut().tick();

        let order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            //Order value is greater than cash balance
            495.00.into(),
            None,
        );
        let res = brkr.execute_order(&order);

        let cash = brkr.get_cash_balance();

        assert!(cash == 100.0);
        assert!(matches!(res, BrokerEvent::TradeFailure(..)));
    }

    #[test]
    fn test_that_sell_order_larger_than_holding_fails_with_error_returned_without_panic() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(100_000.0.into());
        let order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            100.0.into(),
            None,
        );
        brkr.execute_order(&order);
        clock.borrow_mut().tick();

        let order1 = Order::new(
            OrderType::MarketSell,
            String::from("ABC"),
            //Order qty greater than current holding
            105.0.into(),
            None,
        );
        let res = brkr.execute_order(&order1);
        println!("{:?}", res);
        assert!(matches!(res, BrokerEvent::TradeFailure(..)));
        let qty = brkr.get_position_qty("ABC").unwrap();
        println!("{:?}", qty);
        assert!(*qty == 100.0);
    }

    #[test]
    fn test_that_market_buy_increases_holdings() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(100_000.0.into());
        clock.borrow_mut().tick();

        let order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            495.00.into(),
            None,
        );
        let _res = brkr.execute_order(&order);

        let qty = *brkr.get_position_qty(&String::from("ABC")).unwrap();
        assert!(qty == 495.00);
    }

    #[test]
    fn test_that_market_sell_decreases_holdings() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(100_000.0.into());
        clock.borrow_mut().tick();

        let order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            495.00.into(),
            None,
        );
        let _res = brkr.execute_order(&order);

        let order1 = Order::new(
            OrderType::MarketSell,
            String::from("ABC"),
            295.00.into(),
            None,
        );
        let _res1 = brkr.execute_order(&order1);

        let qty = *brkr.get_position_qty(&String::from("ABC")).unwrap();
        assert!(qty == 200.00);
    }

    #[test]
    fn test_that_limit_order_increases_holdings_when_price_hits() {
        //This shouldn't just trigger but we check that the order executes at the market price, not
        //the price of the limit order

        let (mut brkr, clock) = setup();
        brkr.deposit_cash(100_000.0.into());

        let order = Order::new(
            OrderType::LimitBuy,
            String::from("ABC"),
            495.00.into(),
            Some(102.00.into()),
        );
        brkr.insert_order(&order);
        clock.borrow_mut().tick();
        brkr.check();

        let qty = *brkr.get_position_qty(&String::from("ABC")).unwrap();
        let cost = brkr.cost_basis(&String::from("ABC")).unwrap();
        println!("{:?}", cost);
        println!("{:?}", qty);
        assert!(qty == 495.00);
        assert!(cost == 105.00);
    }

    #[test]
    fn test_that_stop_order_decreases_holdings_when_price_hits() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(100_000.0.into());
        clock.borrow_mut().tick();
        brkr.check();

        let entry_order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            500.0.into(),
            None,
        );
        let _res = brkr.execute_order(&entry_order);

        let stop_order = Order::new(
            OrderType::StopSell,
            String::from("ABC"),
            500.0.into(),
            Some(98.0.into()),
        );
        let _res1 = brkr.insert_order(&stop_order);
        clock.borrow_mut().tick();
        brkr.check();

        let qty = *brkr.get_position_qty(&String::from("ABC")).unwrap();
        assert!(qty == 0.0);
    }

    #[test]
    fn test_that_valuation_updates_in_next_period() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(100_000.0.into());
        clock.borrow_mut().tick();

        let order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            495.0.into(),
            None,
        );
        let _res = brkr.execute_order(&order);

        let val = brkr.get_position_value(&String::from("ABC")).unwrap();
        clock.borrow_mut().tick();
        let val1 = brkr.get_position_value(&String::from("ABC")).unwrap();
        assert_ne!(val, val1);
    }

    #[test]
    fn test_that_profit_calculation_is_accurate() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(100_000.0.into());
        let order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            495.0.into(),
            None,
        );
        brkr.execute_order(&order);
        clock.borrow_mut().tick();
        clock.borrow_mut().tick();

        let profit = brkr.get_position_profit(&String::from("ABC")).unwrap();
        assert!(profit == -2970.00);
    }

    #[test]
    fn test_that_dividends_are_paid() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(100_000.0.into());
        let order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            100.0.into(),
            None,
        );
        brkr.execute_order(&order);
        let cash_before_dividend = brkr.get_cash_balance();
        clock.borrow_mut().tick();
        brkr.check();

        let cash_after_dividend = brkr.get_cash_balance();
        assert!(cash_before_dividend != cash_after_dividend);
    }

    #[test]
    #[should_panic]
    fn test_that_broker_builder_fails_without_data() {
        let _brkr = SimulatedBrokerBuilder::<HashMapInput>::new()
            .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
            .build();
    }

    #[test]
    fn test_that_broker_build_passes_without_trade_costs() {
        let mut prices: HashMap<DateTime, Vec<Quote>> = HashMap::new();

        let quote = Quote {
            bid: 100.00.into(),
            ask: 101.00.into(),
            date: 100.into(),
            symbol: String::from("ABC"),
        };
        let quote2 = Quote {
            bid: 104.00.into(),
            ask: 105.00.into(),
            date: 101.into(),
            symbol: String::from("ABC"),
        };
        let quote4 = Quote {
            bid: 95.00.into(),
            ask: 96.00.into(),
            date: 102.into(),
            symbol: String::from("ABC"),
        };
        prices.insert(100.into(), vec![quote]);
        prices.insert(101.into(), vec![quote2]);
        prices.insert(102.into(), vec![quote4]);

        let clock = ClockBuilder::from_fixed(100.into(), 102.into()).every();

        let source = HashMapInputBuilder::new()
            .with_quotes(prices)
            .with_clock(Rc::clone(&clock))
            .build();

        let _brkr = SimulatedBrokerBuilder::new().with_data(source).build();
    }

    #[test]
    #[should_panic]
    fn test_that_broker_panics_if_client_attempts_to_trade_nonexistent_stock() {
        //If the client attempts to pass orders for companies for which there is no data we want to
        //panic. Whilst this condition is harsh for any live trading environment, it makes sense
        //here because it likely means that the client will be passed a result which makes no sense
        //in return
        //May change in future
        let (mut brkr, _clock) = setup();
        brkr.deposit_cash(100_000.0.into());
        let order = Order::new(
            OrderType::MarketBuy,
            //Non-existent ticker
            String::from("XYZ"),
            100.0.into(),
            None,
        );
        brkr.execute_order(&order);
    }
}
