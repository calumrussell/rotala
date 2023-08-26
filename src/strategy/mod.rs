use async_trait::async_trait;
use log::info;

use crate::broker::{
    BacktestBroker, BrokerCalculations, BrokerCashEvent, DividendPayment, EventLog, Trade,
    TransferCash,
};
use crate::clock::Clock;
use crate::input::{DataSource, Dividendable, Quotable};
use crate::schedule::{DefaultTradingSchedule, TradingSchedule};
use crate::sim::SimulatedBroker;
use crate::types::{CashValue, PortfolioAllocation, StrategySnapshot};

///Strategies define an a set of operations that should be performed on some schedule to bring the
///broker passed to the strategy into the desired state.
///
///Strategies can have their own data dependencies seperate from Broker but, at least in a
///backtest, care should be taken to give that data source a reference to a `Clock` so that the
///date is updated correctly across the backtest components.
///
///The strategy target is represented in the `StaticWeightStrategy` implementation as percentages
///of portfolio but there is no need to do so. Brokers just accept a series of orders so it does
///not matter how these orders are created.
///
///The `StaticWeightStrategy` implementation has a reference to `Clock` but a direct reference is
///not required to run the strategy, it is only used to record the state for performance calcs. Strategy
///implementations should run idempotently, although some with a dependence on external data which
///has it's own state, without much additional state

///The `Strategy` trait defines the key lifecycle events that are required to create and run a backtest.
///This functionality is closely bound into `SimContext` which is the struct that wraps around the
///components of a backtest, runs it, and offers the interface into the components (like
///`Strategy`) to clients. The reasoning for this is explained in the documentation for
///`SimContext`.
pub trait Strategy: TransferTo {
    fn update(&mut self);
    fn init(&mut self, initial_cash: &f64);
}

///Defines events that can be triggered by the client that modify the internal state of the
///strategy somehow. This doesn't refer to internally-generated events such as order creation but
///only events that the client can trigger at the start or during a simulation run.
///
///Only used for clients that implement [TransferTo] and/or [TransferFrom] traits.
pub enum StrategyEvent {
    //Mirrors BrokerEvent
    WithdrawSuccess(CashValue),
    WithdrawFailure(CashValue),
    DepositSuccess(CashValue),
}

///Defines a set of functions on a strategy that reports events to clients. This is used
///practically for tax calculations at the moment. Mirrors methods on broker.
pub trait Audit {
    fn trades_between(&self, start: &i64, end: &i64) -> Vec<Trade>;
    fn dividends_between(&self, start: &i64, end: &i64) -> Vec<DividendPayment>;
}

///Trait to transfer cash into a strategy either at the start or whilst it is running. This is in a
///separate trait as some clients may wish to create strategies to wish no further cash can be
///deposited in the course of a simulation.
pub trait TransferTo {
    fn deposit_cash(&mut self, cash: &f64) -> StrategyEvent;
}

///Trait to withdraw cash from a strategy, typically whilst it is running. This is a separate trait
///as some clients may wish to create strategies from which no cash can be withdrawn whilst the
///simulation is running.
pub trait TransferFrom {
    fn withdraw_cash(&mut self, cash: &f64) -> StrategyEvent;
    fn withdraw_cash_with_liquidation(&mut self, cash: &f64) -> StrategyEvent;
}

///Strategy records and can return history to client. This history is used for performance
///calculations. [StrategySnapshot] is a struct defined in the perf module.
pub trait History {
    fn get_history(&self) -> Vec<StrategySnapshot>;
}

pub struct StaticWeightStrategyBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    //If missing either field, we cannot run this strategy
    brkr: Option<SimulatedBroker<T, Q, D>>,
    weights: Option<PortfolioAllocation>,
    clock: Option<Clock>,
}

impl<T, Q, D> StaticWeightStrategyBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    pub fn default(&mut self) -> StaticWeightStrategy<T, Q, D> {
        if self.brkr.is_none() || self.weights.is_none() || self.clock.is_none() {
            panic!("Strategy must have broker, weights, and clock");
        }

