use core::panic;
use log::info;

use crate::broker::record::BrokerLog;
use crate::broker::{
    BacktestBroker, BrokerCalculations, BrokerCashEvent, BrokerCost, BrokerEvent, DividendPayment,
    EventLog, GetsQuote, Order, OrderType, Quote, Trade, TradeType, TransferCash, Dividend,
};
use crate::exchange::{DefaultExchange, Exchange};
use crate::input::DataSource;
use crate::types::{CashValue, PortfolioHoldings, PortfolioQty, Price};

pub struct SimulatedBrokerBuilder<T> where 
 T: DataSource<Quote, Dividend> {
    //Cannot run without data but can run with empty trade_costs
    data: Option<T>,
    trade_costs: Vec<BrokerCost>,
    exchange: Option<DefaultExchange<T>>,
}

impl<T> SimulatedBrokerBuilder<T> where
 T: DataSource<Quote, Dividend> {
    pub fn build(&self) -> SimulatedBroker<T> {
        if self.data.is_none() {
            panic!("Cannot build broker without data");
        }

        if self.exchange.is_none() {
            panic!("Cannot build broker without exchange");
        }

        let holdings = PortfolioHoldings::new();
        let log = BrokerLog::new();

        SimulatedBroker {
            data: self.data.as_ref().unwrap().clone(),
            //Intialised as invalid so errors throw if client tries to run before init
            holdings,
            cash: CashValue::from(0.0),
            log,
            trade_costs: self.trade_costs.clone(),
            exchange: self.exchange.as_ref().unwrap().clone(),
            //Initialized as ready because there is no state to catch up with when we create it
            ready_state: SimulatedBrokerReadyState::Ready,
        }
    }

    pub fn with_exchange(&mut self, exchange: DefaultExchange<T>) -> &mut Self {
        self.exchange = Some(exchange);
        self
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
            exchange: None,
        }
    }
}

impl<T> Default for SimulatedBrokerBuilder<T> where
 T: DataSource<Quote, Dividend> {
    fn default() -> Self {
        Self::new()
    }
}

///Broker implementation that can be used to replicate the execution logic and data structures of a
///broker. Created through the Builder struct, and requires an implementation of `DataSource` to
///run correctly.
///
///Orders are executed through a seperate exchange implementation that is held on the broker
///implementation. When a broker receives an order from the client, this order cannot be executed
///immediately but is sent to an exchange that executes the order in the next period for which
///there is a price. This structure ensures that clients cannot lookahead.
///
///Supports multiple `BrokerCost` models defined in broker/mod.rs: Flat, PerShare, PctOfValue.
///
///Cash balance held in single currency, which is assumed to be the same currency used in all
///quotes found in the implementation of `DataSource` passed to `SimulatedBroker`. Cash balance can
///be negative due to the non-immediate execution of trades. Broker will try to re-balance
///automatically.
///
///If series has a lot of volatility between periods, this will cause unexpected outcomes as
///the broker tries to continuously rebalance the negative cash balance.
///
///Broker is initialized with a cash buffer. When making transactions to raise cash, this is the
///value that will be targeted (as opposed to zero).
///
///Keeps an internal log of trades executed and dividends received/paid. The events supported by
///the `BrokerLog` are stored in the `BrokerRecordedEvent` enum in broker/mod.rs.
#[derive(Clone, Debug)]
pub struct SimulatedBroker<T> where 
 T: DataSource<Quote, Dividend> {
    //We have overlapping functionality because we are storing
    data: T,
    holdings: PortfolioHoldings,
    cash: CashValue,
    log: BrokerLog,
    trade_costs: Vec<BrokerCost>,
    exchange: DefaultExchange<T>,
    ready_state: SimulatedBrokerReadyState,
}

