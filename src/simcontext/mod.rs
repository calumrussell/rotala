use futures::future::join_all;
use std::sync::{Arc, Mutex};

use crate::clock::Clock;
use crate::perf::{BacktestOutput, PerformanceCalculator};
use crate::strategy::{History, Strategy};
use crate::types::{CashValue, Frequency};

pub struct SimContext<T: Strategy + History> {
    clock: Clock,
    strategy: T,
}

impl<T: Strategy + History> SimContext<T> {
    pub fn run(&mut self) {
        while self.clock.has_next() {
            self.clock.tick();
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

///Provides context for a single run of a simulation. Once a run has started, all communication
///with the components of a simulation should happen through this context.
///
///This occurs because there is no separation between components: the context must hold the
///reference to a `Strategy` to run it. Passing references around with smart pointers would
///introduce a level of complexity beyond the requirements of current use-cases. The cost of this
///is that `SimContext` is tightly-bound to `Strategy`.
pub struct SimContextMulti<T: Strategy + History + std::marker::Send> {
    clock: Clock,
    strategies: Vec<Arc<Mutex<T>>>,
}

impl<T: Strategy + History + std::marker::Send + 'static> SimContextMulti<T> {
    pub async fn run(&mut self) {
        while self.clock.has_next() {
            self.clock.tick();
            //self.strategy.lock().unwrap().update();

            let mut handles = Vec::new();
            for strategy in &self.strategies {
                let strat = Arc::clone(strategy);
                let handle = tokio::spawn(async move {
                    strat.lock().unwrap().update();
                });
                handles.push(handle);
            }
            join_all(handles).await;
        }
    }

    pub fn perf(&self, freq: Frequency) -> Vec<BacktestOutput> {
        let mut res = Vec::new();
        //Intended to be called at end of simulation
        for strategy in &self.strategies {
            let hist = strategy.lock().unwrap().get_history();
            let perf = PerformanceCalculator::calculate(freq.clone(), hist);
            res.push(perf);
        }
        res
    }

    pub fn init(&mut self, initial_cash: &CashValue) {
        for strategy in &self.strategies {
            strategy.lock().unwrap().init(&initial_cash);
        }
    }
}

pub struct SimContextBuilder<T: Strategy + History + std::marker::Send> {
    clock: Option<Clock>,
    strategies: Vec<T>,
}

impl<T: Strategy + History + std::marker::Send + 'static> SimContextBuilder<T> {
    pub fn add_strategy(&mut self, strategy: T) -> &mut Self {
        self.strategies.push(strategy);
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
    pub fn init_first(&mut self, initial_cash: &CashValue) -> SimContext<T> {
        if self.clock.is_none() || self.strategies.is_empty() {
            panic!("SimContext must be called with clock and strategy");
        }

        let strategy = self.strategies.remove(0);
        let mut cxt = SimContext::<T> {
            clock: self.clock.as_ref().unwrap().clone(),
            strategy,
        };
        cxt.init(initial_cash);
        cxt
    }

    pub fn init_all(&mut self, initial_cash: &CashValue) -> SimContextMulti<T> {
        if self.clock.is_none() || self.strategies.is_empty() {
            panic!("SimContext must be called with clock and strategy");
        }

        let mut into_arc_mutex = Vec::new();
        for i in 0..self.strategies.len() {
            into_arc_mutex.push(Arc::new(Mutex::new(self.strategies.remove(i))));
        }

        let mut cxt = SimContextMulti::<T> {
            clock: self.clock.as_ref().unwrap().clone(),
            strategies: into_arc_mutex,
        };
        cxt.init(initial_cash);
        cxt
    }

    pub fn new() -> Self {
        Self {
            clock: None,
            strategies: Vec::new(),
        }
    }
}

impl<T: Strategy + History + std::marker::Send + 'static> Default for SimContextBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}
