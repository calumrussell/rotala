use std::marker::PhantomData;

use crate::clock::Clock;
use crate::input::{Quotable, PriceSource};

use super::ConcurrentExchange;

pub struct ConcurrentExchangeBuilder<T, Q>
where
    Q: Quotable,
    T: PriceSource<Q>,
{
    data_source: Option<T>,
    clock: Option<Clock>,
    _quote: PhantomData<Q>,
}

impl<T, Q> ConcurrentExchangeBuilder<T, Q>
where
    Q: Quotable,
    T: PriceSource<Q>,
{
    pub fn build(&mut self) -> ConcurrentExchange<T, Q> {
        if self.data_source.is_none() {
            panic!("Exchange must have data source");
        }

        if self.clock.is_none() {
            panic!("Exchange must have clock");
        }

        let data = std::mem::take(&mut self.data_source).unwrap();

        ConcurrentExchange::new(self.clock.as_ref().unwrap().clone(), data.clone())
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
        }
    }
}

impl<T, Q> Default for ConcurrentExchangeBuilder<T, Q>
where
    Q: Quotable,
    T: PriceSource<Q>,
{
    fn default() -> Self {
        Self::new()
    }
}
