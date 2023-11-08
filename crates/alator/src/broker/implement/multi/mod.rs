//! Multi-threaded broker
mod builder;

pub use builder::ConcurrentBrokerBuilder;

use async_trait::async_trait;
use log::info;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

#[allow(unused)]
use crate::broker::implement::single::SingleBroker;
use crate::exchange::{DefaultSubscriberId, NotifyReceiver, OrderSender, PriceReceiver};
use crate::input::{CorporateEventsSource, Dividendable, Quotable};
use crate::types::{CashValue, PortfolioHoldings, PortfolioQty, Price};

use crate::broker::{
    BacktestBroker, BrokerCalculations, BrokerCashEvent, BrokerCost, BrokerEvent, BrokerLog,
    DividendPayment, EventLog, GetsQuote, Order, OrderType, ReceivesOrdersAsync, Trade,
};

/// Once the broker moves into Failed state then all operations that mutate state are rejected.
///
/// This flag is intended to cover any situation where the broker moves into a state where it is
/// unclear how to move the state foward. In most cases, and contrary to the intuition with this
/// kind of error, this will be due to errors with the strategy code and the interaction with
/// external state (i.e. price source).
///
/// Once this happens, the broker will stop performing cash transactions and issuing orders. The
/// broker won't throw an error once this happens and will continue reading from exchange to
/// reconcile trades/liquidate current position in order to return a correct cash balance to the
/// strategy. If the price source is missing data after a liquidation is triggered then it is
/// possible for incorrect results to be returned.
///
/// The most common scenario for this state to be triggered is due to bad strategy code triggering
/// the liquidation process and the broker being unable to find sufficient cash (plus a buffer of
/// 1000, currently hardcoded).
///
/// A less common scenario contrived to demonstrate how this can occur due to external data: we
/// have a portfolio with cash of 100, the strategy issues a market order for 100 shares @ 1,
/// the market price doubles on the next tick, and so the exchange asks for 200 in cash to settle
/// the trade. Once this happens, it is unclear what the broker should do so we move into an error
/// condition and stop mutating more state.
///
/// Broker should be in Ready state on creation.
#[derive(Debug)]
enum BrokerState {
    Ready,
    Failed,
}

/// Multi-threaded broker. Created with [ConcurrentBrokerBuilder].
#[derive(Debug)]
pub struct ConcurrentBroker<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    corporate_source: Option<T>,
    holdings: PortfolioHoldings,
    cash: CashValue,
    log: BrokerLog,
    trade_costs: Vec<BrokerCost>,
    price_receiver: PriceReceiver<Q>,
    notify_receiver: NotifyReceiver,
    order_sender: OrderSender,
    exchange_subscriber_id: DefaultSubscriberId,
    latest_quotes: HashMap<String, Arc<Q>>,
    dividend: PhantomData<D>,
    broker_state: BrokerState,
}

impl<D, T, Q> ConcurrentBroker<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    pub fn cost_basis(&self, symbol: &str) -> Option<Price> {
        self.log.cost_basis(symbol)
    }

    //Contains tasks that should be run on every iteration of the simulation irregardless of the
    //state on the client.
    pub async fn check(&mut self) {
        self.pay_dividends();
        //Reconcile broker against executed trades
        while let Ok(notification) = self.notify_receiver.try_recv() {
            //TODO: if cash is below zero, we end the simulation. In practice, this shouldn't cause
            //problems because the broker will be unable to fund any future trades but exiting
            //early will give less confusing output.
            match notification {
                crate::exchange::ExchangeNotificationMessage::OrderBooked(_id, _order) => {
                    //TODO: when the exchange books an order we should store the change
                }
                crate::exchange::ExchangeNotificationMessage::TradeCompleted(trade) => {
                    match trade.typ {
                        //Force debit so we can end up with negative cash here
                        crate::exchange::TradeType::Buy => self.debit_force(&trade.value),
                        crate::exchange::TradeType::Sell => self.credit(&trade.value),
                    };
                    self.log.record::<Trade>(trade.clone().into());

                    let curr_position = self.get_position_qty(&trade.symbol).unwrap_or_default();

                    let updated = match trade.typ {
                        crate::exchange::TradeType::Buy => *curr_position + trade.quantity,
                        crate::exchange::TradeType::Sell => *curr_position - trade.quantity,
                    };

                    self.update_holdings(&trade.symbol, PortfolioQty::from(updated));
                }
                crate::exchange::ExchangeNotificationMessage::OrderDeleted(_order_id) => (),
            }
        }
        //Previous step can cause negative cash balance so we have to rebalance here, this
        //is not instant so will never balance properly if the series is very volatile
        self.rebalance_cash().await;

        //Update prices, these prices are not tradable
        while let Ok(quotes) = self.price_receiver.try_recv() {
            for quote in &quotes {
                self.latest_quotes
                    .insert(quote.get_symbol().to_string(), Arc::clone(quote));
            }
        }
    }

    async fn rebalance_cash(&mut self) {
        //Has to be less than, we can have zero value without needing to liquidate if we initialize
        //the portfolio but exchange doesn't execute any trades. This can happen if we are missing
        //prices at the start of the series
        if *self.cash < 0.0 {
            let shortfall = *self.cash * -1.0;
            //When we raise cash, we try to raise a small amount more to stop continuous
            //rebalancing, this amount is arbitrary atm
            let plus_buffer = shortfall + 1000.0;

            let res =
                BrokerCalculations::withdraw_cash_with_liquidation_async(&plus_buffer, self).await;

            if let BrokerCashEvent::WithdrawFailure(_val) = res {
                //The broker tried to generate cash required but was unable to do so. Stop all
                //further mutations, and run out the current portfolio state to return some
                //value to strategy
                self.broker_state = BrokerState::Failed;
            }
        }
    }
}

