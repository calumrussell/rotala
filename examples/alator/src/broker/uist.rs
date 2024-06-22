use itertools::Itertools;
use std::{
    collections::HashMap,
    error::Error,
    fmt::{Display, Formatter},
    mem,
};

use log::info;
use rotala::exchange::uist_v1::{Order, OrderType, Trade, TradeType, UistQuote, UistV1};
use rotala::http::uist::uistv1_client::Client;
use rotala::{
    clock::DateTime,
    http::uist::uistv1_client::{BacktestId, UistClient},
};

use crate::{strategy::staticweight::StaticWeightBroker, types::{
    CashValue, PortfolioAllocation, PortfolioHoldings, PortfolioQty, PortfolioValues, Price,
}};

use super::{
    BrokerCost, BrokerEvent, BrokerOperations, BrokerState, BrokerStates, CashOperations,
    Portfolio, Quote, SendOrder, Update,
};

type UistBrokerEvent = BrokerEvent<Order>;

/// Implementation of broker that uses the [Uist](rotala::exchange::uist::UistV1) exchange.
#[derive(Debug)]
pub struct UistBroker<C: UistClient> {
    cash: CashValue,
    holdings: PortfolioHoldings,
    //Kept distinct from holdings because some perf calculations may need to distinguish between
    //trades that we know are booked vs ones that we think should get booked
    pending_orders: PortfolioHoldings,
    //Used to mark last trade seen by broker when reconciling completed trades with exchange
    last_seen_trade: usize,
    latest_quotes: HashMap<String, UistQuote>,
    log: UistBrokerLog,
    trade_costs: Vec<BrokerCost>,
    broker_state: BrokerState,
    http_client: C,
    backtest_id: BacktestId,
}

impl<C: UistClient> StaticWeightBroker<UistQuote, Order> for UistBroker<C> {}

impl<C: UistClient> Quote<UistQuote> for UistBroker<C> {
    fn get_quote(&self, symbol: &str) -> Option<UistQuote> {
        self.latest_quotes.get(symbol).cloned()
    }

    fn get_quotes(&self) -> Option<Vec<UistQuote>> {
        if self.latest_quotes.is_empty() {
            return None;
        }

        let mut tmp = Vec::new();
        for quote in self.latest_quotes.values() {
            tmp.push(quote.clone());
        }
        Some(tmp)
    }
}

impl<C: UistClient> Portfolio<UistQuote> for UistBroker<C> {
    fn get_trade_costs(&self) -> Vec<BrokerCost> {
        self.trade_costs.clone()
    }

    fn get_holdings(&self) -> PortfolioHoldings {
        self.holdings.clone()
    }

    fn get_cash_balance(&self) -> CashValue {
        self.cash.clone()
    }

    fn update_cash_balance(&mut self, cash: CashValue) {
        self.cash = cash;
    }

    fn get_position_cost(&self, symbol: &str) -> Option<Price> {
        self.log.cost_basis(symbol)
    }

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

    fn get_pending_orders(&self) -> PortfolioHoldings {
        self.pending_orders.clone()
    }
}

impl<C: UistClient> BrokerStates for UistBroker<C> {
    fn get_broker_state(&self) -> BrokerState {
        self.broker_state.clone()
    }

    fn update_broker_state(&mut self, state: BrokerState) {
        self.broker_state = state;
    }
}

impl<C: UistClient> CashOperations<UistQuote> for UistBroker<C> {}

impl<C: UistClient> BrokerOperations<Order, UistQuote> for UistBroker<C> {}

