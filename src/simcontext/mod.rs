use crate::clock::Clock;
use crate::perf::{BacktestOutput, PerformanceCalculator};
use crate::strategy::{History, Strategy};
use crate::types::{CashValue, Frequency};

///Provides context for a single run of a simulation. Once a run has started, all communication
///with the components of a simulation should happen through this context.
///
///This occurs because there is no separation between components: the context must hold the
///reference to a `Strategy` to run it. Passing references around with smart pointers would
///introduce a level of complexity beyond the requirements of current use-cases. The cost of this
///is that `SimContext` is tightly-bound to `Strategy`.
pub struct SimContext<T: Strategy + History> {
    clock: Clock,
    strategy: T,
}

impl<T: Strategy + History> SimContext<T> {
    pub fn run(&mut self) {
        while self.clock.borrow().has_next() {
            self.clock.borrow_mut().tick();
            self.strategy.update();
        }
    }

    pub fn perf(&self, freq: Frequency) -> BacktestOutput {
        //Intended to be called at end of simulation
        let hist = self.strategy.get_history();
        PerformanceCalculator::calculate(freq, hist)
    }

    pub fn init(&mut self, initial_cash: &CashValue) {
        self.strategy.init(initial_cash);
    }
}

pub struct SimContextBuilder<T: Strategy + History> {
    clock: Option<Clock>,
    strategy: Option<T>,
}

impl<T: Strategy + History> SimContextBuilder<T> {
    pub fn with_strategy(&mut self, strategy: T) -> &mut Self {
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
    pub fn init(&self, initial_cash: &CashValue) -> SimContext<T> {
        if self.clock.is_none() || self.strategy.is_none() {
            panic!("SimContext must be called with clock and strategy");
        }
        let mut cxt = SimContext::<T> {
            clock: self.clock.as_ref().unwrap().clone(),
            strategy: self.strategy.as_ref().unwrap().clone(),
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

impl<T: Strategy + History> Default for SimContextBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}