impl<T> SimulatedBroker<T> where
 T: DataSource<Quote, Dividend> {
    pub fn cost_basis(&self, symbol: &str) -> Option<Price> {
        self.log.cost_basis(symbol)
    }

    //Contains tasks that should be run on every iteration of the simulation irregardless of the
    //state on the client.
    pub fn check(&mut self) {
        match self.ready_state {
            SimulatedBrokerReadyState::Ready => {
                //This happens when the client does not call finish to close a transaction before
                //calling check for the next transaction
                panic!("Cannot call check on a broker that is already ready");
            }
            SimulatedBrokerReadyState::Invalid => {
                self.ready_state = SimulatedBrokerReadyState::Ready;
                info!("BROKER: Moved into Ready state");
                self.pay_dividends();
                self.exchange.check();
                //Reconcile must come after check so we can immediately reconcile the state of the
                //exchange with the broker
                self.reconcile_exchange();
                //Previous step can cause negative cash balance so we have to rebalance here, this
                //is not instant so will never balance properly if the series is very volatile
                self.rebalance_cash();
            }
            SimulatedBrokerReadyState::InsufficientCash => {
                //Make sure that the exchange state matches this by removing all unexecuted orders
                self.exchange.clear();
            }
        }
    }

    pub fn finish(&mut self) {
        self.exchange.finish();
        self.ready_state = SimulatedBrokerReadyState::Invalid;
    }

    fn reconcile_exchange(&mut self) {
        //All trades executed since the last call to this function
        let executed_trades = self.exchange.flush_buffer();
        for trade in &executed_trades {
            //TODO: if cash is below zero, we end the simulation. In practice, this shouldn't cause
            //problems because the broker will be unable to fund any future trades but exiting
            //early will give less confusing output.
            match trade.typ {
                //Force debit so we can end up with negative cash here
                TradeType::Buy => self.debit_force(&trade.value),
                TradeType::Sell => self.credit(&trade.value),
            };
            self.log.record(trade.clone());

            let default = PortfolioQty::from(0.0);
            let curr_position = self.get_position_qty(&trade.symbol).unwrap_or(&default);

            let updated = match trade.typ {
                TradeType::Buy => **curr_position + *trade.quantity,
                TradeType::Sell => **curr_position - *trade.quantity,
            };

            self.update_holdings(&trade.symbol, PortfolioQty::from(updated));
        }
    }

    fn rebalance_cash(&mut self) {
        //Has to be less than, we can have zero value without needing to liquidate if we initialize
        //the portfolio but exchange doesn't execute any trades. This can happen if we are missing
        //prices at the start of the series
        if *self.cash < 0.0 {
            let shortfall = *self.cash * -1.0;
            //When we raise cash, we try to raise a small amount more to stop continuous
            //rebalancing, this amount is arbitrary atm
            let plus_buffer = shortfall + 1000.0;
            let res = BrokerCalculations::withdraw_cash_with_liquidation(&plus_buffer, self);
            if let BrokerCashEvent::WithdrawFailure(_val) = res {
                info!("BROKER: Moved into InsufficientCash state");
                self.ready_state = SimulatedBrokerReadyState::InsufficientCash;
            }
        }
    }
}

