//! Running context for backtest

mod builder;

use std::marker::PhantomData;

pub use builder::{SimContextBuilder, SimContextMultiBuilder};

use futures::future::join_all;

use crate::clock::Clock;
use crate::exchange::implement::multi::ConcurrentExchange;
use crate::input::{Dividendable, PriceSource, Quotable};
use crate::perf::{BacktestOutput, PerformanceCalculator};
use crate::strategy::{AsyncStrategy, History, Strategy};
use crate::types::{CashValue, Frequency};

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

/// Context for multi-threaded simulation run
/// 
/// Unlike the single-threaded run, context has to play some role in co-ordinating operations
/// between components. Because broker cannot pass trades through to the exchange directly,
/// as it only holds a reference to a channel to which it sends orders, we have to first tick
/// the exchange (which ticks, passes updated prices and notifications and executes any trades)
/// and then strategy updates, potentially telling broker to send new orders.
/// 
/// Context, therefore, contains more logic orchestrating between components.
pub struct SimContextMulti<D, Q, P, S>
where
    D: Dividendable,
    Q: Quotable,
    P: PriceSource<Q>,
    S: AsyncStrategy + History,
{
    clock: Clock,
    exchange: ConcurrentExchange<Q, P>,
    strategies: Vec<S>,
    dividend: PhantomData<D>,
}

impl<D, Q, P, S> SimContextMulti<D, Q, P, S>
where
    D: Dividendable,
    Q: Quotable,
    P: PriceSource<Q>,
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
