#![allow(dead_code)]

use std::collections::btree_map::Range;
use std::collections::BTreeMap;
use std::path::Path;

use rand::thread_rng;
use rand_distr::{Distribution, Uniform};

use crate::source::hyperliquid::{get_hyperliquid_l2, DateBBO, DateDepth, Depth, Level, Side};

pub struct Athena {
    inner: BTreeMap<i64, DateDepth>,
}

impl Athena {
    pub fn get_date_bounds(&self) -> Option<(i64, i64)> {
        let first_date = *self.inner.first_key_value().unwrap().0;
        let last_date = *self.inner.last_key_value().unwrap().0;
        Some((first_date, last_date))
    }

    pub fn get_quotes_between(&self, dates: std::ops::Range<i64>) -> Range<i64, DateDepth> {
        self.inner.range(dates)
    }

    pub fn get_best_bid(
        &self,
        dates: std::ops::Range<i64>,
        symbol: &str,
        exchange: &str,
    ) -> Option<&Level> {
        let depth_between = self.get_quotes_between(dates);
        if let Some(last_depth) = depth_between.last() {
            if let Some(exchange_depth) = last_depth.1.get(exchange) {
                if let Some(coin_depth) = exchange_depth.get(symbol) {
                    return coin_depth.get_best_bid();
                }
            }
        }
        None
    }

    pub fn get_best_ask(
        &self,
        dates: std::ops::Range<i64>,
        symbol: &str,
        exchange: &str,
    ) -> Option<&Level> {
        let depth_between = self.get_quotes_between(dates);
        if let Some(last_depth) = depth_between.last() {
            if let Some(exchange_depth) = last_depth.1.get(exchange) {
                if let Some(coin_depth) = exchange_depth.get(symbol) {
                    return coin_depth.get_best_ask();
                }
            }
        }
        None
    }

    pub fn get_bbo(&self, dates: std::ops::Range<i64>, exchange: &str) -> Option<DateBBO> {
        let mut res = BTreeMap::new();

        let depth_between = self.get_quotes_between(dates);
        if let Some(last_depth) = depth_between.last() {
            if let Some(exchange_depth) = last_depth.1.get(exchange) {
                for (symbol, depth) in exchange_depth {
                    res.insert(symbol.clone(), depth.get_bbo()?);
                }
            }
        }
        Some(res)
    }

    pub fn add_depth(&mut self, depth: Depth) {
        let date = depth.date;
        let symbol = depth.symbol.clone();
        let exchange = depth.exchange.clone();

        self.inner.entry(date).or_default();

        let date_map = self.inner.get_mut(&date).unwrap();
        date_map.entry(exchange.to_string()).or_default();
        date_map.get_mut(&exchange).unwrap().insert(symbol, depth);
    }

    pub fn add_price_level(
        &mut self,
        date: i64,
        symbol: &str,
        level: Level,
        side: Side,
        exchange: &str,
    ) {
        self.inner.entry(date).or_default();

        let date_map = self.inner.get_mut(&date).unwrap();
        date_map.entry(exchange.to_string()).or_default();

        let date_levels = date_map.get_mut(&exchange.to_string()).unwrap();
        if let Some(depth) = date_levels.get_mut(symbol) {
            depth.add_level(level, side)
        } else {
            let depth = match side {
                Side::Bid => Depth {
                    bids: vec![level],
                    asks: vec![],
                    symbol: symbol.to_string(),
                    date,
                    exchange: exchange.to_string(),
                },
                Side::Ask => Depth {
                    bids: vec![],
                    asks: vec![level],
                    symbol: symbol.to_string(),
                    date,
                    exchange: exchange.to_string(),
                },
            };

            date_levels.insert(symbol.to_string(), depth);
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

                source.add_price_level(date, symbol, bid_level, Side::Bid, "exchange");
                source.add_price_level(date, symbol, ask_level, Side::Ask, "exchange");
            }
        }
        source
    }

    pub fn from_file(path: &Path) -> Self {
        let hl_source = get_hyperliquid_l2(path);

        let mut athena = Self::new();
        for (_key, value) in hl_source {
            let into_depth: Depth = value.into();
            athena.add_depth(into_depth);
        }
        athena
    }

    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
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
    use crate::source::hyperliquid::{Level, Side};

    use super::Athena;

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

        athena.add_price_level(100, "ABC", bid0, Side::Bid, "exchange");
        athena.add_price_level(100, "ABC", ask0, Side::Ask, "exchange");

        athena.add_price_level(100, "ABC", bid1, Side::Bid, "exchange");
        athena.add_price_level(100, "ABC", ask1, Side::Ask, "exchange");

        println!(
            "{:?}",
            athena.get_best_bid(100..101, "ABC", "exchange").unwrap()
        );
        assert_eq!(
            athena
                .get_best_bid(100..101, "ABC", "exchange")
                .unwrap()
                .price,
            101.0
        );
        assert_eq!(
            athena
                .get_best_ask(100..101, "ABC", "exchange")
                .unwrap()
                .price,
            102.0
        );
    }
}
