use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    clock::{Clock, ClockBuilder, DateTime, Frequency},
    exchange::{
        jura::{JuraQuote, JuraSource},
        uist::{UistQuote, UistSource},
    },
    source::get_binance_1m_klines,
};

pub trait PenelopeQuote {
    fn get_bid(&self) -> f64;
    fn get_ask(&self) -> f64;
    fn get_symbol(&self) -> String;
    fn get_date(&self) -> i64;
    fn create(bid: f64, ask: f64, date: i64, symbol: String) -> Self;
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Penelope<Q: PenelopeQuote + Clone> {
    inner: HashMap<i64, HashMap<String, Q>>,
}

impl<Q: PenelopeQuote + Clone> Penelope<Q> {
    pub fn get_quote(&self, date: &i64, symbol: &str) -> Option<&Q> {
        if let Some(date_row) = self.inner.get(date) {
            if let Some(quote) = date_row.get(symbol) {
                return Some(quote);
            }
        }
        None
    }

    pub fn get_quotes(&self, date: &i64) -> Option<Vec<Q>> {
        if let Some(date_row) = self.inner.get(date) {
            return Some(date_row.values().cloned().collect());
        }
        None
    }
    pub fn from_binance() -> (Self, Clock) {
        let mut builder = PenelopeBuilder::<Q>::new();

        for record in get_binance_1m_klines() {
            builder.add_quote(record.open, record.open, record.open_date, "BTC");
            builder.add_quote(record.close, record.close, record.close_date, "BTC");
        }
        builder.build()
    }

    pub fn from_hashmap(inner: HashMap<i64, HashMap<String, Q>>) -> Self {
        Self { inner }
    }
}

impl JuraSource for Penelope<JuraQuote> {
    fn get_quote(&self, date: &i64, security: &u64) -> Option<JuraQuote> {
        Self::get_quote(self, date, &security.to_string()).cloned()
    }
}

impl UistSource for Penelope<UistQuote> {
    fn get_quote(&self, date: &i64, security: &str) -> Option<UistQuote> {
        Self::get_quote(self, date, security).cloned()
    }
}

pub struct PenelopeBuilder<Q: PenelopeQuote + Clone> {
    inner: HashMap<i64, HashMap<String, Q>>,
    dates: HashSet<DateTime>,
}

impl<Q: PenelopeQuote + Clone> PenelopeBuilder<Q> {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
            dates: HashSet::new(),
        }
    }

    pub fn build_with_frequency(&self, frequency: Frequency) -> (Penelope<Q>, Clock) {
        // TODO: there is a clone of the underlying hashmap/dates which is very expensive, need to std::move
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

    pub fn build(&self) -> (Penelope<Q>, Clock) {
        // TODO: there is a clone of the underlying hashmap/dates which is very expensive, need to std::move
        (
            Penelope::from_hashmap(self.inner.clone()),
            Clock::from_fixed(Vec::from_iter(self.dates.clone())),
        )
    }

    pub fn add_quote(&mut self, bid: f64, ask: f64, date: i64, symbol: impl Into<String> + Clone) {
        let quote = PenelopeQuote::create(bid, ask, date, symbol.clone().into());

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

impl<Q: PenelopeQuote + Clone> Default for PenelopeBuilder<Q> {
    fn default() -> Self {
        Self::new()
    }
}
