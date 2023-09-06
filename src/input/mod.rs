use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use std::collections::HashMap;
use std::sync::Arc;

use crate::broker::{Dividend, Quote};
use crate::clock::Clock;
use crate::types::{DateTime, Price};

#[cfg(feature = "python")]
use crate::broker::{PyDividend, PyQuote};
#[cfg(feature = "python")]
use pyo3::pycell::PyCell;
#[cfg(feature = "python")]
use pyo3::types::{PyDict, PyList};

pub trait Quotable: Clone + std::marker::Send + std::marker::Sync {
    fn get_bid(&self) -> &Price;
    fn get_ask(&self) -> &Price;
    fn get_date(&self) -> &DateTime;
    fn get_symbol(&self) -> &String;
}

pub trait Dividendable: Clone + std::marker::Send + std::marker::Sync {
    fn get_symbol(&self) -> &String;
    fn get_date(&self) -> &DateTime;
    fn get_value(&self) -> &Price;
}

pub trait PriceSource<Q>: Clone
where
    Q: Quotable,
{
    fn get_quote(&self, symbol: &str) -> Option<Arc<Q>>;
    fn get_quotes(&self) -> Option<Vec<Arc<Q>>>;
}

pub trait CorporateEventsSource<D>: Clone
where
    D: Dividendable,
{
    fn get_dividends(&self) -> Option<Vec<Arc<D>>>;
}

type HashMapInner<Q> = (HashMap<DateTime, Vec<Arc<Q>>>, Clock);

#[derive(Debug)]
pub struct DefaultPriceSource {
    inner: Arc<HashMapInner<Quote>>,
}

impl PriceSource<Quote> for DefaultPriceSource {
    fn get_quote(&self, symbol: &str) -> Option<Arc<Quote>> {
        let curr_date = self.inner.1.now();
        if let Some(quotes) = self.inner.0.get(&curr_date) {
            for quote in quotes {
                if quote.get_symbol().eq(symbol) {
                    return Some(quote.clone());
                }
            }
        }
        None
    }

    fn get_quotes(&self) -> Option<Vec<Arc<Quote>>> {
        let curr_date = self.inner.1.now();
        if let Some(quotes) = self.inner.0.get(&curr_date) {
            return Some(quotes.clone());
        }
        None
    }
}

impl Clone for DefaultPriceSource {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl DefaultPriceSource {
    pub fn add_quotes(
        &mut self,
        bid: impl Into<Price>,
        ask: impl Into<Price>,
        date: impl Into<DateTime>,
        symbol: impl Into<String>,
    ) {
        let inner = Arc::get_mut(&mut self.inner).unwrap();
        let datetime: DateTime = date.into();

        let quote = Quote::new(bid, ask, datetime, symbol);
        if let Some(quotes) = inner.0.get_mut(&datetime) {
            quotes.push(Arc::new(quote))
        } else {
            inner.0.insert(datetime, vec![Arc::new(quote)]);
        }
    }

    pub fn from_hashmap(quotes: HashMap<DateTime, Vec<Arc<Quote>>>, clock: Clock) -> Self {
        Self {
            inner: Arc::new((quotes, clock)),
        }
    }

    pub fn new(clock: Clock) -> Self {
        let quotes = HashMap::with_capacity(clock.len());
        Self {
            inner: Arc::new((quotes, clock)),
        }
    }
}

#[cfg(feature = "python")]
#[derive(Clone, Debug)]
pub struct PyPriceSource<'a> {
    pub quotes: &'a PyDict,
    pub tickers: &'a PyDict,
    pub clock: Clock,
}

#[cfg(feature = "python")]
impl<'a> PriceSource<PyQuote> for PyPriceSource<'a> {
    fn get_quote(&self, symbol: &str) -> Option<Arc<PyQuote>> {
        if let Some(ticker_pos_any) = self.tickers.get_item(symbol) {
            let curr_date = self.clock.now();
            if let Some(quotes) = self.quotes.get_item(i64::from(curr_date)) {
                if let Ok(quotes_list) = quotes.downcast::<PyList>() {
                    if let Ok(ticker_pos) = ticker_pos_any.extract::<usize>() {
                        let quote_any = &quotes_list[ticker_pos];
                        if let Ok(quote) = quote_any.downcast::<PyCell<PyQuote>>() {
                            let to_inner = quote.get();
                            return Some(Arc::new(to_inner.clone()));
                        }
                    }
                }
            }
        }
        None
    }

    //TODO: need to implement, can't do this without Python-native types
    fn get_quotes(&self) -> Option<Vec<Arc<PyQuote>>> {
        None
    }
}

#[cfg(feature = "python")]
#[derive(Clone, Debug)]
pub struct PyCorporateEventsSource<'a> {
    pub dividends: &'a PyDict,
    pub clock: Clock,
}

#[cfg(feature = "python")]
impl<'a> CorporateEventsSource<PyDividend> for PyCorporateEventsSource<'a> {
    fn get_dividends(&self) -> Option<Vec<Arc<PyDividend>>> {
        None
    }
}

type HashMapCorporateEventsSourceInner<D> = (HashMap<DateTime, Vec<Arc<D>>>, Clock);

#[derive(Debug)]
pub struct HashMapCorporateEventsSource {
    inner: std::sync::Arc<HashMapCorporateEventsSourceInner<Dividend>>,
}

impl CorporateEventsSource<Dividend> for HashMapCorporateEventsSource {
    fn get_dividends(&self) -> Option<Vec<Arc<Dividend>>> {
        let curr_date = self.inner.1.now();
        if let Some(dividends) = self.inner.0.get(&curr_date) {
            return Some(dividends.clone());
        }
        None
    }
}

impl Clone for HashMapCorporateEventsSource {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl HashMapCorporateEventsSource {
    pub fn add_dividends(&mut self, date: impl Into<DateTime>, dividend: Dividend) {
        let inner = Arc::get_mut(&mut self.inner).unwrap();
        let datetime: DateTime = date.into();

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

pub fn fake_price_source_generator(clock: Clock) -> DefaultPriceSource {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    let mut price_source = DefaultPriceSource::new(clock.clone());
    for date in clock.peek() {
        price_source.add_quotes(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "ABC",
        );
        price_source.add_quotes(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "BCD",
        );
    }
    price_source
}