impl<C: UistClient> SendOrder<Order> for UistBroker<C> {
    fn send_order(&mut self, order: Order) -> UistBrokerEvent {
        //This is an estimate of the cost based on the current price, can still end with negative
        //balance when we reconcile with actuals, may also reject valid orders at the margin
        match self.get_broker_state() {
            BrokerState::Failed => {
                info!(
                    "BROKER: Unable to send {:?} order for {:?} shares of {:?} to exchange as broker in Failed state",
                    order.get_order_type(),
                    order.get_shares(),
                    order.get_symbol()
                );
                UistBrokerEvent::OrderInvalid(order.clone())
            }
            BrokerState::Ready => {
                info!(
                    "BROKER: Attempting to send {:?} order for {:?} shares of {:?} to the exchange",
                    order.get_order_type(),
                    order.get_shares(),
                    order.get_symbol()
                );

                let quote = self.get_quote(order.get_symbol()).unwrap();
                let price = match order.get_order_type() {
                    OrderType::MarketBuy | OrderType::LimitBuy | OrderType::StopBuy => quote.ask,
                    OrderType::MarketSell | OrderType::LimitSell | OrderType::StopSell => quote.bid,
                };

                if let Err(_err) =
                    self.client_has_sufficient_cash::<OrderType>(&order, &Price::from(price))
                {
                    info!(
                        "BROKER: Unable to send {:?} order for {:?} shares of {:?} to exchange",
                        order.get_order_type(),
                        order.get_shares(),
                        order.get_symbol()
                    );
                    return UistBrokerEvent::OrderInvalid(order.clone());
                }
                if let Err(_err) = self.client_has_sufficient_holdings_for_sale::<OrderType>(&order)
                {
                    info!(
                        "BROKER: Unable to send {:?} order for {:?} shares of {:?} to exchange",
                        order.get_order_type(),
                        order.get_shares(),
                        order.get_symbol()
                    );
                    return UistBrokerEvent::OrderInvalid(order.clone());
                }
                if let Err(_err) = self.client_is_issuing_nonsense_order(&order) {
                    info!(
                        "BROKER: Unable to send {:?} order for {:?} shares of {:?} to exchange",
                        order.get_order_type(),
                        order.get_shares(),
                        order.get_symbol()
                    );
                    return UistBrokerEvent::OrderInvalid(order.clone());
                }

                self.http_client
                    .insert_order(order.clone(), self.backtest_id);
                //From the point of view of strategy, an order pending is the same as an order
                //executed. If the order is executed, then it is executed. If the order isn't
                //executed then the strategy must wait but all the strategy's work has been
                //done. So once we send the order, we need some way for clients to work out
                //what orders are pending and whether they need to do more work.
                let order_effect = match order.get_order_type() {
                    OrderType::MarketBuy | OrderType::LimitBuy | OrderType::StopBuy => {
                        order.get_shares()
                    }

                    OrderType::MarketSell | OrderType::LimitSell | OrderType::StopSell => {
                        -order.get_shares()
                    }
                };

                if let Some(position) = self.pending_orders.get(order.get_symbol()) {
                    let existing = *position + order_effect;
                    self.pending_orders
                        .insert(order.get_symbol(), &PortfolioQty::from(existing));
                } else {
                    self.pending_orders
                        .insert(order.get_symbol(), &PortfolioQty::from(order_effect));
                }
                info!(
                    "BROKER: Successfully sent {:?} order for {:?} shares of {:?} to exchange",
                    order.get_order_type(),
                    order.get_shares(),
                    order.get_symbol()
                );
                UistBrokerEvent::OrderSentToExchange(order)
            }
        }
    }

    fn send_orders(&mut self, orders: &[Order]) -> Vec<UistBrokerEvent> {
        let mut res = Vec::new();
        for o in orders {
            let trade = self.send_order(o.clone());
            res.push(trade);
        }
        res
    }
}

impl<C: UistClient> Update for UistBroker<C> {
    /// Called on every tick of clock to ensure that state is synchronized with other components.
    ///
    /// * Calls `check` on exchange
    /// * Updates last seen prices for exchange tick
    /// * Reconciles internal state against trades completed on current tick
    /// * Rebalances cash, which can trigger new trades if broker is in invalid state
    async fn check(&mut self) {
        if let Ok(tick_response) = self.http_client.tick(self.backtest_id).await {
            if let Ok(quotes_response) = self.http_client.fetch_quotes(self.backtest_id).await {
                //Update prices, these prices are not tradable
                for quote in &quotes_response.quotes {
                    self.latest_quotes
                        .insert(quote.symbol.clone(), quote.clone());
                }

                for trade in tick_response.executed_trades {
                    match trade.typ {
                        //Force debit so we can end up with negative cash here
                        TradeType::Buy => self.debit_force(&trade.value),
                        TradeType::Sell => self.credit(&trade.value),
                    };
                    self.log.record::<Trade>(trade.clone());

                    let curr_position = self.get_position_qty(&trade.symbol).unwrap_or_default();

                    let updated = match trade.typ {
                        TradeType::Buy => *curr_position + trade.quantity,
                        TradeType::Sell => *curr_position - trade.quantity,
                    };
                    self.update_holdings(&trade.symbol, PortfolioQty::from(updated));

                    //Because the order has completed, we should be able to unwrap pending_orders safetly
                    //If this fails then there must be an application bug and panic is required.
                    let pending = self.pending_orders.get(&trade.symbol).unwrap_or_default();

                    let updated_pending = match trade.typ {
                        TradeType::Buy => *pending - trade.quantity,
                        TradeType::Sell => *pending + trade.quantity,
                    };
                    if updated_pending == 0.0 {
                        self.pending_orders.remove(&trade.symbol);
                    } else {
                        self.pending_orders
                            .insert(&trade.symbol, &PortfolioQty::from(updated_pending));
                    }

                    self.last_seen_trade += 1;
                }
            }
        }
        //Previous step can cause negative cash balance so we have to rebalance here, this
        //is not instant so will never balance properly if the series is very volatile
        self.rebalance_cash();
    }
}

