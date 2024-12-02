#![allow(dead_code)]
use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use tokio_pg_mapper::FromTokioPostgresRow;
use tokio_postgres::{Client, NoTls};

use crate::source::hyperliquid::{DateDepth, DateTrade, Depth, Level, Side};

pub struct Minerva {
    db: Client,
}

#[derive(tokio_pg_mapper::PostgresMapper, Clone, Debug)]
#[pg_mapper(table = "l2Book")]
pub struct L2Book {
    coin: String,
    side: bool,
    px: String,
    sz: String,
    time: i64,
}

#[derive(tokio_pg_mapper::PostgresMapper, Clone, Debug)]
#[pg_mapper(table = "trade")]
pub struct Trade {
    pub coin: String,
    pub side: String,
    pub px: String,
    pub sz: String,
    pub hash: String,
    pub time: i64,
    pub tid: i64,
}

impl From<Trade> for crate::source::hyperliquid::Trade {
    fn from(value: Trade) -> Self {
        let side = if value.side == "B" {
            Side::Bid
        } else {
            Side::Ask
        };

        Self {
            coin: value.coin,
            side,
            px: str::parse::<f64>(&value.px).unwrap(),
            sz: str::parse::<f64>(&value.sz).unwrap(),
            time: value.time,
        }
    }
}

impl From<Vec<L2Book>> for Depth {
    fn from(values: Vec<L2Book>) -> Self {
        let mut bids = Vec::with_capacity(5);
        let mut asks = Vec::with_capacity(5);

        let date = values.first().unwrap().time;
        let symbol = values.first().unwrap().coin.clone();

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
        }
    }
}

impl Minerva {
    pub async fn new(connection_string: &str) -> Minerva {
        if let Ok(client) = Minerva::get_connection(connection_string).await {
            return Minerva { db: client };
        }
        panic!("Could not connect to database")
    }

    async fn get_connection(connection_string: &str) -> Result<Client> {
        let (client, connection) = tokio_postgres::connect(connection_string, NoTls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        Ok(client)
    }

    pub async fn get_date_bounds(&self) -> Option<(i64, i64)> {
        //TODO: should cache result because this is potentially crippling
        let _query_result = self
            .db
            .query("select min(time), max(time) from trade", &[])
            .await;
        unimplemented!()
    }

    pub async fn get_trades(&self, dates: std::ops::Range<i64>) -> DateTrade {
        let start_date = dates.start;
        let end_date = dates.end;

        let query_result = self
            .db
            .query(
                "select * from trade where time between $1 and $2",
                &[&start_date, &end_date],
            )
            .await;

        let mut res = BTreeMap::new();
        if let Ok(rows) = query_result {
            for row in rows {
                if let Ok(trade) = Trade::from_row(row) {
                    let hl_trade: crate::source::hyperliquid::Trade = trade.into();

                    res.entry(hl_trade.time).or_insert_with(Vec::new);

                    let date_trades = res.get_mut(&hl_trade.time)
                        .unwrap();
                    date_trades.push(hl_trade);
                }
            }
        }
        res
    }

    pub async fn get_depth_between(&self, dates: std::ops::Range<i64>) -> BTreeMap<i64, DateDepth> {
        //Looks weird right now but we need this to work with BTreeMap because we will want to
        //cache values rather than send every request to DB
        let start_date = dates.start;
        let end_date = dates.end;

        let query_result = self
            .db
            .query(
                "select * from l2Book where time between $1 and $2",
                &[&start_date, &end_date],
            )
            .await;

        let mut sort_into_dates = HashMap::new();
        if let Ok(rows) = query_result {
            for row in rows {
                if let Ok(book) = L2Book::from_row(row) {
                    sort_into_dates.entry(book.time).or_insert_with(Vec::new);
                    sort_into_dates
                        .get_mut(&book.time)
                        .unwrap()
                        .push(book.clone());
                }
            }
        }

        let mut depth_result = BTreeMap::new();
        for (date, rows) in sort_into_dates.iter_mut() {
            let depth: Depth = std::mem::take(rows).into();

            depth_result.entry(*date).or_insert_with(BTreeMap::new);
            depth_result
                .get_mut(date)
                .unwrap()
                .insert(depth.symbol.clone(), depth);
        }

        depth_result
    }
}
