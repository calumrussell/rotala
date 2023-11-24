use crate::broker::single::SingleBroker;
use crate::types::PortfolioAllocation;
use alator_clock::Clock;

use super::StaticWeightStrategy;

pub struct StaticWeightStrategyBuilder {
    //If missing either field, we cannot run this strategy
    brkr: Option<SingleBroker>,
    weights: Option<PortfolioAllocation>,
    clock: Option<Clock>,
}

impl StaticWeightStrategyBuilder {
    pub fn default(&mut self) -> StaticWeightStrategy {
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

    pub fn with_brkr(&mut self, brkr: SingleBroker) -> &mut Self {
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

impl Default for StaticWeightStrategyBuilder {
    fn default() -> Self {
        Self::new()
    }
}