impl<C: UistClient> UistBroker<C> {
    pub fn cost_basis(&self, symbol: &str) -> Option<Price> {
        self.log.cost_basis(symbol)
    }

    pub fn trades_between(&self, start: &i64, stop: &i64) -> Vec<Trade> {
        self.log.trades_between(start, stop)
    }
}

pub struct UistBrokerBuilder<C: UistClient> {
    trade_costs: Vec<BrokerCost>,
    client: Option<C>,
    backtest_id: Option<BacktestId>,
}

impl<C: UistClient> UistBrokerBuilder<C> {
    pub async fn build(&mut self) -> UistBroker<C> {
        if self.client.is_none() {
            panic!("Cannot build broker without client");
        }

        let mut client = mem::take(&mut self.client).unwrap();
        let backtest_id = mem::take(&mut self.backtest_id).unwrap();

        //If we don't have quotes on first tick, we shouldn't error but we should expect every
        //`DataSource` to provide a first tick
        let mut first_quotes = HashMap::new();
        let quote_response = client.fetch_quotes(backtest_id).await.unwrap();
        for quote in &quote_response.quotes {
            first_quotes.insert(quote.symbol.clone(), quote.clone());
        }

        let holdings = PortfolioHoldings::new();
        let pending_orders = PortfolioHoldings::new();
        let log = UistBrokerLog::new();

        UistBroker {
            //Intialised as invalid so errors throw if client tries to run before init
            holdings,
            pending_orders,
            cash: CashValue::from(0.0),
            log,
            last_seen_trade: 0,
            trade_costs: self.trade_costs.clone(),
            latest_quotes: first_quotes,
            broker_state: BrokerState::Ready,
            http_client: client,
            backtest_id,
        }
    }

    pub fn with_client(&mut self, client: C, backtest_id: BacktestId) -> &mut Self {
        self.client = Some(client);
        self.backtest_id = Some(backtest_id);
        self
    }

    pub fn with_trade_costs(&mut self, trade_costs: Vec<BrokerCost>) -> &mut Self {
        self.trade_costs = trade_costs;
        self
    }

    pub fn new() -> Self {
        UistBrokerBuilder {
            trade_costs: Vec::new(),
            client: None,
            backtest_id: None,
        }
    }
}

impl<C: UistClient> Default for UistBrokerBuilder<C> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub enum UistRecordedEvent {
    TradeCompleted(Trade),
}

impl From<Trade> for UistRecordedEvent {
    fn from(value: Trade) -> Self {
        UistRecordedEvent::TradeCompleted(value)
    }
}

//Records events generated by brokers. Used for internal calculations but is public for tax
//calculations.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct UistBrokerLog {
    log: Vec<UistRecordedEvent>,
}

impl UistBrokerLog {
    pub fn record<E: Into<UistRecordedEvent>>(&mut self, event: E) {
        let brokerevent: UistRecordedEvent = event.into();
        self.log.push(brokerevent);
    }

    pub fn trades(&self) -> Vec<Trade> {
        let mut trades = Vec::new();
        for event in &self.log {
            let UistRecordedEvent::TradeCompleted(trade) = event;
            trades.push(trade.clone());
        }
        trades
    }

    pub fn trades_between(&self, start: &i64, stop: &i64) -> Vec<Trade> {
        let trades = self.trades();
        trades
            .iter()
            .filter(|v| v.date >= *DateTime::from(*start) && v.date <= *DateTime::from(*stop))
            .cloned()
            .collect_vec()
    }

    pub fn cost_basis(&self, symbol: &str) -> Option<Price> {
        let mut cum_qty = PortfolioQty::default();
        let mut cum_val = CashValue::default();
        for event in &self.log {
            let UistRecordedEvent::TradeCompleted(trade) = event;
            if trade.symbol.eq(symbol) {
                match trade.typ {
                    TradeType::Buy => {
                        cum_qty = PortfolioQty::from(*cum_qty + trade.quantity);
                        cum_val = CashValue::from(*cum_val + trade.value);
                    }
                    TradeType::Sell => {
                        cum_qty = PortfolioQty::from(*cum_qty - trade.quantity);
                        cum_val = CashValue::from(*cum_val - trade.value);
                    }
                }
                //reset the value if we are back to zero
                if (*cum_qty).eq(&0.0) {
                    cum_val = CashValue::default();
                }
            }
        }
        if (*cum_qty).eq(&0.0) {
            return None;
        }
        Some(Price::from(*cum_val / *cum_qty))
    }
}