impl<T> BacktestBroker for SimulatedBroker<T> where
 T: DataSource<Quote, Dividend> {
    //Identical to deposit_cash but is seperated to distinguish internal cash
    //transactions from external with no value returned to client
    fn credit(&mut self, value: &f64) -> BrokerCashEvent {
        match self.ready_state {
            SimulatedBrokerReadyState::Ready => {
                info!(
                    "BROKER: Credited {:?} cash, current balance of {:?}",
                    value, self.cash
                );
                self.cash = CashValue::from(*value + *self.cash);
                BrokerCashEvent::DepositSuccess(CashValue::from(*value))
            }
            SimulatedBrokerReadyState::Invalid => {
                panic!("Attempted to credit cash before state update");
            }
            SimulatedBrokerReadyState::InsufficientCash => {
                //If the broker is in this state, then it is possible that the broker has
                //insufficient cash and could be returned to a valid state with more cash.
                //We let the transaction go through, checks elsewhere will work out whether the
                //broker has been successfully moved out of this state.
                self.cash = CashValue::from(value + *self.cash);
                BrokerCashEvent::DepositSuccess(CashValue::from(*value))
            }
        }
    }

    //Looks similar to withdraw_cash but distinguished because it represents
    //failure of an internal transaction with no value returned to clients
    fn debit(&mut self, value: &f64) -> BrokerCashEvent {
        match self.ready_state {
            SimulatedBrokerReadyState::Ready => {
                if value > &self.cash {
                    info!(
                        "BROKER: Debit failed of {:?} cash, current balance of {:?}",
                        value, self.cash
                    );
                    return BrokerCashEvent::WithdrawFailure(CashValue::from(*value));
                }
                info!(
                    "BROKER: Debited {:?} cash, current balance of {:?}",
                    value, self.cash
                );
                self.cash = CashValue::from(*self.cash - *value);
                BrokerCashEvent::WithdrawSuccess(CashValue::from(*value))
            }
            SimulatedBrokerReadyState::Invalid => {
                panic!("Attempted to debit cash before state update");
            }
            SimulatedBrokerReadyState::InsufficientCash => {
                BrokerCashEvent::WithdrawFailure(CashValue::from(*value))
            }
        }
    }

    fn debit_force(&mut self, value: &f64) -> BrokerCashEvent {
        match self.ready_state {
            SimulatedBrokerReadyState::Ready => {
                info!(
                    "BROKER: Force debt {:?} cash, current balance of {:?}",
                    value, self.cash
                );
                self.cash = CashValue::from(*self.cash - *value);
                BrokerCashEvent::WithdrawSuccess(CashValue::from(*value))
            }
            SimulatedBrokerReadyState::Invalid => {
                panic!("Attempt to debit cash before state update");
            }
            SimulatedBrokerReadyState::InsufficientCash => {
                //Because this state is technically recoverable, we deduct the cash
                self.cash = CashValue::from(*self.cash - *value);
                BrokerCashEvent::WithdrawSuccess(CashValue::from(*value))
            }
        }
    }

    fn get_cash_balance(&self) -> CashValue {
        self.cash.clone()
    }

    //This method used to mut because we needed to sort last prices on the broker, this has now
    //been moved to the exchange. The exchange is responsible for storing last prices for cases
    //when a quote is missing.
    fn get_position_value(&self, symbol: &str) -> Option<CashValue> {
        //TODO: we need to introduce some kind of distinction between short and long
        //      positions.

        if let Some(quote) = self.get_quote(symbol) {
            //We only have long positions so we only need to look at the bid
            let price = &quote.bid;
            if let Some(qty) = self.get_position_qty(symbol) {
                let val = **price * **qty;
                return Some(CashValue::from(val));
            }
        }
        //This should only occur in cases when the client erroneously asks for a security with no
        //current or historical prices, which should never happen for a security in the portfolio.
        //This path likely represent an error in the application code so may panic here in future.
        None
    }

    fn get_position_cost(&self, symbol: &str) -> Option<Price> {
        self.log.cost_basis(symbol)
    }

    fn get_position_qty(&self, symbol: &str) -> Option<&PortfolioQty> {
        self.holdings.get(symbol)
    }

    fn get_positions(&self) -> Vec<String> {
        self.holdings.keys()
    }

    fn get_holdings(&self) -> PortfolioHoldings {
        self.holdings.clone()
    }

    fn update_holdings(&mut self, symbol: &str, change: PortfolioQty) {
        //This should never happen because update_holdings should only be called internally.
        match self.ready_state {
            SimulatedBrokerReadyState::Ready => {
                //We have to take ownership for logging but it is easier just to use ref for symbol as that
                //is used throughout
                let symbol_own = symbol.to_string();
                info!(
                    "BROKER: Incrementing holdings in {:?} by {:?}",
                    symbol_own, change
                );
                if (*change).eq(&0.0) {
                    self.holdings.remove(symbol.as_ref());
                } else {
                    self.holdings.insert(symbol.as_ref(), &change);
                }
            }
            SimulatedBrokerReadyState::Invalid => {
                panic!("Attempted to update holdings before calling check")
            }
            SimulatedBrokerReadyState::InsufficientCash => {}
        }
    }

    fn get_trade_costs(&self, trade: &Trade) -> CashValue {
        let mut cost = CashValue::default();
        for trade_cost in &self.trade_costs {
            cost = CashValue::from(*cost + *trade_cost.calc(trade));
        }
        cost
    }

    fn calc_trade_impact(&self, budget: &f64, price: &f64, is_buy: bool) -> (CashValue, Price) {
        BrokerCost::trade_impact_total(&self.trade_costs, budget, price, is_buy)
    }

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
                    let cash_value = CashValue::from(*qty.clone() * *dividend.value);
                    self.credit(&cash_value);
                    let dividend_paid = DividendPayment::new(
                        cash_value.clone(),
                        dividend.symbol.clone(),
                        dividend.date,
                    );
                    self.log.record(dividend_paid);
                }
            }
        }
    }

    fn send_order(&mut self, order: Order) -> BrokerEvent {
        match self.ready_state {
            SimulatedBrokerReadyState::Ready => {
                //This is an estimate of the cost based on the current price, can still end with negative
                //balance when we reconcile with actuals, may also reject valid orders at the margin
                info!(
                    "BROKER: Attempting to send {:?} order for {:?} shares of {:?} to the exchange",
                    order.get_order_type(),
                    order.get_shares(),
                    order.get_symbol()
                );

                let quote = self.get_quote(order.get_symbol()).unwrap();
                let price = match order.get_order_type() {
                    OrderType::MarketBuy | OrderType::LimitBuy | OrderType::StopBuy => &quote.ask,
                    OrderType::MarketSell | OrderType::LimitSell | OrderType::StopSell => {
                        &quote.bid
                    }
                };

                if let Err(_err) =
                    BrokerCalculations::client_has_sufficient_cash(&order, price, self)
                {
                    info!(
                        "BROKER: Unable to send {:?} order for {:?} shares of {:?} to exchange",
                        order.get_order_type(),
                        order.get_shares(),
                        order.get_symbol()
                    );
                    return BrokerEvent::OrderInvalid(order.clone());
                }
                if let Err(_err) =
                    BrokerCalculations::client_has_sufficient_holdings_for_sale(&order, self)
                {
                    info!(
                        "BROKER: Unable to send {:?} order for {:?} shares of {:?} to exchange",
                        order.get_order_type(),
                        order.get_shares(),
                        order.get_symbol()
                    );
                    return BrokerEvent::OrderInvalid(order.clone());
                }
                if let Err(_err) = BrokerCalculations::client_is_issuing_nonsense_order(&order) {
                    info!(
                        "BROKER: Unable to send {:?} order for {:?} shares of {:?} to exchange",
                        order.get_order_type(),
                        order.get_shares(),
                        order.get_symbol()
                    );
                    return BrokerEvent::OrderInvalid(order.clone());
                }
                self.exchange.insert_order(order.clone());
                info!(
                    "BROKER: Successfully sent {:?} order for {:?} shares of {:?} to exchange",
                    order.get_order_type(),
                    order.get_shares(),
                    order.get_symbol()
                );
                BrokerEvent::OrderSentToExchange(order)
            }
            SimulatedBrokerReadyState::Invalid => {
                panic!("Tried to send order before calling check");
            }
            SimulatedBrokerReadyState::InsufficientCash => BrokerEvent::OrderInvalid(order),
        }
    }

    fn send_orders(&mut self, orders: &[Order]) -> Vec<BrokerEvent> {
        let mut res = Vec::new();
        for o in orders {
            let trade = self.send_order(o.clone());
            res.push(trade);
        }
        res
    }

    fn clear_pending_market_orders_by_symbol(&mut self, symbol: &str) {
        self.exchange.clear_pending_market_orders_by_symbol(symbol);
    }
}

