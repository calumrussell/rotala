#![allow(dead_code)]
use std::collections::{btree_map::Range, BTreeMap, HashMap};

use deadpool_postgres::Pool;
use serde_json::Value;
use tokio_pg_mapper::FromTokioPostgresRow;

use crate::source::hyperliquid::{DateDepth, DateTrade, Depth, Level, Side};

#[derive(tokio_pg_mapper::PostgresMapper, Clone, Debug)]
#[pg_mapper(table = "depth")]
pub struct L2Book {
    coin: String,
    side: bool,
    px: String,
    sz: String,
    time: i64,
    exchange: String,
    meta: Value,
}

#[derive(tokio_pg_mapper::PostgresMapper, Clone, Debug)]
#[pg_mapper(table = "trade")]
pub struct Trade {
    pub coin: String,
    pub side: bool,
    pub px: String,
    pub sz: String,
    pub time: i64,
    pub exchange: String,
    pub meta: Value,
}

impl From<Trade> for crate::source::hyperliquid::Trade {
    fn from(value: Trade) -> Self {
        let side = if !value.side { Side::Bid } else { Side::Ask };

        Self {
            coin: value.coin,
            side,
            px: str::parse::<f64>(&value.px).unwrap(),
            sz: str::parse::<f64>(&value.sz).unwrap(),
            time: value.time,
            exchange: value.exchange,
        }
    }
}

impl From<Vec<L2Book>> for Depth {
    fn from(values: Vec<L2Book>) -> Self {
        let mut bids = Vec::with_capacity(5);
        let mut asks = Vec::with_capacity(5);

        let date = values.first().unwrap().time;
        let symbol = values.first().unwrap().coin.clone();
        let exchange = values.first().unwrap().exchange.clone();

        for row in values {
            match row.side {
                true => bids.push(Level {
                    price: str::parse::<f64>(&row.px).unwrap(),
                    size: str::parse::<f64>(&row.sz).unwrap(),
                }),
                false => asks.push(Level {
                    price: str::parse::<f64>(&row.px).unwrap(),
                    size: str::parse::<f64>(&row.sz).unwrap(),
                }),
            }
        }

        Depth {
            bids,
            asks,
            date,
            symbol,
            exchange,
        }
    }
}

pub struct Minerva {
    trades: DateTrade,
    depths: BTreeMap<i64, DateDepth>,
}

impl Default for Minerva {
    fn default() -> Self {
        Self::new()
    }
}

impl Minerva {
    pub fn new() -> Self {
        Self {
            trades: BTreeMap::new(),
            depths: BTreeMap::new(),
        }
    }

    async fn init_depth_between(&mut self, pool: &Pool, dates: &std::ops::Range<i64>) {
        //Looks weird right now but we need this to work with BTreeMap because we will want to
        //cache values rather than send every request to DB
        let start_date = dates.start;
        let end_date = dates.end;

        if let Ok(client) = pool.get().await {
            let query_result = client
                .query(
                    "select * from depth where time between $1 and $2",
                    &[&start_date, &end_date],
                )
                .await;

            let mut sort_into_dates: HashMap<i64, HashMap<String, HashMap<String, Vec<L2Book>>>> =
                HashMap::new();
            if let Ok(rows) = query_result {
                for row in rows {
                    if let Ok(book) = L2Book::from_row(row) {
                        sort_into_dates.entry(book.time).or_default();

                        let date = sort_into_dates.get_mut(&book.time).unwrap();

                        if !date.contains_key(&book.exchange) {
                            date.insert(book.exchange.clone(), HashMap::new());
                        }

                        let exchange = date.get_mut(&book.exchange).unwrap();

                        if !exchange.contains_key(&book.coin) {
                            exchange.insert(book.coin.clone(), Vec::new());
                        }

                        let coin_date: &mut Vec<L2Book> = exchange.get_mut(&book.coin).unwrap();
                        coin_date.push(book);
                    }
                }
            }

            for (date, exchange_map) in sort_into_dates.iter_mut() {
                for (exchange, coin_map) in exchange_map.iter_mut() {
                    for (coin, book) in coin_map.iter_mut() {
                        let depth: Depth = std::mem::take(book).into();
                        self.depths.entry(*date).or_default();

                        let date_map = self.depths.get_mut(date).unwrap();
                        date_map.entry(exchange.to_string()).or_default();
                        date_map
                            .get_mut(exchange)
                            .unwrap()
                            .insert(coin.to_string(), depth);
                    }
                }
            }
        }
    }

    async fn init_trades(&mut self, pool: &Pool, dates: &std::ops::Range<i64>) {
        let start_date = dates.start;
        let end_date = dates.end;

        if let Ok(client) = pool.get().await {
            let query_result = client
                .query(
                    "select * from trade where time between $1 and $2",
                    &[&start_date, &end_date],
                )
                .await;

            if let Ok(rows) = query_result {
                for row in rows {
                    if let Ok(trade) = Trade::from_row(row) {
                        let hl_trade: crate::source::hyperliquid::Trade = trade.into();

                        self.trades.entry(hl_trade.time).or_default();

                        let date_trades = self.trades.get_mut(&hl_trade.time).unwrap();
                        date_trades.push(hl_trade);
                    }
                }
            }
        }
    }

    pub async fn get_date_bounds(&self, pool: &Pool) -> Option<(i64, i64)> {
        if let Ok(client) = pool.get().await {
            let query_result = client
                .query("select min(time), max(time) from trade", &[])
                .await;

            if let Ok(rows) = query_result {
                let first = rows.first().unwrap();
                return Some((first.get(0), first.get(1)));
            };
        }
        None
    }

    pub async fn get_trades_between(
        &self,
        dates: std::ops::Range<i64>,
    ) -> Range<i64, Vec<crate::source::hyperliquid::Trade>> {
        self.trades.range(dates)
    }

    pub async fn get_depth_between(&self, dates: std::ops::Range<i64>) -> Range<i64, DateDepth> {
        self.depths.range(dates)
    }

    pub async fn init_cache(&mut self, pool: &Pool, dates: std::ops::Range<i64>) {
        self.init_trades(pool, &dates).await;
        self.init_depth_between(pool, &dates).await;
    }
}
