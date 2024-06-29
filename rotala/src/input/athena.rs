use std::{borrow::Borrow, collections::HashMap};

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
}

impl Depth {
    pub fn add_level(&mut self, level: Level, side: Side) {
        match side {
            Side::Bid => {
                self.bids.push(level);
                self.bids
                    .sort_by(|x, y| x.price.partial_cmp(&y.price).unwrap());
            }
            Side::Ask => {
                self.asks.push(level);
                self.asks
                    .sort_by(|x, y| x.price.partial_cmp(&y.price).unwrap());
            }
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

pub struct Athena {
    dates: Vec<i64>,
    inner: HashMap<i64, HashMap<String, Depth>>,
}

impl Athena {
    pub fn get_best_bid(
        &self,
        date: impl Borrow<i64>,
        symbol: impl Into<String>,
    ) -> Option<&Level> {
        if let Some(date_levels) = self.inner.get(date.borrow()) {
            if let Some(depth) = date_levels.get(&symbol.into()) {
                return depth.bids.last();
            }
        }
        None
    }

    pub fn get_best_ask(
        &self,
        date: impl Borrow<i64>,
        symbol: impl Into<String>,
    ) -> Option<&Level> {
        if let Some(date_levels) = self.inner.get(date.borrow()) {
            if let Some(depth) = date_levels.get(&symbol.into()) {
                return depth.asks.first();
            }
        }
        None
    }

    pub fn add_price_level(
        &mut self,
        date: i64,
        symbol: impl Into<String>,
        level: Level,
        side: Side,
    ) {
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
                    date,
                },
                Side::Ask => Depth {
                    bids: vec![],
                    asks: vec![level],
                    date,
                },
            };

            date_levels.insert(symbol_string, depth);
        }
    }

    pub fn new() -> Self {
        Self {
            dates: Vec::new(),
            inner: HashMap::new(),
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

        let level = Level {
            price: 100.0,
            size: 100.0,
        };

        let level1 = Level {
            price: 101.0,
            size: 100.0,
        };

        let level2 = Level {
            price: 102.0,
            size: 100.0,
        };

        athena.add_price_level(100, "ABC", level2, Side::Ask);
        athena.add_price_level(100, "ABC", level1, Side::Bid);
        athena.add_price_level(100, "ABC", level, Side::Bid);

        assert_eq!(athena.get_best_bid(100, "ABC").unwrap().price, 101.0);
        assert_eq!(athena.get_best_ask(100, "ABC").unwrap().price, 102.0);
    }
}
