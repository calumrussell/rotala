use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    clock::{Clock, ClockBuilder, DateTime, Frequency},
    source::get_binance_1m_klines,
};

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
    pub fn from_binance() -> (Self, Clock) {
        let mut builder = PenelopeBuilder::new();

        for record in get_binance_1m_klines() {
            builder.add_quote(record.open, record.open, record.open_date, "BTC");
            builder.add_quote(record.close, record.close, record.close_date, "BTC");
        }
        builder.build()
    }

    pub fn from_hashmap(inner: HashMap<i64, HashMap<String, PenelopeQuote>>) -> Self {
        Self { inner }
    }
}

pub struct PenelopeBuilder {
    inner: HashMap<i64, HashMap<String, PenelopeQuote>>,
    dates: HashSet<DateTime>,
}

impl PenelopeBuilder {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
            dates: HashSet::new(),
        }
    }

    pub fn build_with_frequency(&self, frequency: Frequency) -> (Penelope, Clock) {
        // FIX: there is a clone of the underlying hashmap/dates which is very expensive, need to std::move
        match frequency {
            Frequency::Fixed => (
                Penelope::from_hashmap(self.inner.clone()),
                Clock::from_fixed(Vec::from_iter(self.dates.clone())),
            ),
            Frequency::Daily => {
                let mut dates_vec = Vec::from_iter(self.dates.clone());
                dates_vec.sort();
                let first = **dates_vec.first().unwrap();
                let last = **dates_vec.last().unwrap();
                let gap = ((last + 1) - first) / 86400;
                let clock = ClockBuilder::with_length_in_days(first, gap)
                    .with_frequency(&Frequency::Daily)
                    .build();
                (Penelope::from_hashmap(self.inner.clone()), clock)
            }
            Frequency::Second => {
                let mut dates_vec = Vec::from_iter(self.dates.clone());
                dates_vec.sort();
                let first = **dates_vec.first().unwrap();
                let last = **dates_vec.last().unwrap();
                let gap = (last + 1) - first;
                let clock = ClockBuilder::with_length_in_seconds(first, gap)
                    .with_frequency(&Frequency::Second)
                    .build();
                (Penelope::from_hashmap(self.inner.clone()), clock)
            }
        }
    }

    pub fn build(&self) -> (Penelope, Clock) {
        // FIX: there is a clone of the underlying hashmap/dates which is very expensive, need to std::move
        (
            Penelope::from_hashmap(self.inner.clone()),
            Clock::from_fixed(Vec::from_iter(self.dates.clone())),
        )
    }

    pub fn add_quote(&mut self, bid: f64, ask: f64, date: i64, symbol: impl Into<String> + Clone) {
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

        self.dates.insert(date.into());
    }
}

impl Default for PenelopeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
