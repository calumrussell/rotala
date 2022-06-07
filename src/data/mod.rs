use itertools::Itertools;
use std::collections::HashMap;

use crate::broker::{Dividend, Quote};

/* Abstracts basic data operations for components that use data.
 */
pub trait SimSource {
    fn get_quotes_dates(&self) -> Vec<i64>;
    //Added dividends, could make sense to support a range of *Actions* such as Quote or Dividend
    //but doesn't make sense when there is only too (and it wouldn't change the public interface).
    fn get_dividends_by_date(&self, date: &i64) -> Option<Vec<Dividend>>;
    fn get_quotes_by_date(&self, date: &i64) -> Option<Vec<Quote>>;
    fn get_quote_by_date_symbol(&self, date: &i64, symbol: &String) -> Option<Quote>;
    fn has_next(&self) -> bool;
    fn step(&mut self);
}

#[derive(Clone)]
pub struct DataSource {
    quotes: HashMap<i64, Vec<Quote>>,
    dividends: HashMap<i64, Vec<Dividend>>,
    pos: usize,
    keys: Vec<i64>,
}

impl SimSource for DataSource {
    fn get_quotes_dates(&self) -> Vec<i64> {
        self.quotes.keys().map(|v| v.to_owned()).collect_vec()
    }

    fn get_quote_by_date_symbol(&self, date: &i64, symbol: &String) -> Option<Quote> {
        if let Some(quotes) = self.get_quotes_by_date(date) {
            for quote in &quotes {
                if quote.symbol.eq(symbol) {
                    return Some(quote.clone());
                }
            }
        }
        None
    }

    fn get_quotes_by_date(&self, date: &i64) -> Option<Vec<Quote>> {
        if let Some(quotes) = self.quotes.get(date) {
            return Some(quotes.clone());
        }
        None
    }

    fn get_dividends_by_date(&self, date: &i64) -> Option<Vec<Dividend>> {
        if let Some(dividends) = self.dividends.get(date) {
            return Some(dividends.clone());
        }
        None
    }

    fn step(&mut self) {
        self.pos += 1;
    }

    fn has_next(&self) -> bool {
        self.pos < self.keys.len()
    }
}

impl DataSource {
    pub fn from_hashmap(
        quotes: HashMap<i64, Vec<Quote>>,
        dividends: HashMap<i64, Vec<Dividend>>,
    ) -> DataSource {
        let keys = quotes.keys().map(|k| k.clone()).collect();
        DataSource {
            quotes,
            pos: 0,
            keys,
            dividends,
        }
    }
}
