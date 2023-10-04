use std::marker::PhantomData;

use crate::clock::Clock;
use crate::input::{PriceSource, Quotable};
use crate::exchange::implement::multi::ConcurrentExchange;

/// Builds [ConcurrentExchange].
pub struct ConcurrentExchangeBuilder<Q, P>
where
    Q: Quotable,
    P: PriceSource<Q>,
{
    price_source: Option<P>,
    clock: Option<Clock>,
    _quote: PhantomData<Q>,
}

impl<Q, P> ConcurrentExchangeBuilder<Q, P>
where
    Q: Quotable,
    P: PriceSource<Q>,
{
    pub fn build(&mut self) -> ConcurrentExchange<Q, P> {
        if self.price_source.is_none() {
            panic!("Exchange must have data source");
        }

        if self.clock.is_none() {
            panic!("Exchange must have clock");
        }

        let data = std::mem::take(&mut self.price_source).unwrap();

        ConcurrentExchange::new(self.clock.as_ref().unwrap().clone(), data.clone())
    }

    pub fn with_clock(&mut self, clock: Clock) -> &mut Self {
        self.clock = Some(clock);
        self
    }

    pub fn with_price_source(&mut self, price_source: P) -> &mut Self {
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

impl<Q, P> Default for ConcurrentExchangeBuilder<Q, P>
where
    Q: Quotable,
    P: PriceSource<Q>,
{
    fn default() -> Self {
        Self::new()
    }
}