impl UistBrokerLog {
    pub fn new() -> Self {
        UistBrokerLog { log: Vec::new() }
    }
}

impl Default for UistBrokerLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {

    use std::collections::HashMap;

    use crate::broker::{
        BrokerCashEvent, BrokerCost, BrokerOperations, CashOperations, Portfolio, SendOrder, Update,
    };
    use crate::types::{CashValue, PortfolioAllocation, PortfolioQty};
    use rotala::clock::{ClockBuilder, Frequency};
    use rotala::exchange::uist_v1::{
        random_uist_generator, Order, OrderType, Trade, TradeType, UistV1,
    };
    use rotala::http::uist::uistv1_client::{Client, TestClient, UistClient};

    use rotala::input::penelope::PenelopeBuilder;

    use super::{UistBroker, UistBrokerBuilder, UistBrokerEvent, UistBrokerLog};

    async fn setup() -> UistBroker<TestClient> {
        let mut source_builder = PenelopeBuilder::new();

        source_builder.add_quote(100.00, 101.00, 100, "ABC");
        source_builder.add_quote(10.00, 11.00, 100, "BCD");

        source_builder.add_quote(104.00, 105.00, 101, "ABC");
        source_builder.add_quote(14.00, 15.00, 101, "BCD");

        source_builder.add_quote(95.00, 96.00, 102, "ABC");
        source_builder.add_quote(10.00, 11.00, 102, "BCD");

        source_builder.add_quote(95.00, 96.00, 103, "ABC");
        source_builder.add_quote(10.00, 11.00, 103, "BCD");

        let (price_source, clock) =
            source_builder.build_with_frequency(rotala::clock::Frequency::Second);

        let exchange = UistV1::new(clock, price_source, "Random");
        let mut datasets = HashMap::new();
        datasets.insert("Random".to_string(), exchange);
        let mut client = TestClient::new(&mut datasets);

        let resp = client.init("Random".to_string()).await.unwrap();

        let brkr = UistBrokerBuilder::new()
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .with_client(client, resp.backtest_id)
            .build()
            .await;

        brkr
    }

    #[tokio::test]
    async fn test_cash_deposit_withdraw() {
        let mut brkr = setup().await;
        brkr.deposit_cash(&100.0);

        brkr.check();

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
        let mut brkr = setup().await;
        brkr.deposit_cash(&100_000.0);

        let res = brkr.send_order(Order::market_buy("ABC", 495.0));
        println!("{:?}", res);
        assert!(matches!(res, UistBrokerEvent::OrderSentToExchange(..)));

        brkr.check();

        let cash = brkr.get_cash_balance();
        assert!(*cash < 100_000.0);

        let qty = brkr
            .get_position_qty("ABC")
            .unwrap_or(PortfolioQty::from(0.0));
        assert_eq!(*qty.clone(), 495.00);
    }

    #[tokio::test]
    async fn test_that_buy_order_larger_than_cash_fails_with_error_returned_without_panic() {
        let mut brkr = setup().await;
        brkr.deposit_cash(&100.0);
        //Order value is greater than cash balance
        let res = brkr.send_order(Order::market_buy("ABC", 495.0));

        assert!(matches!(res, UistBrokerEvent::OrderInvalid(..)));
        brkr.check();

        let cash = brkr.get_cash_balance();
        assert!(*cash == 100.0);
    }

    #[tokio::test]
    async fn test_that_sell_order_larger_than_holding_fails_with_error_returned_without_panic() {
        let mut brkr = setup().await;
        brkr.deposit_cash(&100_000.0);

        let res = brkr.send_order(Order::market_buy("ABC", 100.0));
        assert!(matches!(res, UistBrokerEvent::OrderSentToExchange(..)));
        brkr.check();

        //Order greater than current holding
        brkr.check();

        let res = brkr.send_order(Order::market_sell("ABC", 105.0));
        assert!(matches!(res, UistBrokerEvent::OrderInvalid(..)));

        //Checking that
        let qty = brkr.get_position_qty("ABC").unwrap_or_default();
        println!("{:?}", qty);
        assert!((*qty.clone()).eq(&100.0));
    }

