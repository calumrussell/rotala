mod builder;

pub use builder::{SimContextBuilder, SimContextMultiBuilder};

use futures::future::join_all;

use crate::clock::Clock;
use crate::exchange::ConcurrentExchange;
use crate::input::{DataSource, Dividendable, Quotable};
use crate::perf::{BacktestOutput, PerformanceCalculator};
use crate::strategy::{AsyncStrategy, History, Strategy};
use crate::types::{CashValue, Frequency};

///Provides context for a single run of a simulation. Once a run has started, all communication
///with the components of a simulation should happen through this context.
///
///This occurs because there is no separation between components: the context must hold the
///reference to a `Strategy` to run it. Passing references around with smart pointers would
///introduce a level of complexity beyond the requirements of current use-cases. The cost of this
///is that `SimContext` is tightly-bound to `Strategy`.
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

pub struct SimContextMulti<Q, D, T, S>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
    S: AsyncStrategy + History,
{
    clock: Clock,
    strategies: Vec<S>,
    exchange: ConcurrentExchange<T, Q, D>,
}

impl<Q, D, T, S> SimContextMulti<Q, D, T, S>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
    S: AsyncStrategy + History,
{
    pub async fn run(&mut self) {
        while self.clock.has_next() {
            self.exchange.check().await;

            let mut handles = Vec::new();
            for strategy in &mut self.strategies {
                handles.push(strategy.update());
            }
            join_all(handles).await;
        }
    }

    pub fn perf(&self, freq: Frequency) -> Vec<BacktestOutput> {
        let mut res = Vec::new();
        //Intended to be called at end of simulation
        for strategy in &self.strategies {
            let hist = strategy.get_history();
            let perf = PerformanceCalculator::calculate(freq.clone(), hist);
            res.push(perf);
        }
        res
    }

    pub async fn init(&mut self, initial_cash: &CashValue) {
        let mut handles = Vec::new();
        for strategy in &mut self.strategies {
            handles.push(strategy.init(initial_cash));
        }
        join_all(handles).await;
    }
}
