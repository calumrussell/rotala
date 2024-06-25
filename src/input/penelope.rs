use std::collections::HashMap;

use rand::thread_rng;
use rand_distr::{Distribution, Uniform};
use serde::{Deserialize, Serialize};

use crate::source::get_binance_1m_klines;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PenelopeQuote {
    pub bid: f64,
    pub ask: f64,
    pub symbol: String,
    pub date: i64,
}

pub type PenelopeQuoteByDate = HashMap<String, PenelopeQuote>;

// Penelope produces data for exchanges to use. Exchanges bind their underlying data representation
// to that used by Penelope: `PenelopeQuote`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Penelope {
    dates: Vec<i64>,
    inner: HashMap<i64, PenelopeQuoteByDate>,
}

impl Penelope {
    pub fn get_quotes(&self, date: &i64) -> Option<&PenelopeQuoteByDate> {
        self.inner.get(date)
    }

    pub fn get_quotes_unchecked(&self, date: &i64) -> &PenelopeQuoteByDate {
        self.get_quotes(date).unwrap()
    }

    pub fn get_date(&self, pos: usize) -> Option<&i64> {
        self.dates.get(pos)
    }

    pub fn has_next(&self, pos: usize) -> bool {
        self.dates.len()  > pos
    }

    pub fn new() -> Self {
        Self {
            dates: Vec::new(),
            inner: HashMap::new(),
        }
    }

    pub fn from_binance() -> Self {
        let mut penelope = Self::new();

        for record in get_binance_1m_klines() {
            penelope.add_quote(record.open, record.open, record.open_date, "BTC");
            penelope.add_quote(record.close, record.close, record.close_date, "BTC");
        }
        penelope
    }

    pub fn add_quote(&mut self, bid: f64, ask: f64, date: i64, symbol: impl Into<String> + Clone) {
        //Inserts should be in sorted order
        let quote = PenelopeQuote {
            bid,
            ask,
            date,
            symbol: symbol.into(),
        };

        if let Some(date_row) = self.inner.get_mut(&date) {
            date_row.insert(quote.symbol.clone(), quote);
        } else {
            let mut date_row = HashMap::new();
            date_row.insert(quote.symbol.clone(), quote);
            self.inner.insert(date, date_row);
            self.dates.push(date);
        }
    }

    pub fn random(length: i64, symbols: Vec<&str>) -> Penelope {
        let price_dist = Uniform::new(90.0, 100.0);
        let mut rng = thread_rng();

        let mut source = Penelope::new();

        for date in 100..length + 100 {
            for symbol in &symbols {
                source.add_quote(
                    price_dist.sample(&mut rng),
                    price_dist.sample(&mut rng),
                    date,
                    *symbol,
                );
            }
        }
        source
    }
}

impl Default for Penelope {
    fn default() -> Self {
        Self::new()
    }
}