impl<D, T, Q> GetsQuote<Q> for ConcurrentBroker<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    fn get_quote(&self, symbol: &str) -> Option<Arc<Q>> {
        self.latest_quotes.get(symbol).cloned()
    }

    fn get_quotes(&self) -> Option<Vec<Arc<Q>>> {
        if self.latest_quotes.is_empty() {
            return None;
        }

        let mut tmp = Vec::new();
        for quote in self.latest_quotes.values() {
            tmp.push(Arc::clone(quote));
        }
        Some(tmp)
    }
}

#[async_trait]
impl<D, T, Q> ReceivesOrdersAsync for ConcurrentBroker<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    async fn send_order(&mut self, order: Order) -> BrokerEvent {
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
            OrderType::MarketBuy | OrderType::LimitBuy | OrderType::StopBuy => quote.get_ask(),
            OrderType::MarketSell | OrderType::LimitSell | OrderType::StopSell => quote.get_bid(),
        };

        if let Err(_err) = BrokerCalculations::client_has_sufficient_cash(&order, price, self) {
            info!(
                "BROKER: Unable to send {:?} order for {:?} shares of {:?} to exchange",
                order.get_order_type(),
                order.get_shares(),
                order.get_symbol()
            );
            return BrokerEvent::OrderInvalid(order.clone());
        }
        if let Err(_err) = BrokerCalculations::client_has_sufficient_holdings_for_sale(&order, self)
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

        let _ = self
            .order_sender
            .send(order.into_exchange_message(self.exchange_subscriber_id))
            .await
            .unwrap();

        info!(
            "BROKER: Successfully sent {:?} order for {:?} shares of {:?} to exchange",
            order.get_order_type(),
            order.get_shares(),
            order.get_symbol()
        );
        BrokerEvent::OrderSentToExchange(order)
    }

    async fn send_orders(&mut self, orders: &[Order]) -> Vec<BrokerEvent> {
        let mut res = Vec::new();
        for o in orders {
            let trade = self.send_order(o.clone()).await;
            res.push(trade);
        }
        res
    }
}

#[async_trait]
impl<D, T, Q> BacktestBroker<Q> for ConcurrentBroker<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    fn update_holdings(&mut self, symbol: &str, change: PortfolioQty) {
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

    fn get_position_cost(&self, symbol: &str) -> Option<Price> {
        self.log.cost_basis(symbol)
    }

    //Identical to deposit_cash but is seperated to distinguish internal cash
    //transactions from external with no value returned to client
    fn credit(&mut self, value: &f64) -> BrokerCashEvent {
        info!(
            "BROKER: Credited {:?} cash, current balance of {:?}",
            value, self.cash
        );
        self.cash = CashValue::from(*value + *self.cash);
        BrokerCashEvent::DepositSuccess(CashValue::from(*value))
    }

