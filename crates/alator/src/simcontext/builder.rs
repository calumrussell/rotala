use crate::strategy::{History, Strategy};
use crate::types::CashValue;
use rotala::clock::Clock;

use super::SimContext;

/// Creates a single-threaded [SimContext]
pub struct SimContextBuilder<S>
where
    S: Strategy + History,
{
    clock: Option<Clock>,
    strategy: Option<S>,
}

impl<S> Default for SimContextBuilder<S>
where
    S: Strategy + History,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S> SimContextBuilder<S>
where
    S: Strategy + History,
{
    pub fn with_strategy(&mut self, strategy: S) -> &mut Self {
        self.strategy = Some(strategy);
        self
    }

    pub fn with_clock(&mut self, clock: Clock) -> &mut Self {
        self.clock = Some(clock);
        self
    }

    //Init stage is not idempotent as it builds a SimContext and then mutates it before handing it
    //back to the caller. This mutation ensures that the SimContext is not handed back in an
    //unintialized state that could lead to subtle errors if the client attempts to trade with, for
    //example, no cash balance.
    pub fn init(&mut self, initial_cash: &CashValue) -> SimContext<S> {
        if self.clock.is_none() || self.strategy.is_none() {
            panic!("SimContext must be called with clock, exchange, and strategy");
        }

        let strategy = self.strategy.take().unwrap();

        let mut cxt = SimContext::<S> {
            clock: self.clock.as_ref().unwrap().clone(),
            strategy,
        };
        cxt.init(initial_cash);
        cxt
    }

    pub fn new() -> Self {
        Self {
            clock: None,
            strategy: None,
        }
    }
}
