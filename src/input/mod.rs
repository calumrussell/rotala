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

///Retrieves price and dividends for symbol/symbols.
///
///Whilst this trait is created with backtests in mind, the calling pattern should match that used
///in live-trading systems. All system time data is stored within structs implementing this trait
///(in this case, a reference to `Clock`). Callers should not have to store time state themselves,
///this pattern reduces runtime errors.
///
///Dates will be known at runtime so when allocating space for `QuotesHashMap`/`DividendsHashMap`,
///`HashMap::with_capacity()` should be used using either length of dates or `len()` of `Clock`.
pub trait DataSource<Q, D>: Clone
where
    Q: Quotable,
    D: Dividendable,
{
    fn get_quote(&self, symbol: &str) -> Option<Arc<Q>>;
    fn get_quotes(&self) -> Option<Vec<Arc<Q>>>;
    fn get_dividends(&self) -> Option<Vec<Arc<D>>>;
}

///Implementation of [DataSource trait that wraps around a HashMap. Time is kept with reference to
///[Clock].
#[derive(Debug)]
pub struct HashMapInput {
    inner: std::sync::Arc<HashMapInputInner>,
}

#[derive(Clone, Debug)]
struct HashMapInputInner {
    quotes: QuotesHashMap,
    dividends: DividendsHashMap,
    clock: Clock,
}

pub type QuotesHashMap = HashMap<DateTime, Vec<Arc<Quote>>>;
pub type DividendsHashMap = HashMap<DateTime, Vec<Arc<Dividend>>>;

impl Clone for HashMapInput {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl DataSource<Quote, Dividend> for HashMapInput {
    fn get_quote(&self, symbol: &str) -> Option<Arc<Quote>> {
        let curr_date = self.inner.clock.now();
        if let Some(quotes) = self.inner.quotes.get(&curr_date) {
            for quote in quotes {
                if quote.symbol.eq(symbol) {
                    return Some(quote.clone());
                }
            }
        }
        None
    }

    fn get_quotes(&self) -> Option<Vec<Arc<Quote>>> {
        let curr_date = self.inner.clock.now();
        if let Some(quotes) = self.inner.quotes.get(&curr_date) {
            return Some(quotes.clone());
        }
        None
    }

    fn get_dividends(&self) -> Option<Vec<Arc<Dividend>>> {
        let curr_date = self.inner.clock.now();
        if let Some(dividends) = self.inner.dividends.get(&curr_date) {
            return Some(dividends.clone());
        }
        None
    }
}

//Can run without dividends but users of struct must initialise date and must set quotes
pub struct HashMapInputBuilder {
    quotes: Option<QuotesHashMap>,
    dividends: Option<DividendsHashMap>,
    clock: Option<Clock>,
}

impl HashMapInputBuilder {
    pub fn build(&mut self) -> HashMapInput {
        if self.clock.is_none() || self.quotes.is_none() {
            panic!("HashMapInput type must have quotes and must have date initialised");
        }

        let quotes = self.quotes.take().unwrap();
        let dividends = if self.dividends.is_none() {
            HashMap::new()
        } else {
            self.dividends.take().unwrap()
        };

        HashMapInput {
            inner: Arc::new(HashMapInputInner {
                quotes,
                dividends,
                clock: self.clock.as_ref().unwrap().clone(),
            }),
        }
    }

    pub fn with_quotes(&mut self, quotes: QuotesHashMap) -> &mut Self {
        self.quotes = Some(quotes);
        self
    }

    pub fn with_dividends(&mut self, dividends: DividendsHashMap) -> &mut Self {
        self.dividends = Some(dividends);
        self
    }

    pub fn with_clock(&mut self, clock: Clock) -> &mut Self {
        self.clock = Some(clock);
        self
    }

    pub fn new() -> Self {
        Self {
            quotes: None,
            dividends: None,
            clock: None,
        }
    }
}

impl Default for HashMapInputBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "python")]
#[derive(Clone, Debug)]
pub struct PyInput<'a> {
    pub quotes: &'a PyDict,
    pub dividends: &'a PyDict,
    pub tickers: &'a PyDict,
    pub clock: Clock,
}

#[cfg(feature = "python")]
impl<'a> DataSource<PyQuote, PyDividend> for PyInput<'a> {
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

    //TODO: need to implement, can't do this without Python-native types
    fn get_dividends(&self) -> Option<Vec<Arc<PyDividend>>> {
        None
    }
}

pub fn fake_data_generator(clock: Clock) -> HashMapInput {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    let mut raw_data: HashMap<DateTime, Vec<Arc<Quote>>> = HashMap::with_capacity(clock.len());
    for date in clock.peek() {
        let q1 = Quote::new(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "ABC",
        );
        let q2 = Quote::new(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "BCD",
        );
        raw_data.insert(i64::from(date).into(), vec![Arc::new(q1), Arc::new(q2)]);
    }

    let source = HashMapInputBuilder::new()
        .with_quotes(raw_data)
        .with_clock(clock.clone())
        .build();
    source
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

type HashMapPriceSourceInner<Q> = (HashMap<DateTime, Vec<Arc<Q>>>, Clock);

#[derive(Debug)]
pub struct HashMapPriceSource<Quote> {
    inner: Arc<HashMapPriceSourceInner<Quote>>,
}

impl PriceSource<Quote> for HashMapPriceSource<Quote> {
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

impl<Quote> Clone for HashMapPriceSource<Quote> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl HashMapPriceSource<Quote> {
    pub fn add_quotes(&mut self, date: impl Into<DateTime>, quote: Quote) {
        let inner = Arc::get_mut(&mut self.inner).unwrap();
        let datetime: DateTime = date.into();
        
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

type HashMapCorporateEventsSourceInner<D> = (HashMap<DateTime, Vec<Arc<D>>>, Clock);

#[derive(Debug)]
pub struct HashMapCorporateEventsSource<Dividend> {
    inner: std::sync::Arc<HashMapCorporateEventsSourceInner<Dividend>>,
}

impl CorporateEventsSource<Dividend> for HashMapCorporateEventsSource<Dividend> {
    fn get_dividends(&self) -> Option<Vec<Arc<Dividend>>> {
        let curr_date = self.inner.1.now();
        if let Some(dividends) = self.inner.0.get(&curr_date) {
            return Some(dividends.clone());
        }
        None
    }
}

impl<Dividend> Clone for HashMapCorporateEventsSource<Dividend> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<Dividend> HashMapCorporateEventsSource<Dividend> {
    pub fn add_dividends(&mut self, date: impl Into<DateTime>, dividend: Dividend) {
        let inner = Arc::get_mut(&mut self.inner).unwrap();
        let datetime: DateTime = date.into();
        
        if let Some(dividends) = inner.0.get_mut(&datetime) {
            dividends.push(Arc::new(dividend));
        } else {
            inner.0.insert(datetime.into(), vec![Arc::new(dividend)]);
        }
    }

    pub fn new(clock: Clock) -> Self {
        let quotes = HashMap::with_capacity(clock.len());
        Self {
            inner: Arc::new((quotes, clock)),
        }
    }
}

pub fn fake_price_source_generator(clock: Clock) -> HashMapPriceSource<Quote> {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    let mut price_source = HashMapPriceSource::new(clock.clone());
    for date in clock.peek() {
        let q1 = Quote::new(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "ABC",
        );
        let q2 = Quote::new(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "BCD",
        );
        price_source.add_quotes(date, q1);
        price_source.add_quotes(date, q2);
    }

    price_source
}