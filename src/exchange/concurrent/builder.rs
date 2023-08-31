use std::marker::PhantomData;

use crate::clock::Clock;
use crate::input::{DataSource, Dividendable, Quotable};

use super::ConcurrentExchange;

pub struct ConcurrentExchangeBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    data_source: Option<T>,
    clock: Option<Clock>,
    _quote: PhantomData<Q>,
    _dividend: PhantomData<D>,
}

impl<T, Q, D> ConcurrentExchangeBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    pub fn build(&mut self) -> ConcurrentExchange<T, Q, D> {
        if self.data_source.is_none() {
            panic!("Exchange must have data source");
        }

        if self.clock.is_none() {
            panic!("Exchange must have clock");
        }

        let data = std::mem::take(&mut self.data_source).unwrap();

        ConcurrentExchange::new(
            self.clock.as_ref().unwrap().clone(),
            data.clone(),
        )
    }

    pub fn with_clock(&mut self, clock: Clock) -> &mut Self {
        self.clock = Some(clock);
        self
    }

    pub fn with_data_source(&mut self, data_source: T) -> &mut Self {
        self.data_source = Some(data_source);
        self
    }

    pub fn new() -> Self {
        Self {
            clock: None,
            data_source: None,
            _quote: PhantomData,
            _dividend: PhantomData,
        }
    }
}

impl<T, Q, D> Default for ConcurrentExchangeBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn default() -> Self {
        Self::new()
    }
}
