use futures::future::join_all;

use crate::clock::Clock;
use crate::exchange::DefaultExchange;
use crate::input::{DataSource, Dividendable, Quotable};
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
pub struct SimContext<Q, D, T, S>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
    S: Strategy + History,
{
    clock: Clock,
    strategy: S,
    exchange: DefaultExchange<T, Q, D>,
}

impl<Q, D, T, S> SimContext<Q, D, T, S>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
    S: Strategy + History,
{
    pub async fn run(&mut self) {
        dbg!(self.clock.has_next());
        while self.clock.has_next() {
            self.exchange.check().await;
            self.strategy.update().await;
        }
    }

    pub fn perf(&self, freq: Frequency) -> BacktestOutput {
        //Intended to be called at end of simulation
        let hist = self.strategy.get_history();
        PerformanceCalculator::calculate(freq, hist)
    }

    pub async fn init(&mut self, initial_cash: &CashValue) {
        self.strategy.init(initial_cash).await;
    }
}

pub struct SimContextMulti<Q, D, T, S>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
    S: Strategy + History,
{
    clock: Clock,
    strategies: Vec<S>,
    exchange: DefaultExchange<T, Q, D>,
}

impl<Q, D, T, S> SimContextMulti<Q, D, T, S>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
    S: Strategy + History,
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
            handles.push(strategy.init(&initial_cash));
        }
        join_all(handles).await;
    }
}

pub struct SimContextBuilder<Q, D, T, S>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
    S: Strategy + History,
{
    clock: Option<Clock>,
    strategies: Vec<S>,
    exchange: Option<DefaultExchange<T, Q, D>>,
}

impl<Q, D, T, S> SimContextBuilder<Q, D, T, S>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
    S: Strategy + History,
{
    pub fn add_strategy(&mut self, strategy: S) -> &mut Self {
        self.strategies.push(strategy);
        self
    }

    pub fn with_clock(&mut self, clock: Clock) -> &mut Self {
        self.clock = Some(clock);
        self
    }

    pub fn with_exchange(&mut self, exchange: DefaultExchange<T, Q, D>) -> &mut Self {
        self.exchange = Some(exchange);
        self
    }

    //Init stage is not idempotent as it builds a SimContext and then mutates it before handing it
    //back to the caller. This mutation ensures that the SimContext is not handed back in an
    //unintialized state that could lead to subtle errors if the client attempts to trade with, for
    //example, no cash balance.
    pub async fn init_first(&mut self, initial_cash: &CashValue) -> SimContext<Q, D, T, S> {
        if self.clock.is_none() || self.strategies.is_empty() || self.exchange.is_none() {
            panic!("SimContext must be called with clock, exchange, and strategy");
        }

        let strategy = self.strategies.remove(0);
        //Move exchange out of builder to save clone
        let exchange = std::mem::replace(&mut self.exchange, None);
        let mut cxt = SimContext::<Q, D, T, S> {
            clock: self.clock.as_ref().unwrap().clone(),
            strategy,
            exchange: exchange.unwrap(),
        };
        cxt.init(initial_cash).await;
        cxt
    }

    pub async fn init_all(&mut self, initial_cash: &CashValue) -> SimContextMulti<Q, D, T, S> {
        if self.clock.is_none() || self.strategies.is_empty() || self.exchange.is_none() {
            panic!("SimContext must be called with clock, exchange, and strategy");
        }

        //Move strategies out of Vec to save clone
        let mut strategies = Vec::new();
        while let Some(strategy) = self.strategies.pop() {
            strategies.push(strategy);
        }

        let exchange = std::mem::replace(&mut self.exchange, None);
        let mut cxt = SimContextMulti::<Q, D, T, S> {
            clock: self.clock.as_ref().unwrap().clone(),
            strategies,
            exchange: exchange.unwrap(),
        };
        cxt.init(initial_cash).await;
        cxt
    }

    pub fn new() -> Self {
        Self {
            clock: None,
            strategies: Vec::new(),
            exchange: None,
        }
    }
}