    #[tokio::test]
    async fn test_that_market_sell_increases_cash_and_decreases_holdings() {
        let mut brkr = setup().await;
        brkr.deposit_cash(&100_000.0);
        let res = brkr.send_order(Order::market_buy("ABC", 495.0));
        assert!(matches!(res, UistBrokerEvent::OrderSentToExchange(..)));
        brkr.check();
        let cash = brkr.get_cash_balance();

        brkr.check();

        let res = brkr.send_order(Order::market_sell("ABC", 295.0));
        assert!(matches!(res, UistBrokerEvent::OrderSentToExchange(..)));

        brkr.check();
        let cash0 = brkr.get_cash_balance();

        let qty = brkr.get_position_qty("ABC").unwrap_or_default();
        assert_eq!(*qty, 200.0);
        assert!(*cash0 > *cash);
    }

    #[tokio::test]
    async fn test_that_valuation_updates_in_next_period() {
        let mut brkr = setup().await;
        brkr.deposit_cash(&100_000.0);

        brkr.send_order(Order::market_buy("ABC", 495.0));
        brkr.check();

        let val = brkr.get_position_value("ABC");

        brkr.check();
        let val1 = brkr.get_position_value("ABC");
        assert_ne!(val, val1);
    }

    #[tokio::test]
    async fn test_that_profit_calculation_is_accurate() {
        let mut brkr = setup().await;
        brkr.deposit_cash(&100_000.0);
        brkr.send_order(Order::market_buy("ABC", 495.0));
        brkr.check();

        brkr.check();

        let profit = brkr.get_position_profit("ABC").unwrap();
        assert_eq!(*profit, -4950.00);
    }

    #[tokio::test]
    async fn test_that_broker_uses_last_value_if_it_fails_to_find_quote() {
        //If the broker cannot find a quote in the current period for a stock, it automatically
        //uses a value of zero. This is a problem because the current time could a weekend or
        //bank holiday, and if the broker is attempting to value the portfolio on that day
        //they will ask for a quote, not find one, and then use a value of zero which is
        //incorrect.
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(100.00, 101.00, 100, "ABC");
        source_builder.add_quote(10.00, 11.00, 100, "BCD");

        //Trades execute here
        source_builder.add_quote(100.00, 101.00, 101, "ABC");
        source_builder.add_quote(10.00, 11.00, 101, "BCD");

        //We are missing a quote for BCD on 101, but the broker should return the last seen value
        source_builder.add_quote(104.00, 105.00, 102, "ABC");

        //And when we check the next date, it updates correctly
        source_builder.add_quote(104.00, 105.00, 103, "ABC");
        source_builder.add_quote(12.00, 13.00, 103, "BCD");

        let (price_source, clock) =
            source_builder.build_with_frequency(rotala::clock::Frequency::Second);
        let exchange = UistV1::new(clock, price_source, "Random");
        let mut datasets = HashMap::new();
        datasets.insert("Random".to_string(), exchange);
        let mut client = TestClient::new(&mut datasets);
        let resp = client.init("Random".to_string()).await.unwrap();

        let mut brkr = UistBrokerBuilder::new()
            .with_client(client, resp.backtest_id)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build()
            .await;

        brkr.deposit_cash(&100_000.0);

        brkr.send_order(Order::market_buy("ABC", 100.0));
        brkr.send_order(Order::market_buy("BCD", 100.0));

        brkr.check();

        //Missing live quote for BCD
        brkr.check();
        let value = brkr
            .get_position_value("BCD")
            .unwrap_or(CashValue::from(0.0));
        println!("{:?}", value);
        //We test against the bid price, which gives us the value exclusive of the price paid at ask
        assert!(*value == 10.0 * 100.0);

        //BCD has quote again
        brkr.check();

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

        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(100.00, 101.00, 100, "ABC");
        source_builder.add_quote(150.00, 151.00, 101, "ABC");
        source_builder.add_quote(150.00, 151.00, 102, "ABC");

        let (price_source, clock) =
            source_builder.build_with_frequency(rotala::clock::Frequency::Second);
        let exchange = UistV1::new(clock, price_source, "Random");
        let mut datasets = HashMap::new();
        datasets.insert("Random".to_string(), exchange);
        let mut client = TestClient::new(&mut datasets);
        let resp = client.init("Random".to_string()).await.unwrap();

        let mut brkr = UistBrokerBuilder::new()
            .with_client(client, resp.backtest_id)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build()
            .await;

        brkr.deposit_cash(&100_000.0);
        //Because the price of ABC rises after this order is sent, we will end up with a negative
        //cash balance after the order is executed
        brkr.send_order(Order::market_buy("ABC", 700.0));

        //Trades execute
        brkr.check();

        let cash = brkr.get_cash_balance();
        assert!(*cash < 0.0);

        //Broker rebalances to raise cash
        brkr.check();
        let cash1 = brkr.get_cash_balance();
        assert!(*cash1 > 0.0);
    }