    //Looks similar to withdraw_cash but distinguished because it represents
    //failure of an internal transaction with no value returned to clients
    fn debit(&mut self, value: &f64) -> BrokerCashEvent {
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

    fn debit_force(&mut self, value: &f64) -> BrokerCashEvent {
        info!(
            "BROKER: Force debt {:?} cash, current balance of {:?}",
            value, self.cash
        );
        self.cash = CashValue::from(*self.cash - *value);
        BrokerCashEvent::WithdrawSuccess(CashValue::from(*value))
    }

    fn get_cash_balance(&self) -> CashValue {
        self.cash.clone()
    }

    fn get_holdings(&self) -> PortfolioHoldings {
        self.holdings.clone()
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
        if self.corporate_source.is_none() {
            //Cannot be dividends so return immediately
            return;
        }

        info!("BROKER: Checking dividends");
        let mut dividend_value: CashValue = CashValue::from(0.0);
        if let Some(dividends) = self.corporate_source.as_ref().unwrap().get_dividends() {
            for dividend in dividends.iter() {
                //Our dataset can include dividends for stocks we don't own so we need to check
                //that we own the stock, not performant but can be changed later
                if let Some(qty) = self.get_position_qty(dividend.get_symbol()) {
                    info!(
                        "BROKER: Found dividend of {:?} for portfolio holding {:?}",
                        dividend.get_value(),
                        dividend.get_symbol()
                    );
                    let cash_value = CashValue::from(*qty.clone() * **dividend.get_value());
                    dividend_value = cash_value.clone() + dividend_value;
                    let dividend_paid = DividendPayment::new(
                        cash_value,
                        dividend.get_symbol().clone(),
                        *dividend.get_date(),
                    );
                    self.log.record(dividend_paid);
                }
            }
        }
        self.credit(&dividend_value);
    }

    fn withdraw_cash(&mut self, cash: &f64) -> BrokerCashEvent {
        match self.broker_state {
            BrokerState::Failed => {
                info!(
                    "BROKER: Attempted cash withdraw of {:?} but broker in Failed State",
                    cash,
                );
                BrokerCashEvent::OperationFailure(CashValue::from(*cash))
            }
            BrokerState::Ready => {
                if cash > &self.get_cash_balance() {
                    info!(
                        "BROKER: Attempted cash withdraw of {:?} but only have {:?}",
                        cash,
                        self.get_cash_balance()
                    );
                    return BrokerCashEvent::WithdrawFailure(CashValue::from(*cash));
                }
                info!(
                    "BROKER: Successful cash withdraw of {:?}, {:?} left in cash",
                    cash,
                    self.get_cash_balance()
                );
                self.debit(cash);
                BrokerCashEvent::WithdrawSuccess(CashValue::from(*cash))
            }
        }
    }

    fn deposit_cash(&mut self, cash: &f64) -> BrokerCashEvent {
        match self.broker_state {
            BrokerState::Failed => {
                info!(
                    "BROKER: Attempted cash deposit of {:?} but broker in Failed State",
                    cash,
                );
                BrokerCashEvent::OperationFailure(CashValue::from(*cash))
            }
            BrokerState::Ready => {
                info!(
                    "BROKER: Deposited {:?} cash, current balance of {:?}",
                    cash,
                    self.get_cash_balance()
                );
                self.credit(cash);
                BrokerCashEvent::DepositSuccess(CashValue::from(*cash))
            }
        }
    }
}

impl<D, T, Q> EventLog for ConcurrentBroker<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    fn trades_between(&self, start: &i64, end: &i64) -> Vec<Trade> {
        self.log.trades_between(start, end)
    }

    fn dividends_between(&self, start: &i64, end: &i64) -> Vec<DividendPayment> {
        self.log.dividends_between(start, end)
    }
}

unsafe impl<D, T, Q> Send for ConcurrentBroker<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
}

unsafe impl<D, T, Q> Sync for ConcurrentBroker<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
}

#[cfg(test)]
mod tests {

    use crate::broker::implement::multi::{ConcurrentBroker, ConcurrentBrokerBuilder};
    use crate::broker::{
        BacktestBroker, BrokerCashEvent, BrokerCost, BrokerEvent, Dividend, Order, OrderType,
        Quote, ReceivesOrdersAsync,
    };
    use crate::clock::ClockBuilder;
    use crate::exchange::implement::multi::{ConcurrentExchange, ConcurrentExchangeBuilder};
    use crate::input::{DefaultCorporateEventsSource, DefaultPriceSource};
    use crate::types::{CashValue, Frequency};