impl<T> TransferCash for SimulatedBroker<T> where 
 T: DataSource<Quote, Dividend> {

}

impl<T> GetsQuote for SimulatedBroker<T> where
 T: DataSource<Quote, Dividend> {
    fn get_quote(&self, symbol: &str) -> Option<&Quote> {
        self.exchange.get_quote(symbol)
    }

    fn get_quotes(&self) -> Option<&Vec<Quote>> {
        self.exchange.get_quotes()
    }
}

impl<T> EventLog for SimulatedBroker<T> where
 T: DataSource<Quote, Dividend> {
    fn trades_between(&self, start: &i64, end: &i64) -> Vec<Trade> {
        self.log.trades_between(start, end)
    }

    fn dividends_between(&self, start: &i64, end: &i64) -> Vec<DividendPayment> {
        self.log.dividends_between(start, end)
    }
}

///Represents whether the broker is in a state from which we can begin to modify state and issue
///new orders.
///
///[Invalid] means the broker can move into [Ready] but has some state that is old (likely needing
///[self.check()] to be called). If the client attempts to call a method dependent on state whilst the
///broker is in [Invalid] then the broker will trigger [panic!()] because there is likely an error
///in the client that will result in incorrect output.
///
///[Ready] state within broker has been fully reconciled and is ready to be modified.
///
///[InsufficientCash] means the broker has insufficient cash to continue working and no further
///state changes can be achieved. The client can deposit more cash to recover this but this
///behaviour isn't expected as most backtests will run with a fixed amount of cash from the start.
#[derive(Clone, Debug)]
enum SimulatedBrokerReadyState {
    Invalid,
    Ready,
    InsufficientCash,
}