    #[tokio::test]
    async fn test_that_broker_stops_when_liquidation_fails() {
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(100.00, 101.00, 100, "ABC");
        //Price doubles over one tick so that the broker is trading on information that has become
        //very inaccurate
        source_builder.add_quote(200.00, 201.00, 101, "ABC");
        source_builder.add_quote(200.00, 201.00, 101, "ABC");

        let (price_source, clock) =
            source_builder.build_with_frequency(rotala::clock::Frequency::Second);
        let exchange = UistV1::new(clock, price_source, "Random");
        let mut datasets = HashMap::new();
        datasets.insert("Random".to_string(), exchange);
        let mut client = TestClient::new(&mut datasets);
        let resp = client.init("Random".to_string()).await.unwrap();

        let mut brkr = UistBrokerBuilder::new()
            .with_client(client, resp.backtest_id)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build()
            .await;

        brkr.deposit_cash(&100_000.0);
        //This will use all the available cash balance, the market price doubles so the broker ends
        //up with a shortfall of -100_000.

        brkr.send_order(Order::market_buy("ABC", 990.0));

        brkr.check();
        brkr.check();

        let cash = brkr.get_cash_balance();
        assert!(*cash < 0.0);

        let res = brkr.send_order(Order::market_buy("ABC", 100.0));
        assert!(matches!(res, UistBrokerEvent::OrderInvalid { .. }));

        assert!(matches!(
            brkr.deposit_cash(&100_000.0),
            BrokerCashEvent::OperationFailure { .. }
        ));
        assert!(matches!(
            brkr.withdraw_cash(&100_000.0),
            BrokerCashEvent::OperationFailure { .. }
        ));
    }

    #[tokio::test]
    async fn test_that_holdings_updates_correctly() {
        let mut brkr = setup().await;
        brkr.deposit_cash(&100_000.0);
        let res = brkr.send_order(Order::market_buy("ABC", 50.0));
        assert!(matches!(res, UistBrokerEvent::OrderSentToExchange(..)));
        assert_eq!(
            *brkr
                .get_holdings_with_pending()
                .get("ABC")
                .unwrap_or_default(),
            50.0
        );
        brkr.check();
        assert_eq!(*brkr.get_holdings().get("ABC").unwrap_or_default(), 50.0);

        let res = brkr.send_order(Order::market_sell("ABC", 10.0));
        assert!(matches!(res, UistBrokerEvent::OrderSentToExchange(..)));
        assert_eq!(
            *brkr
                .get_holdings_with_pending()
                .get("ABC")
                .unwrap_or_default(),
            40.0
        );
        brkr.check();
        assert_eq!(*brkr.get_holdings().get("ABC").unwrap_or_default(), 40.0);

        let res = brkr.send_order(Order::market_buy("ABC", 50.0));
        assert!(matches!(res, UistBrokerEvent::OrderSentToExchange(..)));
        assert_eq!(
            *brkr
                .get_holdings_with_pending()
                .get("ABC")
                .unwrap_or_default(),
            90.0
        );
        brkr.check();
        assert_eq!(*brkr.get_holdings().get("ABC").unwrap_or_default(), 90.0)
    }

    fn setup_log() -> UistBrokerLog {
        let mut rec = UistBrokerLog::new();

        let t1 = Trade::new("ABC", 100.0, 10.00, 100, TradeType::Buy);
        let t2 = Trade::new("ABC", 500.0, 90.00, 101, TradeType::Buy);
        let t3 = Trade::new("BCD", 100.0, 100.0, 102, TradeType::Buy);
        let t4 = Trade::new("BCD", 500.0, 100.00, 103, TradeType::Sell);
        let t5 = Trade::new("BCD", 50.0, 50.00, 104, TradeType::Buy);

        rec.record(t1);
        rec.record(t2);
        rec.record(t3);
        rec.record(t4);
        rec.record(t5);
        rec
    }

    #[test]
    fn test_that_log_filters_trades_between_dates() {
        let log = setup_log();
        let between = log.trades_between(&102.into(), &104.into());
        assert!(between.len() == 3);
    }

    #[test]
    fn test_that_log_calculates_the_cost_basis() {
        let log = setup_log();
        let abc_cost = log.cost_basis("ABC").unwrap();
        let bcd_cost = log.cost_basis("BCD").unwrap();

        assert_eq!(*abc_cost, 6.0);
        assert_eq!(*bcd_cost, 1.0);
    }