    async fn setup() -> (
        ConcurrentBroker<Dividend, DefaultCorporateEventsSource, Quote>,
        ConcurrentExchange<Quote, DefaultPriceSource>,
    ) {
        let clock = crate::clock::ClockBuilder::with_length_in_seconds(100, 5)
            .with_frequency(&crate::types::Frequency::Second)
            .build();
        let mut price_source = DefaultPriceSource::new(clock.clone());
        price_source.add_quotes(100.00, 101.00, 100, "ABC");
        price_source.add_quotes(10.00, 11.00, 100, "BCD");
        price_source.add_quotes(104.00, 105.00, 101, "ABC");
        price_source.add_quotes(14.00, 15.00, 101, "BCD");
        price_source.add_quotes(95.00, 96.00, 102, "ABC");
        price_source.add_quotes(10.00, 11.00, 102, "BCD");
        price_source.add_quotes(95.00, 96.00, 103, "ABC");
        price_source.add_quotes(10.00, 11.00, 103, "BCD");

        let mut corporate_source = DefaultCorporateEventsSource::new(clock.clone());
        corporate_source.add_dividends(5.0, "ABC", 102);

        let mut exchange = ConcurrentExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_price_source(price_source)
            .build();

        let brkr = ConcurrentBrokerBuilder::new()
            .with_corporate_source(corporate_source)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build(&mut exchange)
            .await;

        (brkr, exchange)
    }

    #[tokio::test]
    async fn test_cash_deposit_withdraw() {
        let (mut brkr, mut exchange) = setup().await;
        brkr.deposit_cash(&100.0);

        exchange.check().await;
        brkr.check().await;

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

    #[tokio::test]
    async fn test_that_buy_order_reduces_cash_and_increases_holdings() {
        let (mut brkr, mut exchange) = setup().await;
        brkr.deposit_cash(&100_000.0);
        let res = brkr
            .send_order(Order::market(OrderType::MarketBuy, "ABC", 495.0))
            .await;
        println!("{:?}", res);
        assert!(matches!(res, BrokerEvent::OrderSentToExchange(..)));

        exchange.check().await;
        brkr.check().await;

        let cash = brkr.get_cash_balance();
        assert!(*cash < 100_000.0);

        let qty = brkr.get_position_qty("ABC").unwrap_or_default();
        assert_eq!(*qty, 495.00);
    }

    #[tokio::test]
    async fn test_that_buy_order_larger_than_cash_fails_with_error_returned_without_panic() {
        let (mut brkr, mut exchange) = setup().await;
        brkr.deposit_cash(&100.0);
        //Order value is greater than cash balance
        let res = brkr
            .send_order(Order::market(OrderType::MarketBuy, "ABC", 495.0))
            .await;
        assert!(matches!(res, BrokerEvent::OrderInvalid(..)));
        exchange.check().await;
        brkr.check().await;

        let cash = brkr.get_cash_balance();
        assert!(*cash == 100.0);
    }

    #[tokio::test]
    async fn test_that_sell_order_larger_than_holding_fails_with_error_returned_without_panic() {
        let (mut brkr, mut exchange) = setup().await;
        brkr.deposit_cash(&100_000.0);
        let res = brkr
            .send_order(Order::market(OrderType::MarketBuy, "ABC", 100.0))
            .await;
        assert!(matches!(res, BrokerEvent::OrderSentToExchange(..)));
        exchange.check().await;
        brkr.check().await;

        //Order greater than current holding
        exchange.check().await;
        brkr.check().await;
        let res = brkr
            .send_order(Order::market(OrderType::MarketSell, "ABC", 105.0))
            .await;
        assert!(matches!(res, BrokerEvent::OrderInvalid(..)));

        //Checking that
        let qty = brkr.get_position_qty("ABC").unwrap_or_default();
        println!("{:?}", qty);
        assert_eq!(*qty, 100.0);
    }

    #[tokio::test]
    async fn test_that_market_sell_increases_cash_and_decreases_holdings() {
        let (mut brkr, mut exchange) = setup().await;
        brkr.deposit_cash(&100_000.0);
        let res = brkr
            .send_order(Order::market(OrderType::MarketBuy, "ABC", 495.0))
            .await;
        assert!(matches!(res, BrokerEvent::OrderSentToExchange(..)));
        exchange.check().await;
        brkr.check().await;
        let cash = brkr.get_cash_balance();

        exchange.check().await;
        brkr.check().await;
        let res = brkr
            .send_order(Order::market(OrderType::MarketSell, "ABC", 295.0))
            .await;
        assert!(matches!(res, BrokerEvent::OrderSentToExchange(..)));

        exchange.check().await;
        brkr.check().await;
        let cash0 = brkr.get_cash_balance();

        let qty = brkr.get_position_qty("ABC").unwrap_or_default();
        assert_eq!(*qty, 200.0);
        assert!(*cash0 > *cash);
    }

    #[tokio::test]
    async fn test_that_valuation_updates_in_next_period() {
        let (mut brkr, mut exchange) = setup().await;
        brkr.deposit_cash(&100_000.0);
        brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 495.0))
            .await;
        exchange.check().await;
        brkr.check().await;

