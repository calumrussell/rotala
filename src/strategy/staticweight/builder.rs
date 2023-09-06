use crate::broker::{ConcurrentBroker, SingleBroker};
use crate::clock::Clock;
use crate::input::{Dividendable, Quotable, CorporateEventsSource, PriceSource};
use crate::types::PortfolioAllocation;

use super::{AsyncStaticWeightStrategy, StaticWeightStrategy};

pub struct StaticWeightStrategyBuilder<D, T, Q, P>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
    P: PriceSource<Q>,
{
    //If missing either field, we cannot run this strategy
    brkr: Option<SingleBroker<D, T, Q, P>>,
    weights: Option<PortfolioAllocation>,
    clock: Option<Clock>,
}

impl<D, T, Q, P> StaticWeightStrategyBuilder<D, T, Q, P>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
    P: PriceSource<Q>,
{
    pub fn default(&mut self) -> StaticWeightStrategy<D, T, Q, P> {
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

    pub fn with_brkr(&mut self, brkr: SingleBroker<D, T, Q, P>) -> &mut Self {
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

impl<D, T, Q, P> Default for StaticWeightStrategyBuilder<D, T, Q, P>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
    P: PriceSource<Q>,
{
    fn default() -> Self {
        Self::new()
    }
}

pub struct AsyncStaticWeightStrategyBuilder<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    //If missing either field, we cannot run this strategy
    brkr: Option<ConcurrentBroker<D, T, Q>>,
    weights: Option<PortfolioAllocation>,
    clock: Option<Clock>,
}

impl<D, T, Q> AsyncStaticWeightStrategyBuilder<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    pub fn default(&mut self) -> AsyncStaticWeightStrategy<D, T, Q> {
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

    pub fn with_brkr(&mut self, brkr: ConcurrentBroker<D, T, Q>) -> &mut Self {
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

impl<T, Q, D> Default for AsyncStaticWeightStrategyBuilder<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    fn default() -> Self {
        Self::new()
    }
}
