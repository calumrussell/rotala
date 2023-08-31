use crate::clock::Clock;
use crate::exchange::ConcurrentExchange;
use crate::input::{DataSource, Dividendable, Quotable};
use crate::strategy::{AsyncStrategy, History, Strategy};
use crate::types::CashValue;

use super::{SimContext, SimContextMulti};

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

pub struct SimContextMultiBuilder<Q, D, T, S>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
    S: AsyncStrategy + History,
{
    clock: Option<Clock>,
    strategies: Vec<S>,
    exchange: Option<ConcurrentExchange<T, Q, D>>,
}

impl<Q, D, T, S> Default for SimContextMultiBuilder<Q, D, T, S>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
    S: AsyncStrategy + History,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Q, D, T, S> SimContextMultiBuilder<Q, D, T, S>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
    S: AsyncStrategy + History,
{
    pub fn add_strategy(&mut self, strategy: S) -> &mut Self {
        self.strategies.push(strategy);
        self
    }

    pub fn with_clock(&mut self, clock: Clock) -> &mut Self {
        self.clock = Some(clock);
        self
    }

    pub fn with_exchange(&mut self, exchange: ConcurrentExchange<T, Q, D>) -> &mut Self {
        self.exchange = Some(exchange);
        self
    }

    //Init stage is not idempotent as it builds a SimContext and then mutates it before handing it
    //back to the caller. This mutation ensures that the SimContext is not handed back in an
    //unintialized state that could lead to subtle errors if the client attempts to trade with, for
    //example, no cash balance.
    pub async fn init(&mut self, initial_cash: &CashValue) -> SimContextMulti<Q, D, T, S> {
        if self.clock.is_none() || self.strategies.is_empty() || self.exchange.is_none() {
            panic!("SimContext must be called with clock, exchange, and strategy");
        }

        //Move strategies out of Vec to save clone
        let mut strategies = Vec::new();
        while let Some(strategy) = self.strategies.pop() {
            strategies.push(strategy);
        }

        let exchange = self.exchange.take();
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