#[cfg(test)]
mod tests {

    use super::{SimulatedBroker, SimulatedBrokerBuilder};
    use crate::broker::{
        BacktestBroker, BrokerCashEvent, BrokerCost, BrokerEvent, Dividend, Quote, TransferCash,
    };
    use crate::broker::{Order, OrderType};
    use crate::clock::{Clock, ClockBuilder};
    use crate::exchange::DefaultExchangeBuilder;
    use crate::input::{HashMapInput, HashMapInputBuilder};
    use crate::types::{DateTime, Frequency};

    use std::collections::HashMap;
    use std::rc::Rc;

    fn setup() -> (SimulatedBroker<HashMapInput>, Clock) {
        let mut prices: HashMap<DateTime, Vec<Quote>> = HashMap::new();
        let mut dividends: HashMap<DateTime, Vec<Dividend>> = HashMap::new();
        let quote = Quote::new(100.00, 101.00, 100, "ABC");
        let quote1 = Quote::new(10.00, 11.00, 100, "BCD");
        let quote2 = Quote::new(104.00, 105.00, 101, "ABC");
        let quote3 = Quote::new(14.00, 15.00, 101, "BCD");
        let quote4 = Quote::new(95.00, 96.00, 102, "ABC");
        let quote5 = Quote::new(10.00, 11.00, 102, "BCD");
        let quote6 = Quote::new(95.00, 96.00, 103, "ABC");
        let quote7 = Quote::new(10.00, 11.00, 103, "BCD");

        prices.insert(100.into(), vec![quote, quote1]);
        prices.insert(101.into(), vec![quote2, quote3]);
        prices.insert(102.into(), vec![quote4, quote5]);
        prices.insert(103.into(), vec![quote6, quote7]);

        let divi1 = Dividend::new(5.0, "ABC", 102);
        dividends.insert(102.into(), vec![divi1]);

        let clock = ClockBuilder::with_length_in_seconds(100, 5)
            .with_frequency(&Frequency::Second)
            .build();

        let source = HashMapInputBuilder::new()
            .with_quotes(prices)
            .with_dividends(dividends)
            .with_clock(Rc::clone(&clock))
            .build();

        let exchange = DefaultExchangeBuilder::new()
            .with_clock(Rc::clone(&clock))
            .with_data_source(source.clone())
            .build();

        let brkr = SimulatedBrokerBuilder::new()
            .with_data(source)
            .with_trade_costs(vec![BrokerCost::flat(1.0)])
            .with_exchange(exchange)
            .build();
        (brkr, clock)
    }

