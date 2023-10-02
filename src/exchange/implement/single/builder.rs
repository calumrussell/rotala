use std::marker::PhantomData;

use crate::clock::Clock;
use crate::input::{PriceSource, Quotable};
use crate::exchange::implement::single::SingleExchange;

/// Used to build [SingleExchange].
pub struct SingleExchangeBuilder<Q, T>
where
    Q: Quotable,
    T: PriceSource<Q>,
{
    price_source: Option<T>,
    clock: Option<Clock>,
    _quote: PhantomData<Q>,
}

impl<Q, T> SingleExchangeBuilder<Q, T>
where
    Q: Quotable,
    T: PriceSource<Q>,
{
    pub fn build(&mut self) -> SingleExchange<Q, T> {
        if self.price_source.is_none() {
            panic!("Exchange must have data source");
        }

        if self.clock.is_none() {
            panic!("Exchange must have clock");
        }

        let data = std::mem::take(&mut self.price_source).unwrap();

        SingleExchange::new(self.clock.as_ref().unwrap().clone(), data)
    }

    pub fn with_clock(&mut self, clock: Clock) -> &mut Self {
        self.clock = Some(clock);
        self
    }

    pub fn with_price_source(&mut self, price_source: T) -> &mut Self {
        self.price_source = Some(price_source);
        self
    }

    pub fn new() -> Self {
        Self {
            clock: None,
            price_source: None,
            _quote: PhantomData,
        }
    }
}

impl<Q, T> Default for SingleExchangeBuilder<Q, T>
where
    Q: Quotable,
    T: PriceSource<Q>,
{
    fn default() -> Self {
        Self::new()
    }
}
