mod staticweight;
pub use staticweight::{
    AsyncStaticWeightStrategy, AsyncStaticWeightStrategyBuilder, StaticWeightStrategy,
    StaticWeightStrategyBuilder,
};

use async_trait::async_trait;

use crate::broker::{DividendPayment, Trade};
use crate::types::{CashValue, StrategySnapshot};

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
#[async_trait]
pub trait AsyncStrategy: TransferTo {
    async fn update(&mut self);
    async fn init(&mut self, initial_cash: &f64);
}

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
#[async_trait]
pub trait AsyncTransferFrom {
    fn withdraw_cash(&mut self, cash: &f64) -> StrategyEvent;
    async fn withdraw_cash_with_liquidation(&mut self, cash: &f64) -> StrategyEvent;
}

pub trait TransferFrom {
    fn withdraw_cash(&mut self, cash: &f64) -> StrategyEvent;
    fn withdraw_cash_with_liquidation(&mut self, cash: &f64) -> StrategyEvent;
}

///Strategy records and can return history to client. This history is used for performance
///calculations. [StrategySnapshot] is a struct defined in the perf module.
pub trait History {
    fn get_history(&self) -> Vec<StrategySnapshot>;
}

#[cfg(test)]
mod tests {
    use super::AsyncStaticWeightStrategyBuilder;
    use crate::broker::{BrokerCost, ConcurrentBroker, ConcurrentBrokerBuilder, Dividend, Quote};
    use crate::clock::{Clock, ClockBuilder};
    use crate::exchange::ConcurrentExchangeBuilder;
    use crate::input::{HashMapCorporateEventsSource, HashMapPriceSource};
    use crate::types::{Frequency, PortfolioAllocation};

    async fn setup() -> (
        ConcurrentBroker<Dividend, HashMapCorporateEventsSource<Dividend>, Quote>,
        Clock,
    ) {
        let clock = ClockBuilder::with_length_in_dates(100, 102)
            .with_frequency(&Frequency::Second)
            .build();

        let mut price_source = HashMapPriceSource::new(clock.clone());
        price_source.add_quotes(100, Quote::new(100.00, 101.00, 100, "ABC"));
        price_source.add_quotes(101, Quote::new(104.00, 105.00, 101, "ABC"));
        price_source.add_quotes(102, Quote::new(95.00, 96.00, 102, "ABC"));

        let mut exchange = ConcurrentExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_price_source(price_source)
            .build();

        let brkr: ConcurrentBroker<Dividend, HashMapCorporateEventsSource<Dividend>, Quote> =
            ConcurrentBrokerBuilder::new()
                .with_trade_costs(vec![BrokerCost::flat(0.1)])
                .build(&mut exchange)
                .await;
        (brkr, clock)
    }

    #[tokio::test]
    #[should_panic]
    async fn test_that_static_builder_fails_without_weights() {
        let comp = setup().await;
        let _strat = AsyncStaticWeightStrategyBuilder::<
            Dividend,
            HashMapCorporateEventsSource<Dividend>,
            Quote,
        >::new()
        .with_brkr(comp.0)
        .with_clock(comp.1)
        .default();
    }

    #[tokio::test]
    #[should_panic]
    async fn test_that_static_builder_fails_without_brkr() {
        let comp = setup().await;
        let weights = PortfolioAllocation::new();
        let _strat = AsyncStaticWeightStrategyBuilder::<
            Dividend,
            HashMapCorporateEventsSource<Dividend>,
            Quote,
        >::new()
        .with_weights(weights)
        .with_clock(comp.1)
        .default();
    }

    #[tokio::test]
    #[should_panic]
    async fn test_that_static_builder_fails_without_clock() {
        let comp = setup().await;
        let weights = PortfolioAllocation::new();
        let _strat = AsyncStaticWeightStrategyBuilder::<
            Dividend,
            HashMapCorporateEventsSource<Dividend>,
            Quote,
        >::new()
        .with_weights(weights)
        .with_brkr(comp.0)
        .default();
    }
}