        let brkr = std::mem::replace(&mut self.brkr, None);
        let weights = std::mem::replace(&mut self.weights, None);
        StaticWeightStrategy {
            brkr: brkr.unwrap(),
            target_weights: weights.unwrap(),
            net_cash_flow: 0.0.into(),
            clock: self.clock.as_ref().unwrap().clone(),
            history: Vec::new(),
        }
    }

    pub fn with_clock(&mut self, clock: Clock) -> &mut Self {
        self.clock = Some(clock);
        self
    }

    pub fn with_brkr(&mut self, brkr: SimulatedBroker<T, Q, D>) -> &mut Self {
        self.brkr = Some(brkr);
        self
    }

    pub fn with_weights(&mut self, weights: PortfolioAllocation) -> &mut Self {
        self.weights = Some(weights);
        self
    }

    pub fn new() -> Self {
        Self {
            brkr: None,
            weights: None,
            clock: None,
        }
    }
}

impl<T, Q, D> Default for StaticWeightStrategyBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn default() -> Self {
        Self::new()
    }
}

///Basic implementation of an investment strategy which takes a set of fixed-weight allocations and
///rebalances over time towards those weights.
pub struct StaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    brkr: SimulatedBroker<T, Q, D>,
    target_weights: PortfolioAllocation,
    net_cash_flow: CashValue,
    clock: Clock,
    history: Vec<StrategySnapshot>,
}

unsafe impl<T, Q, D> Send for StaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
}

impl<T, Q, D> StaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    pub fn get_snapshot(&mut self) -> StrategySnapshot {
        // Defaults to zero inflation because most users probably aren't looking
        // for real returns calcs
        let now = self.clock.now();
        StrategySnapshot {
            date: now,
            portfolio_value: self.brkr.get_total_value(),
            net_cash_flow: self.net_cash_flow.clone(),
            inflation: 0.0,
        }
    }
}

#[async_trait]
impl<T, Q, D> Strategy for StaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn init(&mut self, initital_cash: &f64) {
        self.deposit_cash(initital_cash);
        if DefaultTradingSchedule::should_trade(&self.clock.now()) {
            let orders = BrokerCalculations::diff_brkr_against_target_weights(
                &self.target_weights,
                &mut self.brkr,
            );
            if !orders.is_empty() {
                self.brkr.send_orders(&orders);
            }
        }
    }

    fn update(&mut self) {
        self.brkr.check();
        let now = self.clock.now();
        if DefaultTradingSchedule::should_trade(&now) {
            let orders = BrokerCalculations::diff_brkr_against_target_weights(
                &self.target_weights,
                &mut self.brkr,
            );
            if !orders.is_empty() {
                self.brkr.send_orders(&orders);
            }
        }
        let snap = self.get_snapshot();
        self.history.push(snap);
    }
}

impl<T, Q, D> TransferTo for StaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn deposit_cash(&mut self, cash: &f64) -> StrategyEvent {
        info!("STRATEGY: Depositing {:?} into strategy", cash);
        self.brkr.deposit_cash(cash);
        self.net_cash_flow = CashValue::from(cash + *self.net_cash_flow);
        StrategyEvent::DepositSuccess(CashValue::from(*cash))
    }
}

