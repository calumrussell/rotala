use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use std::collections::HashMap;
use std::rc::Rc;

use crate::broker::{Dividend, Quote};
use crate::clock::Clock;
use crate::types::{DateTime, Price};

#[cfg(feature = "python")]
use crate::broker::{PyDividend, PyQuote};
#[cfg(feature = "python")]
use pyo3::types::{PyDict, PyList};

pub trait Quotable {
    fn get_bid(&self) -> &Price;
    fn get_ask(&self) -> &Price;
    fn get_date(&self) -> &DateTime;
    fn get_symbol(&self) -> &String;
}

pub trait Dividendable {
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
pub trait DataSource<Q: Quotable, D: Dividendable>: Clone {
    fn get_quote(&self, symbol: &str) -> Option<&Q>;
    fn get_quotes(&self) -> Option<&Vec<Q>>;
    fn get_dividends(&self) -> Option<&Vec<D>>;
}

///Implementation of [DataSource trait that wraps around a HashMap. Time is kept with reference to
///[Clock].
#[derive(Clone, Debug)]
pub struct HashMapInput {
    quotes: QuotesHashMap,
    dividends: DividendsHashMap,
    clock: Clock,
}

pub type QuotesHashMap = HashMap<DateTime, Vec<Quote>>;
pub type DividendsHashMap = HashMap<DateTime, Vec<Dividend>>;

impl DataSource<Quote, Dividend> for HashMapInput {
    fn get_quote(&self, symbol: &str) -> Option<&Quote> {
        let curr_date = self.clock.borrow().now();
        if let Some(quotes) = self.quotes.get(&curr_date) {
            for quote in quotes {
                if quote.symbol.eq(symbol) {
                    return Some(quote);
                }
            }
        }
        None
    }

    fn get_quotes(&self) -> Option<&Vec<Quote>> {
        let curr_date = self.clock.borrow().now();
        self.quotes.get(&curr_date)
    }

    fn get_dividends(&self) -> Option<&Vec<Dividend>> {
        let curr_date = self.clock.borrow().now();
        self.dividends.get(&curr_date)
    }
}

//Can run without dividends but users of struct must initialise date and must set quotes
pub struct HashMapInputBuilder {
    quotes: Option<QuotesHashMap>,
    dividends: DividendsHashMap,
    clock: Option<Clock>,
}

impl HashMapInputBuilder {
    pub fn build(&self) -> HashMapInput {
        if self.clock.is_none() || self.quotes.is_none() {
            panic!("HashMapInput type must have quotes and must have date initialised");
        }

        HashMapInput {
            quotes: self.quotes.as_ref().unwrap().clone(),
            dividends: self.dividends.clone(),
            clock: self.clock.as_ref().unwrap().clone(),
        }
    }

    pub fn with_quotes(&mut self, quotes: QuotesHashMap) -> &mut Self {
        self.quotes = Some(quotes);
        self
    }

    pub fn with_dividends(&mut self, dividends: DividendsHashMap) -> &mut Self {
        self.dividends = dividends;
        self
    }

    pub fn with_clock(&mut self, clock: Clock) -> &mut Self {
        self.clock = Some(clock);
        self
    }

    pub fn new() -> Self {
        Self {
            quotes: None,
            dividends: HashMap::new(),
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
    quotes: &'a PyDict,
    dividends: &'a PyDict,
    tickers: &'a PyDict,
    clock: Clock,
}

#[cfg(feature = "python")]
impl<'a> PyInput<'a> {
    fn get_ticker_by_pos(&self, pos: i32) -> String {
        let count = 0;
        for key in self.tickers.keys() {
            if count == pos {
                if let Ok(ticker) = key.extract::<String>() {
                    return ticker;
                }
            }
        }
        panic!("Can't find position, there is an error in the code");
    }
}

#[cfg(feature = "python")]
impl<'a> DataSource<PyQuote, PyDividend> for PyInput<'a> {
    fn get_quote(&self, symbol: &str) -> Option<&PyQuote> {
        if let Some(ticker_pos_any) = self.tickers.get_item(symbol) {
            let curr_date = self.clock.borrow().now();
            if let Some(quotes) = self.quotes.get_item(i64::from(curr_date)) {
                if let Ok(quotes_list) = quotes.downcast::<PyList>() {
                    if let Ok(ticker_pos) = ticker_pos_any.extract::<usize>() {
                        let quote_any = quotes_list[ticker_pos];
                        if let Ok(quote) = quote_any.downcast::<PyQuote>() {
                            return Some(quote);
                        }
                    }
                }
            }
        }
        None
    }

    //TODO: need to implement, can't do this without Python-native types
    fn get_quotes(&self) -> Option<&Vec<PyQuote>> {
        let fake = Vec::new();
        return Some(&fake);
    }

    //TODO: need to implement, can't do this without Python-native types
    fn get_dividends(&self) -> Option<&Vec<PyDividend>> {
        let fake = Vec::new();
        return Some(&fake);
    }
}

pub fn fake_data_generator(clock: Clock) -> HashMapInput {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    let mut raw_data: HashMap<DateTime, Vec<Quote>> = HashMap::with_capacity(clock.borrow().len());
    for date in clock.borrow().peek() {
        let q1 = Quote::new(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date.clone(),
            "ABC",
        );
        let q2 = Quote::new(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date.clone(),
            "BCD",
        );
        raw_data.insert(i64::from(date).into(), vec![q1, q2]);
    }

    let source = HashMapInputBuilder::new()
        .with_quotes(raw_data)
        .with_clock(Rc::clone(&clock))
        .build();
    source
}