        let val = brkr.get_position_value("ABC");

        exchange.check().await;
        brkr.check().await;
        let val1 = brkr.get_position_value("ABC");
        assert_ne!(val, val1);
    }

    #[tokio::test]
    async fn test_that_profit_calculation_is_accurate() {
        let (mut brkr, mut exchange) = setup().await;
        brkr.deposit_cash(&100_000.0);
        brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 495.0))
            .await;
        exchange.check().await;
        brkr.check().await;

        exchange.check().await;
        brkr.check().await;

        let profit = brkr.get_position_profit("ABC").unwrap();
        assert_eq!(*profit, -4950.00);
    }

    #[tokio::test]
    async fn test_that_dividends_are_paid() {
        let (mut brkr, mut exchange) = setup().await;
        brkr.deposit_cash(&101_000.0);
        brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 495.0))
            .await;

        exchange.check().await;
        brkr.check().await;
        let cash_before_dividend = brkr.get_cash_balance();

        exchange.check().await;
        brkr.check().await;
        let cash_after_dividend = brkr.get_cash_balance();
        assert_ne!(cash_before_dividend, cash_after_dividend);
    }

    #[tokio::test]
    async fn test_that_broker_build_passes_without_trade_costs() {
        let clock = ClockBuilder::with_length_in_dates(100, 102)
            .with_frequency(&Frequency::Second)
            .build();

        let mut price_source = DefaultPriceSource::new(clock.clone());
        price_source.add_quotes(100.00, 101.00, 100, "ABC");
        price_source.add_quotes(104.00, 105.00, 101, "ABC");
        price_source.add_quotes(95.00, 96.00, 102, "ABC");

        let mut exchange = ConcurrentExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_price_source(price_source)
            .build();

        let _brkr: ConcurrentBroker<Dividend, DefaultCorporateEventsSource, Quote> =
            ConcurrentBrokerBuilder::new()
                .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
                .build(&mut exchange)
                .await;
    }

    #[tokio::test]
    async fn test_that_broker_uses_last_value_if_it_fails_to_find_quote() {
        //If the broker cannot find a quote in the current period for a stock, it automatically
        //uses a value of zero. This is a problem because the current time could a weekend or
        //bank holiday, and if the broker is attempting to value the portfolio on that day
        //they will ask for a quote, not find one, and then use a value of zero which is
        //incorrect.

        let clock = ClockBuilder::with_length_in_seconds(100, 5)
            .with_frequency(&Frequency::Second)
            .build();

        let mut price_source = DefaultPriceSource::new(clock.clone());

        price_source.add_quotes(100.00, 101.00, 100, "ABC");
        price_source.add_quotes(10.00, 11.00, 100, "BCD");

        //Trades execute here
        price_source.add_quotes(100.00, 101.00, 101, "ABC");
        price_source.add_quotes(10.00, 11.00, 101, "BCD");

        //We are missing a quote for BCD on 101, but the broker should return the last seen value
        price_source.add_quotes(104.00, 105.00, 102, "ABC");

        //And when we check the next date, it updates correctly
        price_source.add_quotes(104.00, 105.00, 103, "ABC");
        price_source.add_quotes(12.00, 13.00, 103, "BCD");

        let mut exchange = ConcurrentExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_price_source(price_source)
            .build();

        let mut brkr: ConcurrentBroker<Dividend, DefaultCorporateEventsSource, Quote> =
            ConcurrentBrokerBuilder::new()
                .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
                .build(&mut exchange)
                .await;

        brkr.deposit_cash(&100_000.0);
        brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 100.0))
            .await;
        brkr.send_order(Order::market(OrderType::MarketBuy, "BCD", 100.0))
            .await;

        exchange.check().await;
        brkr.check().await;

        //Missing live quote for BCD
        exchange.check().await;
        brkr.check().await;
        let value = brkr
            .get_position_value("BCD")
            .unwrap_or(CashValue::from(0.0));
        println!("{:?}", value);
        //We test against the bid price, which gives us the value exclusive of the price paid at ask
        assert!(*value == 10.0 * 100.0);

        //BCD has quote again
        exchange.check().await;
        brkr.check().await;

        let value1 = brkr
            .get_position_value("BCD")
            .unwrap_or(CashValue::from(0.0));
        println!("{:?}", value1);
        assert!(*value1 == 12.0 * 100.0);
    }

    #[tokio::test]
    async fn test_that_broker_handles_negative_cash_balance_due_to_volatility() {
        //Because orders sent to the exchange are not executed instantaneously it is possible for a
        //broker to issue an order for a stock, the price to fall/rise before the trade gets
        //executed, and the broker end up with more/less cash than expected.
        //
        //For example, if orders are issued for 100% of the portfolio then if prices rises then we
        //can end up with negative balances.

        let clock = ClockBuilder::with_length_in_seconds(100, 5)
            .with_frequency(&Frequency::Second)
            .build();

        let mut price_source = DefaultPriceSource::new(clock.clone());
        price_source.add_quotes(100.00, 101.00, 100, "ABC");
        price_source.add_quotes(150.00, 151.00, 101, "ABC");
        price_source.add_quotes(150.00, 151.00, 102, "ABC");

        let mut exchange = ConcurrentExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_price_source(price_source)
            .build();

        let mut brkr: ConcurrentBroker<Dividend, DefaultCorporateEventsSource, Quote> =
            ConcurrentBrokerBuilder::new()
                .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
                .build(&mut exchange)
                .await;

        brkr.deposit_cash(&100_000.0);
        //Because the price of ABC rises after this order is sent, we will end up with a negative
        //cash balance after the order is executed
        brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 700.0))
            .await;

        //Trades execute
        exchange.check().await;
        brkr.check().await;

        let cash = brkr.get_cash_balance();
        assert!(*cash < 0.0);

        //Broker rebalances to raise cash
        exchange.check().await;
        brkr.check().await;
        let cash1 = brkr.get_cash_balance();
        assert!(*cash1 > 0.0);
    }

    #[tokio::test]
    async fn test_that_broker_stops_when_liquidation_fails() {
        let clock = crate::clock::ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&crate::types::Frequency::Second)
            .build();

        let mut price_source = DefaultPriceSource::new(clock.clone());

        price_source.add_quotes(100.00, 101.00, 100, "ABC");
        //Price doubles over one tick so that the broker is trading on information that has become
        //very inaccurate
        price_source.add_quotes(200.00, 201.00, 101, "ABC");
        price_source.add_quotes(200.00, 201.00, 101, "ABC");

        let mut exchange = ConcurrentExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_price_source(price_source)
            .build();

        let mut brkr: ConcurrentBroker<Dividend, DefaultCorporateEventsSource, Quote> =
            ConcurrentBrokerBuilder::new()
                .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
                .build(&mut exchange)
                .await;

        brkr.deposit_cash(&100_000.0);
        //This will use all the available cash balance, the market price doubles so the broker ends
        //up with a shortfall of -100_000.
        brkr.send_order(Order::market(OrderType::MarketBuy, "ABC", 990.0))
            .await;

        exchange.check().await;
        brkr.check().await;
        exchange.check().await;
        brkr.check().await;
        exchange.check().await;
        brkr.check().await;

        let cash = brkr.get_cash_balance();
        assert!(*cash < 0.0);

        let res = brkr
            .send_order(Order::market(OrderType::MarketBuy, "ABC", 100.0))
            .await;
        assert!(matches!(res, BrokerEvent::OrderInvalid { .. }));

        assert!(matches!(
            brkr.deposit_cash(&100_000.0),
            BrokerCashEvent::OperationFailure { .. }
        ));
        assert!(matches!(
            brkr.withdraw_cash(&100_000.0),
            BrokerCashEvent::OperationFailure { .. }
        ));
    }
}