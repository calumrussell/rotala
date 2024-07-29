#![allow(dead_code)]

use std::{borrow::Borrow, collections::HashSet};
use std::collections::HashMap;

use rand::thread_rng;
use rand_distr::{Distribution, Uniform};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Side {
    Bid,
    Ask,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Level {
    pub price: f64,
    pub size: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Depth {
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
    pub date: i64,
    pub symbol: String,
}

impl Depth {
    pub fn add_level(&mut self, level: Level, side: Side) {
        match side {
            Side::Bid => {
                self.bids.push(level);
                self.bids
                    .sort_by(|x, y| x.price.partial_cmp(&y.price).unwrap().reverse());
            }
            Side::Ask => {
                self.asks.push(level);
                self.asks
                    .sort_by(|x, y| x.price.partial_cmp(&y.price).unwrap());
            }
        }
    }

    pub fn get_best_bid(&self) -> Option<&Level> {
        self.bids.first()
    }

    pub fn get_best_ask(&self) -> Option<&Level> {
        self.asks.first()
    }

    pub fn get_bbo(&self) -> Option<BBO> {
        let best_bid = self.get_best_bid()?;
        let best_ask = self.get_best_ask()?;

        Some(BBO {
            bid: best_bid.price,
            bid_volume: best_bid.size,
            ask: best_ask.price,
            ask_volume: best_ask.size,
            symbol: self.symbol.clone(),
            date: self.date,
        })
    }

    pub fn new(date: i64, symbol: impl Into<String>) -> Self {
        Self {
            bids: vec![],
            asks: vec![],
            date,
            symbol: symbol.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BBO {
    pub bid: f64,
    pub bid_volume: f64,
    pub ask: f64,
    pub ask_volume: f64,
    pub symbol: String,
    pub date: i64,
}

pub type DateQuotes = HashMap<String, Depth>;

pub struct Athena {
    dates: Vec<i64>,
    //TODO: this is not great, added because the dates weren't being added at all, not sure if this
    //is really ideal path
    dates_seen: HashSet<i64>,
    inner: HashMap<i64, DateQuotes>,
}

impl Athena {
    pub fn get_quotes(&self, date: &i64) -> Option<&DateQuotes> {
        self.inner.get(date)
    }

    fn get_quotes_unchecked(&self, date: &i64) -> &DateQuotes {
        self.get_quotes(date).unwrap()
    }

    pub fn get_date(&self, pos: usize) -> Option<&i64> {
        self.dates.get(pos)
    }

    pub fn has_next(&self, pos: usize) -> bool {
        self.dates.len() > pos
    }

    pub fn get_best_bid(&self, date: impl Borrow<i64>, symbol: &str) -> Option<&Level> {
        let date_levels = self.inner.get(date.borrow())?;
        let depth = date_levels.get(symbol)?;
        depth.get_best_bid()
    }

    pub fn get_best_ask(&self, date: impl Borrow<i64>, symbol: &str) -> Option<&Level> {
        let date_levels = self.inner.get(date.borrow())?;
        let depth = date_levels.get(symbol)?;
        depth.get_best_ask()
    }

    pub fn get_bbo(&self, date: impl Borrow<i64>, symbol: &str) -> Option<BBO> {
        let date_levels = self.inner.get(date.borrow())?;
        let depth = date_levels.get(symbol)?;
        depth.get_bbo()
    }

    pub fn add_price_level(&mut self, date: i64, symbol: &str, level: Level, side: Side) {
        self.inner.entry(date).or_default();

        let symbol_string = symbol.into();

        //We will always have a value due to the above block so can unwrap safely
        let date_levels = self.inner.get_mut(&date).unwrap();
        if let Some(depth) = date_levels.get_mut(&symbol_string) {
            depth.add_level(level, side)
        } else {
            let depth = match side {
                Side::Bid => Depth {
                    bids: vec![level],
                    asks: vec![],
                    symbol: symbol.to_string(),
                    date,
                },
                Side::Ask => Depth {
                    bids: vec![],
                    asks: vec![level],
                    symbol: symbol.to_string(),
                    date,
                },
            };

            date_levels.insert(symbol_string, depth);
        }

        if !self.dates_seen.contains(&date) {
            self.dates.push(date);
            self.dates_seen.insert(date);
        }

    }

    pub fn random(length: i64, symbols: Vec<&str>) -> Self {
        let price_dist = Uniform::new(90.0, 100.0);
        let size_dist = Uniform::new(100.0, 1000.0);
        let mut rng = thread_rng();

        let mut source = Self::new();

        for date in 100..length + 100 {
            let random_price = price_dist.sample(&mut rng);
            let random_size = size_dist.sample(&mut rng);

            for symbol in &symbols {
                let bid_level = Level {
                    price: random_price * 1.01,
                    size: random_size,
                };

                let ask_level = Level {
                    price: random_price * 0.99,
                    size: random_size,
                };

                source.add_price_level(date, symbol, bid_level, Side::Bid);
                source.add_price_level(date, symbol, ask_level, Side::Ask);
            }
        }
        source
    }

    pub fn new() -> Self {
        Self {
            dates: Vec::new(),
            inner: HashMap::new(),
            dates_seen: HashSet::new(),
        }
    }
}

impl Default for Athena {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Athena, Level, Side};

    #[test]
    fn test_that_insertions_are_sorted() {
        let mut athena = Athena::new();

        let bid0 = Level {
            price: 100.0,
            size: 100.0,
        };

        let bid1 = Level {
            price: 101.0,
            size: 100.0,
        };

        let ask0 = Level {
            price: 102.0,
            size: 100.0,
        };

        let ask1 = Level {
            price: 103.0,
            size: 100.0,
        };

        athena.add_price_level(100, "ABC", bid0, Side::Bid);
        athena.add_price_level(100, "ABC", ask0, Side::Ask);

        athena.add_price_level(100, "ABC", bid1, Side::Bid);
        athena.add_price_level(100, "ABC", ask1, Side::Ask);

        assert_eq!(athena.get_best_bid(100, "ABC").unwrap().price, 101.0);
        assert_eq!(athena.get_best_ask(100, "ABC").unwrap().price, 102.0);
    }
}
