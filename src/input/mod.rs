use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use std::collections::HashMap;
use std::rc::Rc;

use crate::broker::{Dividend, Quote};
use crate::clock::Clock;
use crate::types::DateTime;

///Retrieves price and dividends for symbol/symbols.
///
///Whilst this trait is created with backtests in mind, the calling pattern should match that used
///in live-trading systems. All system time data is stored within structs implementing this trait
///(in this case, a reference to `Clock`). Callers should not have to store time state themselves,
///this pattern reduces runtime errors.
pub trait DataSource: Clone {
    fn get_quote(&self, symbol: &str) -> Option<Quote>;
    fn get_quotes(&self) -> Option<Vec<Quote>>;
    fn get_dividends(&self) -> Option<Vec<Dividend>>;
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

impl DataSource for HashMapInput {
    fn get_quote(&self, symbol: &str) -> Option<Quote> {
        let curr_date = self.clock.borrow().now();
        if let Some(quotes) = self.quotes.get(&curr_date) {
            for quote in quotes {
                if quote.symbol.eq(symbol) {
                    return Some(quote.clone());
                }
            }
        }
        None
    }

    fn get_quotes(&self) -> Option<Vec<Quote>> {
        let curr_date = self.clock.borrow().now();
        self.quotes.get(&curr_date).cloned()
    }

    fn get_dividends(&self) -> Option<Vec<Dividend>> {
        let curr_date = self.clock.borrow().now();
        self.dividends.get(&curr_date).cloned()
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

pub fn fake_data_generator(clock: Clock) -> HashMapInput {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    let mut raw_data: HashMap<DateTime, Vec<Quote>> = HashMap::new();
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
