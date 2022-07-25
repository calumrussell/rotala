use crate::types::CashValue;
use crate::perf::PerfStruct;
use crate::strategy::Strategy;
use crate::clock::Clock;

pub struct SimContext<T: Strategy> {
    clock: Clock,
    strategy: T,
}

impl<T: Strategy> SimContext<T> {
    pub fn run(&mut self) {
        while self.clock.borrow().has_next() {
            self.clock.borrow_mut().tick();
            self.strategy.update();
        }
    }

    pub fn init(&mut self, initial_cash: &CashValue) {
        self.strategy.init(initial_cash);
    }

    pub fn calculate_perf(&mut self) -> PerfStruct {
        self.strategy.get_perf()
    }
}

pub struct SimContextBuilder<T: Strategy> {
    clock: Option<Clock>,
    strategy: Option<T>,
}

impl<T: Strategy> SimContextBuilder<T> {

    pub fn with_strategy(&mut self, strategy: T) -> &mut Self {
        self.strategy = Some(strategy);
        self
    }

    pub fn with_clock(&mut self, clock: Clock) -> &mut Self {
        self.clock = Some(clock);
        self
    }

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