    #[test]
    fn test_cash_deposit_withdraw() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(&100.0);
        clock.borrow_mut().tick();

        //Test cash
        assert!(matches!(
            brkr.withdraw_cash(&50.0),
            BrokerCashEvent::WithdrawSuccess(..)
        ));
        assert!(matches!(
            brkr.withdraw_cash(&51.0),
            BrokerCashEvent::WithdrawFailure(..)
        ));
        assert!(matches!(
            brkr.deposit_cash(&50.0),
            BrokerCashEvent::DepositSuccess(..)
        ));

        //Test transactions
        assert!(matches!(
            brkr.debit(&50.0),
            BrokerCashEvent::WithdrawSuccess(..)
        ));
        assert!(matches!(
            brkr.debit(&51.0),
            BrokerCashEvent::WithdrawFailure(..)
        ));
        assert!(matches!(
            brkr.credit(&50.0),
            BrokerCashEvent::DepositSuccess(..)
        ));
    }

    #[test]
    fn test_that_buy_order_reduces_cash_and_increases_holdings() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(&100_000.0);
        let res = brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 495.0));
        println!("{:?}", res);
        assert!(matches!(res, BrokerEvent::OrderSentToExchange(..)));
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();

        let cash = brkr.get_cash_balance();
        assert!(*cash < 100_000.0);

        let qty = brkr.get_position_qty("ABC").unwrap();
        assert_eq!(*qty.clone(), 495.00);
    }

    #[test]
    fn test_that_buy_order_larger_than_cash_fails_with_error_returned_without_panic() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(&100.0);
        //Order value is greater than cash balance
        let res = brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 495.0));
        assert!(matches!(res, BrokerEvent::OrderInvalid(..)));
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();

        let cash = brkr.get_cash_balance();
        assert!(*cash == 100.0);
    }

    #[test]
    fn test_that_sell_order_larger_than_holding_fails_with_error_returned_without_panic() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(&100_000.0);
        let res = brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 100.0));
        assert!(matches!(res, BrokerEvent::OrderSentToExchange(..)));
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();

        //Order greater than current holding
        clock.borrow_mut().tick();
        brkr.check();
        let res = brkr.send_order(Order::market(OrderType::MarketSell, "ABC", 105.0));
        assert!(matches!(res, BrokerEvent::OrderInvalid(..)));
        brkr.finish();

        //Checking that
        let qty = brkr.get_position_qty("ABC").unwrap();
        println!("{:?}", qty);
        assert!((*qty.clone()).eq(&100.0));
    }

    #[test]
    fn test_that_market_sell_increases_cash_and_decreases_holdings() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(&100_000.0);
        let res = brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 495.0));
        assert!(matches!(res, BrokerEvent::OrderSentToExchange(..)));
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();
        let cash = brkr.get_cash_balance();

        clock.borrow_mut().tick();
        brkr.check();
        let res = brkr.send_order(Order::market(OrderType::MarketSell, "ABC", 295.0));
        assert!(matches!(res, BrokerEvent::OrderSentToExchange(..)));
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();
        let cash0 = brkr.get_cash_balance();

        let qty = brkr.get_position_qty("ABC").unwrap();
        assert_eq!(**qty, 200.0);
        assert!(*cash0 > *cash);
    }

    #[test]
    fn test_that_valuation_updates_in_next_period() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(&100_000.0);
        brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 495.0));
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();
        let val = brkr.get_position_value("ABC").unwrap();

        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();
        let val1 = brkr.get_position_value("ABC").unwrap();
        assert_ne!(val, val1);
    }

    #[test]
    fn test_that_profit_calculation_is_accurate() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(&100_000.0);
        brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 495.0));
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();

        let profit = brkr.get_position_profit("ABC").unwrap();
        assert_eq!(*profit, -4950.00);
    }

    #[test]
    fn test_that_dividends_are_paid() {
        let (mut brkr, clock) = setup();
        brkr.deposit_cash(&101_000.0);
        brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 495.0));
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();
        let cash_before_dividend = brkr.get_cash_balance();

        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();
        let cash_after_dividend = brkr.get_cash_balance();
        assert_ne!(cash_before_dividend, cash_after_dividend);
    }

    #[test]
    #[should_panic]
    fn test_that_broker_builder_fails_without_exchange() {
        let mut prices: HashMap<DateTime, Vec<Quote>> = HashMap::new();
        let quote = Quote::new(100.00, 101.00, 100, "ABC");
        prices.insert(100.into(), vec![quote]);

        let clock = ClockBuilder::with_length_in_seconds(100, 2)
            .with_frequency(&Frequency::Second)
            .build();

        let source = HashMapInputBuilder::new()
            .with_quotes(prices)
            .with_clock(Rc::clone(&clock))
            .build();

        let _brkr = SimulatedBrokerBuilder::new().with_data(source).build();
    }

    #[test]
    #[should_panic]
    fn test_that_broker_builder_fails_without_data() {
        let _brkr = SimulatedBrokerBuilder::<HashMapInput>::new()
            .with_trade_costs(vec![BrokerCost::flat(1.0)])
            .build();
    }

    #[test]
    fn test_that_broker_build_passes_without_trade_costs() {
        let mut prices: HashMap<DateTime, Vec<Quote>> = HashMap::new();

        let quote = Quote::new(100.00, 101.00, 100, "ABC");
        let quote2 = Quote::new(104.00, 105.00, 101, "ABC");
        let quote4 = Quote::new(95.00, 96.00, 102, "ABC");
        prices.insert(100.into(), vec![quote]);
        prices.insert(101.into(), vec![quote2]);
        prices.insert(102.into(), vec![quote4]);

        let clock = ClockBuilder::with_length_in_dates(100, 102)
            .with_frequency(&Frequency::Second)
            .build();

        let source = HashMapInputBuilder::new()
            .with_quotes(prices)
            .with_clock(Rc::clone(&clock))
            .build();

        let exchange = DefaultExchangeBuilder::new()
            .with_clock(Rc::clone(&clock))
            .with_data_source(source.clone())
            .build();

        let _brkr = SimulatedBrokerBuilder::new()
            .with_data(source)
            .with_exchange(exchange)
            .build();
    }

    #[test]
    fn test_that_broker_uses_last_value_if_it_fails_to_find_quote() {
        //If the broker cannot find a quote in the current period for a stock, it automatically
        //uses a value of zero. This is a problem because the current time could a weekend or
        //bank holiday, and if the broker is attempting to value the portfolio on that day
        //they will ask for a quote, not find one, and then use a value of zero which is
        //incorrect.

        let mut prices: HashMap<DateTime, Vec<Quote>> = HashMap::new();
        let dividends: HashMap<DateTime, Vec<Dividend>> = HashMap::new();
        let quote = Quote::new(100.00, 101.00, 100, "ABC");
        let quote1 = Quote::new(10.00, 11.00, 100, "BCD");

        let quote2 = Quote::new(100.00, 101.00, 101, "ABC");
        let quote3 = Quote::new(10.00, 11.00, 101, "BCD");

        let quote4 = Quote::new(104.00, 105.00, 102, "ABC");

        let quote5 = Quote::new(104.00, 105.00, 103, "ABC");
        let quote6 = Quote::new(12.00, 13.00, 103, "BCD");

        prices.insert(100.into(), vec![quote, quote1]);
        //Trades execute here
        prices.insert(101.into(), vec![quote2, quote3]);
        //We are missing a quote for BCD on 101, but the broker should return the last seen value
        prices.insert(102.into(), vec![quote4]);
        //And when we check the next date, it updates correctly
        prices.insert(103.into(), vec![quote5, quote6]);

        let clock = ClockBuilder::with_length_in_seconds(100, 5)
            .with_frequency(&Frequency::Second)
            .build();

        let source = HashMapInputBuilder::new()
            .with_quotes(prices)
            .with_dividends(dividends)
            .with_clock(Rc::clone(&clock))
            .build();

        let exchange = DefaultExchangeBuilder::new()
            .with_clock(Rc::clone(&clock))
            .with_data_source(source.clone())
            .build();

        let mut brkr = SimulatedBrokerBuilder::new()
            .with_data(source)
            .with_exchange(exchange)
            .with_trade_costs(vec![BrokerCost::flat(1.0)])
            .build();

        brkr.deposit_cash(&100_000.0);
        brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 100.0));
        brkr.send_order(Order::market(OrderType::MarketBuy, "BCD", 100.0));
        brkr.finish();

        //Trades execute
        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();

        //Missing live quote for BCD
        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();
        let value = brkr.get_position_value("BCD").unwrap();
        println!("{:?}", value);
        //We test against the bid price, which gives us the value exclusive of the price paid at ask
        assert!(*value == 10.0 * 100.0);

        //BCD has quote again
        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();

        let value1 = brkr.get_position_value("BCD").unwrap();
        println!("{:?}", value1);
        assert!(*value1 == 12.0 * 100.0);
    }

    #[test]
    fn test_that_broker_handles_negative_cash_balance_due_to_volatility() {
        //Because orders sent to the exchange are not executed instantaneously it is possible for a
        //broker to issue an order for a stock, the price to fall/rise before the trade gets
        //executed, and the broker end up with more/less cash than expected.
        //
        //For example, if orders are issued for 100% of the portfolio then if prices rises then we
        //can end up with negative balances.

        let mut prices: HashMap<DateTime, Vec<Quote>> = HashMap::new();
        let quote = Quote::new(100.00, 101.00, 100, "ABC");
        let quote1 = Quote::new(150.00, 151.00, 101, "ABC");
        let quote2 = Quote::new(150.00, 151.00, 102, "ABC");

        prices.insert(100.into(), vec![quote]);
        prices.insert(101.into(), vec![quote1]);
        prices.insert(102.into(), vec![quote2]);

        let clock = ClockBuilder::with_length_in_seconds(100, 5)
            .with_frequency(&Frequency::Second)
            .build();

        let source = HashMapInputBuilder::new()
            .with_quotes(prices)
            .with_clock(Rc::clone(&clock))
            .build();

        let exchange = DefaultExchangeBuilder::new()
            .with_clock(Rc::clone(&clock))
            .with_data_source(source.clone())
            .build();

        let mut brkr = SimulatedBrokerBuilder::new()
            .with_data(source)
            .with_exchange(exchange)
            .with_trade_costs(vec![BrokerCost::flat(1.0)])
            .build();

        brkr.deposit_cash(&100_000.0);
        //Because the price of ABC rises after this order is sent, we will end up with a negative
        //cash balance after the order is executed
        brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 700.0));
        brkr.finish();

        //Trades execute
        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();

        let cash = brkr.get_cash_balance();
        assert!(*cash < 0.0);

        //Broker rebalances to raise cash
        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();
        let cash1 = brkr.get_cash_balance();
        assert!(*cash1 > 0.0);
    }
}
