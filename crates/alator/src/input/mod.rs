//! Data sources

use std::collections::HashMap;
use std::sync::Arc;
use alator_clock::{DateTime, Clock};
use alator_exchange::input::DefaultPriceSource;
use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;

use crate::broker::Dividend;
use crate::types::Price;

/// Inner type for dividends for [CorporateEventsSource].
pub trait Dividendable: Clone + std::marker::Send + std::marker::Sync {
    fn get_symbol(&self) -> &String;
    fn get_date(&self) -> &DateTime;
    fn get_value(&self) -> &Price;
}

/// Represents structure that generates dividend information.
///
/// There can be multiple types of corporate events but we currently only support dividends.
pub trait CorporateEventsSource<D>: Clone
where
    D: Dividendable,
{
    fn get_dividends(&self) -> Option<Vec<Arc<D>>>;
}


/// Generates random [DefaultPriceSource] for use in tests that don't depend on prices.
pub fn fake_price_source_generator(clock: Clock) -> DefaultPriceSource {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    let mut price_source = DefaultPriceSource::new();
    for date in clock.peek() {
        price_source.add_quotes(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            *date,
            "ABC",
        );
        price_source.add_quotes(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            *date,
            "BCD",
        );
    }
    price_source
}

type CorporateEventsSourceImpl<D> = (HashMap<DateTime, Vec<Arc<D>>>, Clock);

/// Default implementation of [CorporateEventsSource] with [Dividend] as inner type.
#[derive(Debug)]
pub struct DefaultCorporateEventsSource {
    inner: std::sync::Arc<CorporateEventsSourceImpl<Dividend>>,
}

impl CorporateEventsSource<Dividend> for DefaultCorporateEventsSource {
    fn get_dividends(&self) -> Option<Vec<Arc<Dividend>>> {
        let curr_date = self.inner.1.now();
        if let Some(dividends) = self.inner.0.get(&curr_date) {
            return Some(dividends.clone());
        }
        None
    }
}

impl Clone for DefaultCorporateEventsSource {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl DefaultCorporateEventsSource {
    pub fn add_dividends(
        &mut self,
        value: impl Into<Price>,
        symbol: impl Into<String>,
        date: impl Into<DateTime>,
    ) {
        let inner = Arc::get_mut(&mut self.inner).unwrap();
        let datetime: DateTime = date.into();
        let dividend = Dividend::new(value, symbol, datetime);

        if let Some(dividends) = inner.0.get_mut(&datetime) {
            dividends.push(Arc::new(dividend));
        } else {
            inner.0.insert(datetime, vec![Arc::new(dividend)]);
        }
    }

    pub fn new(clock: Clock) -> Self {
        let quotes = HashMap::with_capacity(clock.len());
        Self {
            inner: Arc::new((quotes, clock)),
        }
    }
}