impl<T, Q, D> TransferFrom for StaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn withdraw_cash(&mut self, cash: &f64) -> StrategyEvent {
        if let BrokerCashEvent::WithdrawSuccess(withdrawn) = self.brkr.withdraw_cash(cash) {
            info!("STRATEGY: Succesfully withdrew {:?} from strategy", cash);
            self.net_cash_flow = CashValue::from(*self.net_cash_flow - *withdrawn);
            return StrategyEvent::WithdrawSuccess(CashValue::from(*cash));
        }
        info!("STRATEGY: Failed to withdraw {:?} from strategy", cash);
        StrategyEvent::WithdrawFailure(CashValue::from(*cash))
    }

    fn withdraw_cash_with_liquidation(&mut self, cash: &f64) -> StrategyEvent {
        if let BrokerCashEvent::WithdrawSuccess(withdrawn) =
            //No logging here because the implementation is fully logged due to the greater
            //complexity of this task vs standard withdraw
            BrokerCalculations::withdraw_cash_with_liquidation(cash, &mut self.brkr)
        {
            self.net_cash_flow = CashValue::from(*self.net_cash_flow - *withdrawn);
            return StrategyEvent::WithdrawSuccess(CashValue::from(*cash));
        }
        StrategyEvent::WithdrawFailure(CashValue::from(*cash))
    }
}

impl<T, Q, D> Audit for StaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn trades_between(&self, start: &i64, end: &i64) -> Vec<Trade> {
        self.brkr.trades_between(start, end)
    }

    fn dividends_between(&self, start: &i64, end: &i64) -> Vec<DividendPayment> {
        self.brkr.dividends_between(start, end)
    }
}

impl<T, Q, D> History for StaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn get_history(&self) -> Vec<StrategySnapshot> {
        self.history.clone()
    }
}

#[cfg(test)]
mod tests {

    use std::collections::HashMap;
    use std::sync::Arc;

    use super::StaticWeightStrategyBuilder;
    use crate::broker::{BrokerCost, Dividend, Quote};
    use crate::clock::{Clock, ClockBuilder};
    use crate::exchange::builder::DefaultExchangeBuilder;
    use crate::input::{HashMapInput, HashMapInputBuilder};
    use crate::sim::{SimulatedBroker, SimulatedBrokerBuilder};
    use crate::types::{DateTime, Frequency, PortfolioAllocation};

    fn setup() -> (SimulatedBroker<HashMapInput, Quote, Dividend>, Clock) {
        let mut prices: HashMap<DateTime, Vec<Arc<Quote>>> = HashMap::new();

        let quote = Arc::new(Quote::new(100.00, 101.00, 100, "ABC"));
        let quote2 = Arc::new(Quote::new(104.00, 105.00, 101, "ABC"));
        let quote4 = Arc::new(Quote::new(95.00, 96.00, 102, "ABC"));
        prices.insert(100.into(), vec![quote]);
        prices.insert(101.into(), vec![quote2]);
        prices.insert(102.into(), vec![quote4]);

        let clock = ClockBuilder::with_length_in_dates(100, 102)
            .with_frequency(&Frequency::Second)
            .build();

        let source = HashMapInputBuilder::new()
            .with_quotes(prices)
            .with_clock(clock.clone())
            .build();

        let mut exchange = DefaultExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_data_source(source.clone())
            .build();

        let brkr = SimulatedBrokerBuilder::<HashMapInput, Quote, Dividend>::new()
            .with_data(source)
            .with_trade_costs(vec![BrokerCost::flat(0.1)])
            .build(&mut exchange);
        (brkr, clock)
    }

    #[test]
    #[should_panic]
    fn test_that_static_builder_fails_without_weights() {
        let comp = setup();
        let _strat = StaticWeightStrategyBuilder::<HashMapInput, Quote, Dividend>::new()
            .with_brkr(comp.0)
            .with_clock(comp.1)
            .default();
    }

    #[test]
    #[should_panic]
    fn test_that_static_builder_fails_without_brkr() {
        let comp = setup();
        let weights = PortfolioAllocation::new();
        let _strat = StaticWeightStrategyBuilder::<HashMapInput, Quote, Dividend>::new()
            .with_weights(weights)
            .with_clock(comp.1)
            .default();
    }

    #[test]
    #[should_panic]
    fn test_that_static_builder_fails_without_clock() {
        let comp = setup();
        let weights = PortfolioAllocation::new();
        let _strat = StaticWeightStrategyBuilder::<HashMapInput, Quote, Dividend>::new()
            .with_weights(weights)
            .with_brkr(comp.0)
            .default();
    }
}
