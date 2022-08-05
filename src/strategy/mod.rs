use log::info;
use std::rc::Rc;

use crate::broker::{
    BrokerCalculations, BrokerEvent, DividendPayment, EventLog, ExecutesOrder, PositionInfo, Trade,
    TransferCash,
};
use crate::clock::Clock;
use crate::input::DataSource;
use crate::perf::{PerfStruct, StrategyPerformance, StrategySnapshot};
use crate::schedule::{DefaultTradingSchedule, TradingSchedule};
use crate::sim::broker::SimulatedBroker;
use crate::types::{CashValue, DateTime, PortfolioAllocation, PortfolioWeight};

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
///not required to run the strategy, it is only used to update `StrategyPerformance`. Strategy
///implementations should run idempotently, although some with a dependence on external data which
///has it's own state, without much additional state

///The `Strategy` trait defines the key lifecycle events that are required to create and run a backtest.
///This functionality is closely bound into `SimContext` which is the struct that wraps around the
///components of a backtest, runs it, and offers the interface into the components (like
///`Strategy`) to clients. The reasoning for this is explained in the documentation for
///`SimContext`.
pub trait Strategy: TransferTo + Clone {
    fn update(&mut self) -> CashValue;
    fn init(&mut self, initial_cash: &CashValue);
    fn get_perf(&self) -> PerfStruct;
}

pub trait TransferTo {
    fn deposit_cash(&mut self, cash: &CashValue);
}

pub trait Audit {
    fn trades_between(&self, start: &DateTime, end: &DateTime) -> Vec<Trade>;
    fn dividends_between(&self, start: &DateTime, end: &DateTime) -> Vec<DividendPayment>;
}

pub trait TransferFrom {
    fn withdraw_cash(&mut self, cash: &CashValue);
    fn withdraw_cash_with_liquidation(&mut self, cash: &CashValue);
}

pub struct StaticWeightStrategyBuilder<T: DataSource> {
    //If missing either field, we cannot run this strategy
    brkr: Option<SimulatedBroker<T>>,
    weights: Option<PortfolioAllocation<PortfolioWeight>>,
    clock: Option<Clock>,
}

impl<T: DataSource> StaticWeightStrategyBuilder<T> {
    pub fn daily(&self) -> StaticWeightStrategy<T> {
        if self.brkr.is_none() || self.weights.is_none() || self.clock.is_none() {
            panic!("Strategy must have broker, weights, and clock");
        }

        StaticWeightStrategy {
            brkr: self.brkr.clone().unwrap(),
            target_weights: self.weights.clone().unwrap(),
            perf: StrategyPerformance::daily(),
            net_cash_flow: 0.0.into(),
            clock: Rc::clone(self.clock.as_ref().unwrap()),
        }
    }

    pub fn yearly(&self) -> StaticWeightStrategy<T> {
        if self.brkr.is_none() || self.weights.is_none() || self.clock.is_none() {
            panic!("Strategy must have broker, weights, and clock");
        }

        StaticWeightStrategy {
            brkr: self.brkr.clone().unwrap(),
            target_weights: self.weights.clone().unwrap(),
            perf: StrategyPerformance::yearly(),
            net_cash_flow: 0.0.into(),
            clock: Rc::clone(self.clock.as_ref().unwrap()),
        }
    }

    pub fn with_clock(&mut self, clock: Clock) -> &mut Self {
        self.clock = Some(clock);
        self
    }

    pub fn with_brkr(&mut self, brkr: SimulatedBroker<T>) -> &mut Self {
        self.brkr = Some(brkr);
        self
    }

