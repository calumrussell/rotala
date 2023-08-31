use crate::broker::{ConcurrentBroker, SingleBroker};
use crate::clock::Clock;
use crate::input::{DataSource, Dividendable, Quotable};
use crate::types::PortfolioAllocation;

use super::{AsyncStaticWeightStrategy, StaticWeightStrategy};

pub struct StaticWeightStrategyBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    //If missing either field, we cannot run this strategy
    brkr: Option<SingleBroker<T, Q, D>>,
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

        let brkr = self.brkr.take();
        let weights = self.weights.take();
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

    pub fn with_brkr(&mut self, brkr: SingleBroker<T, Q, D>) -> &mut Self {
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

pub struct AsyncStaticWeightStrategyBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    //If missing either field, we cannot run this strategy
    brkr: Option<ConcurrentBroker<T, Q, D>>,
    weights: Option<PortfolioAllocation>,
    clock: Option<Clock>,
}

impl<T, Q, D> AsyncStaticWeightStrategyBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    pub fn default(&mut self) -> AsyncStaticWeightStrategy<T, Q, D> {
        if self.brkr.is_none() || self.weights.is_none() || self.clock.is_none() {
            panic!("Strategy must have broker, weights, and clock");
        }

        let brkr = self.brkr.take();
        let weights = self.weights.take();
        AsyncStaticWeightStrategy {
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

    pub fn with_brkr(&mut self, brkr: ConcurrentBroker<T, Q, D>) -> &mut Self {
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

impl<T, Q, D> Default for AsyncStaticWeightStrategyBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn default() -> Self {
        Self::new()
    }
}