    #[tokio::test]
    async fn diff_direction_correct_if_need_to_buy() {
        let (uist, clock) = random_uist_generator(100);

        let mut datasets = HashMap::new();
        datasets.insert("Random".to_string(), uist);
        let mut client = TestClient::new(&mut datasets);
        let resp = client.init("Random".to_string()).await.unwrap();

        let mut brkr = UistBrokerBuilder::new()
            .with_client(client, resp.backtest_id)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build()
            .await;

        let mut weights = PortfolioAllocation::new();
        weights.insert("ABC", 1.0);

        brkr.deposit_cash(&100_000.0);
        brkr.check();

        let orders = brkr.diff_brkr_against_target_weights(&weights);

        println!("{:?}", orders);
        let first = orders.first().unwrap();
        assert!(matches!(
            first.get_order_type(),
            OrderType::MarketBuy { .. }
        ));
    }

    #[tokio::test]
    async fn diff_direction_correct_if_need_to_sell() {
        //This is connected to the previous test, if the above fails then this will never pass.
        //However, if the above passes this could still fail.

        let (uist, clock) = random_uist_generator(100);
        let mut datasets = HashMap::new();
        datasets.insert("Random".to_string(), uist);
        let mut client = TestClient::new(&mut datasets);
        let resp = client.init("Random".to_string()).await.unwrap();

        let mut brkr = UistBrokerBuilder::new()
            .with_client(client, resp.backtest_id)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build()
            .await;

        let mut weights = PortfolioAllocation::new();
        weights.insert("ABC", 1.0);

        brkr.deposit_cash(&100_000.0);
        let orders = brkr.diff_brkr_against_target_weights(&weights);
        brkr.send_orders(&orders);

        brkr.check();

        brkr.check();

        let mut weights1 = PortfolioAllocation::new();
        //This weight needs to very small because it is possible for the data generator to generate
        //a price that drops significantly meaning that rebalancing requires a buy not a sell. This
        //is unlikely but seems to happen eventually.
        weights1.insert("ABC", 0.01);
        let orders1 = brkr.diff_brkr_against_target_weights(&weights1);

        println!("{:?}", orders1);
        let first = orders1.first().unwrap();
        assert!(matches!(
            first.get_order_type(),
            OrderType::MarketSell { .. }
        ));
    }

    #[tokio::test]
    async fn diff_continues_if_security_missing() {
        //In this scenario, the user has inserted incorrect information but this scenario can also occur if there is no quote
        //for a given security on a certain date. We are interested in the latter case, not the former but it is more
        //difficult to test for the latter, and the code should be the same.
        let (uist, clock) = random_uist_generator(100);
        let mut datasets = HashMap::new();
        datasets.insert("Random".to_string(), uist);
        let mut client = TestClient::new(&mut datasets);
        let resp = client.init("Random".to_string()).await.unwrap();

        let mut brkr = UistBrokerBuilder::new()
            .with_client(client, resp.backtest_id)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build()
            .await;

        let mut weights = PortfolioAllocation::new();
        weights.insert("ABC", 0.5);
        //There is no quote for this security in the underlying data, code should make the assumption (that doesn't apply here)
        //that there is some quote for this security at a later date and continues to generate order for ABC without throwing
        //error
        weights.insert("XYZ", 0.5);

        brkr.deposit_cash(&100_000.0);
        brkr.check();
        let orders = brkr.diff_brkr_against_target_weights(&weights);
        assert!(orders.len() == 1);
    }

    #[tokio::test]
    #[should_panic]
    async fn diff_panics_if_brkr_has_no_cash() {
        //If we get to a point where the client is diffing without cash, we can assume that no further operations are possible
        //and we should panic
        let (uist, clock) = random_uist_generator(100);
        let mut datasets = HashMap::new();
        datasets.insert("Random".to_string(), uist);
        let mut client = TestClient::new(&mut datasets);
        let resp = client.init("Random".to_string()).await.unwrap();

        let mut brkr = UistBrokerBuilder::new()
            .with_client(client, resp.backtest_id)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build()
            .await;

        let mut weights = PortfolioAllocation::new();
        weights.insert("ABC", 1.0);

        brkr.check();
        brkr.diff_brkr_against_target_weights(&weights);
    }