    pub fn with_weights(&mut self, weights: PortfolioAllocation<PortfolioWeight>) -> &mut Self {
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

impl<T: DataSource> Default for StaticWeightStrategyBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

///Basic implementation of an investment strategy which takes a set of fixed-weight allocations and
///rebalances over time towards those weights.
#[derive(Clone)]
pub struct StaticWeightStrategy<T: DataSource> {
    brkr: SimulatedBroker<T>,
    target_weights: PortfolioAllocation<PortfolioWeight>,
    perf: StrategyPerformance,
    net_cash_flow: CashValue,
    clock: Clock,
}

impl<T: DataSource> StaticWeightStrategy<T> {
    pub fn get_snapshot(&self) -> StrategySnapshot {
        StrategySnapshot {
            date: self.clock.borrow().now(),
            value: self.brkr.get_total_value(),
            net_cash_flow: self.net_cash_flow,
        }
    }
}

impl<T: DataSource> Strategy for StaticWeightStrategy<T> {
    fn init(&mut self, initital_cash: &CashValue) {
        self.deposit_cash(initital_cash);
        let state = self.get_snapshot();
        self.perf.update(&state);
    }

    fn update(&mut self) -> CashValue {
        if DefaultTradingSchedule::should_trade(&self.clock.borrow().now()) {
            let orders = BrokerCalculations::diff_brkr_against_target_weights(
                &self.target_weights,
                &self.brkr,
            );
            if !orders.is_empty() {
                self.brkr.execute_orders(orders);
            }
        }
        self.brkr.check();
        let state = self.get_snapshot();
        self.perf.update(&state);
        self.brkr.get_total_value()
    }

    fn get_perf(&self) -> PerfStruct {
        self.perf.get_output()
    }
}

impl<T: DataSource> TransferTo for StaticWeightStrategy<T> {
    fn deposit_cash(&mut self, cash: &CashValue) {
        info!("STRATEGY: Depositing {:?} into strategy", cash);
        self.brkr.deposit_cash(*cash);
        self.net_cash_flow += *cash;
    }
}

impl<T: DataSource> TransferFrom for StaticWeightStrategy<T> {
    fn withdraw_cash(&mut self, cash: &CashValue) {
        if let BrokerEvent::WithdrawSuccess(withdrawn) = self.brkr.withdraw_cash(*cash) {
            info!("STRATEGY: Succesfully withdrew {:?} from strategy", cash);
            self.net_cash_flow -= withdrawn;
        }
        info!("STRATEGY: Failed to withdraw {:?} from strategy", cash);
    }
    fn withdraw_cash_with_liquidation(&mut self, cash: &CashValue) {
        if let BrokerEvent::WithdrawSuccess(withdrawn) =
            //No logging here because the implementation is fully logged due to the greater
            //complexity of this task vs standard withdraw
            BrokerCalculations::withdraw_cash_with_liquidation(cash, &mut self.brkr)
        {
            self.net_cash_flow -= withdrawn;
        }
    }
}

impl<T: DataSource> Audit for StaticWeightStrategy<T> {
    fn trades_between(&self, start: &DateTime, end: &DateTime) -> Vec<Trade> {
        self.brkr.trades_between(start, end)
    }

    fn dividends_between(&self, start: &DateTime, end: &DateTime) -> Vec<DividendPayment> {
        self.brkr.dividends_between(start, end)
    }
}

#[cfg(test)]
mod tests {

    use std::collections::HashMap;
    use std::rc::Rc;

    use super::StaticWeightStrategyBuilder;
    use crate::broker::{BrokerCost, Quote};
    use crate::clock::{Clock, ClockBuilder};
    use crate::input::{HashMapInput, HashMapInputBuilder};
    use crate::sim::broker::{SimulatedBroker, SimulatedBrokerBuilder};
    use crate::types::{DateTime, PortfolioAllocation};

    fn setup() -> (SimulatedBroker<HashMapInput>, Clock) {
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

        let brkr = SimulatedBrokerBuilder::<HashMapInput>::new()
            .with_data(source)
            .with_trade_costs(vec![BrokerCost::Flat(0.1.into())])
            .build();
        (brkr, clock)
    }

    #[test]
    #[should_panic]
    fn test_that_static_builder_fails_without_weights() {
        let comp = setup();
        let _strat = StaticWeightStrategyBuilder::<HashMapInput>::new()
            .with_brkr(comp.0)
            .with_clock(Rc::clone(&comp.1))
            .daily();
    }

    #[test]
    #[should_panic]
    fn test_that_static_builder_fails_without_brkr() {
        let comp = setup();
        let weights = PortfolioAllocation::new();
        let _strat = StaticWeightStrategyBuilder::<HashMapInput>::new()
            .with_weights(weights)
            .with_clock(Rc::clone(&comp.1))
            .daily();
    }

    #[test]
    #[should_panic]
    fn test_that_static_builder_fails_without_clock() {
        let comp = setup();
        let weights = PortfolioAllocation::new();
        let _strat = StaticWeightStrategyBuilder::<HashMapInput>::new()
            .with_weights(weights)
            .with_brkr(comp.0)
            .daily();
    }
}
