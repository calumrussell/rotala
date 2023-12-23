use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{source::get_binance_1m_klines, clock::{Clock, DateTime}};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PenelopeQuote {
    pub bid: f64,
    pub ask: f64,
    pub date: i64,
    pub symbol: String,
}

impl PenelopeQuote {
    pub fn get_bid(&self) -> f64 {
        self.bid
    }

    pub fn get_ask(&self) -> f64 {
        self.ask
    }

    pub fn get_symbol(&self) -> String {
        self.symbol.clone()
    }

    pub fn get_date(&self) -> i64 {
        self.date
    }
}

#[derive(Debug)]
pub struct Penelope {
    inner: HashMap<i64, HashMap<String, PenelopeQuote>>,
}

impl Penelope {
    pub fn get_quote(&self, date: &i64, symbol: &str) -> Option<&PenelopeQuote> {
        if let Some(date_row) = self.inner.get(date) {
            if let Some(quote) = date_row.get(symbol) {
                return Some(quote);
            }
        }
        None
    }

    pub fn get_quotes(&self, date: &i64) -> Option<Vec<PenelopeQuote>> {
        if let Some(date_row) = self.inner.get(date) {
            return Some(date_row.values().cloned().collect());
        }
        None
    }

    pub fn add_quotes(&mut self, bid: f64, ask: f64, date: i64, symbol: impl Into<String> + Clone) {
        let quote = PenelopeQuote {
            bid,
            ask,
            date,
            symbol: symbol.clone().into(),
        };

        if let Some(date_row) = self.inner.get_mut(&date) {
            date_row.insert(symbol.into(), quote);
        } else {
            let mut date_row = HashMap::new();
            date_row.insert(symbol.into(), quote);
            self.inner.insert(date, date_row);
        }
    }

    pub fn from_binance() -> (Self, Clock) {
        let mut penelope = Penelope::new();

        let mut dates: HashSet<DateTime> = HashSet::new();
        for record in get_binance_1m_klines() {
            penelope.add_quotes(record.open, record.open, record.open_date, "BTC");
            penelope.add_quotes(record.close, record.close, record.close_date, "BTC");
            dates.insert(record.open_date.into());
            dates.insert(record.close_date.into());
        }
        let clock = Clock::from_fixed(Vec::from_iter(dates));
        (penelope, clock)
    }

    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn from_hashmap(inner: HashMap<i64, HashMap<String, PenelopeQuote>>) -> Self {
        Self { inner }
    }
}

impl Default for Penelope {
    fn default() -> Self {
        Self::new()
    }
}
