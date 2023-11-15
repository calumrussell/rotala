//! Running context for backtest

mod builder;

pub use builder::SimContextBuilder;

use crate::perf::{BacktestOutput, PerformanceCalculator};
use crate::strategy::{AsyncStrategy, History, Strategy};
use crate::types::CashValue;
use alator_clock::{Clock, Frequency};

/// Context for single-threaded simulation run.
///
/// Within the single-threaded context, the call stack it totally vertical: strategy passes signal
/// to broker, broker passes signal to exchange, and then the exchange gets updated and there is a
/// quick update of the broker before we get passed back to the top-level context. This call
/// pattern is very simple and performant but does mean that operations aren't transparent from this
/// level.
pub struct SimContext<S>
where
    S: Strategy + History,
{
    clock: Clock,
    strategy: S,
}

impl<S> SimContext<S>
where
    S: Strategy + History,
{
    pub fn run(&mut self) {
        while self.clock.has_next() {
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