    #[test]
    fn can_estimate_trade_costs_of_proposed_trade() {
        let pershare = BrokerCost::per_share(0.1);
        let flat = BrokerCost::flat(10.0);
        let pct = BrokerCost::pct_of_value(0.01);

        let res = pershare.trade_impact(&1000.0, &1.0, true);
        assert!((*res.1).eq(&1.1));

        let res = pershare.trade_impact(&1000.0, &1.0, false);
        assert!((*res.1).eq(&0.9));

        let res = flat.trade_impact(&1000.0, &1.0, true);
        assert!((*res.0).eq(&990.00));

        let res = pct.trade_impact(&100.0, &1.0, true);
        assert!((*res.0).eq(&99.0));

        let costs = vec![pershare, flat];
        let initial = BrokerCost::trade_impact_total(&costs, &1000.0, &1.0, true);
        assert!((*initial.0).eq(&990.00));
        assert!((*initial.1).eq(&1.1));
    }

    #[tokio::test]
    async fn diff_handles_sent_but_unexecuted_orders() {
        //It is possible for the client to issue orders for infinitely increasing numbers of shares
        //if there is a gap between orders being issued and executed. For example, if we are
        //missing price data the client could think we need 100 shares, that order doesn't get
        //executed on the next tick, and the client then issues orders for another 100 shares.
        //
        //This is not possible without earlier price data either. If there is no price data then
        //the diff will be unable to work out how many shares are required. So the test case is
        //some price but no price for the execution period.
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(100.00, 100.00, 100, "ABC");
        source_builder.add_quote(100.00, 100.00, 101, "ABC");
        source_builder.add_quote(100.00, 100.00, 103, "ABC");

        let (price_source, clock) =
            source_builder.build_with_frequency(rotala::clock::Frequency::Second);
        let exchange = UistV1::new(clock, price_source, "Random");
        let mut datasets = HashMap::new();
        datasets.insert("Random".to_string(), exchange);
        let mut client = TestClient::new(&mut datasets);
        let resp = client.init("Random".to_string()).await.unwrap();

        let mut brkr = UistBrokerBuilder::new()
            .with_client(client, resp.backtest_id)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build()
            .await;

        brkr.deposit_cash(&100_000.0);

        //No price for security so we haven't diffed correctly
        brkr.check();

        brkr.check();

        let mut target_weights = PortfolioAllocation::new();
        target_weights.insert("ABC", 0.9);

        let orders = brkr.diff_brkr_against_target_weights(&target_weights);
        brkr.send_orders(&orders);

        brkr.check();

        let orders1 = brkr.diff_brkr_against_target_weights(&target_weights);

        brkr.send_orders(&orders1);
        brkr.check();

        dbg!(brkr.get_position_qty("ABC"));
        //If the logic isn't correct the orders will have doubled up to 1800
        assert_eq!(*brkr.get_position_qty("ABC").unwrap(), 900.0);
    }

    #[tokio::test]
    async fn diff_handles_case_when_existing_order_requires_sell_to_rebalance() {
        //Tests similar scenario to previous test but for the situation in which the price is
        //missing, and we try to rebalance by buying but the pending order is for a significantly
        //greater amount of shares than we now need (e.g. we have a price of X, we miss a price,
        //and then it drops 20%).
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(100.00, 100.00, 100, "ABC");
        source_builder.add_quote(75.00, 75.00, 103, "ABC");
        source_builder.add_quote(75.00, 75.00, 104, "ABC");

        let (price_source, clock) =
            source_builder.build_with_frequency(rotala::clock::Frequency::Second);
        let exchange = UistV1::new(clock, price_source, "Random");
        let mut datasets = HashMap::new();
        datasets.insert("Random".to_string(), exchange);
        let mut client = TestClient::new(&mut datasets);
        let resp = client.init("Random".to_string()).await.unwrap();

        let mut brkr = UistBrokerBuilder::new()
            .with_client(client, resp.backtest_id)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build()
            .await;

        brkr.deposit_cash(&100_000.0);

        let mut target_weights = PortfolioAllocation::new();
        target_weights.insert("ABC", 0.9);
        let orders = brkr.diff_brkr_against_target_weights(&target_weights);
        println!("{:?}", orders);

        brkr.send_orders(&orders);

        //No price for security so we haven't diffed correctly
        brkr.check();

        brkr.check();

        brkr.check();

        let orders1 = brkr.diff_brkr_against_target_weights(&target_weights);
        println!("{:?}", orders1);

        brkr.send_orders(&orders1);

        brkr.check();

        println!("{:?}", brkr.get_holdings());
        //If the logic isn't correct then the order will be for less shares than is actually
        //required by the newest price
        assert_eq!(*brkr.get_position_qty("ABC").unwrap(), 1200.0);
    }
}
