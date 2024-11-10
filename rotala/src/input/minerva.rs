#![allow(dead_code)]
use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio_pg_mapper::FromTokioPostgresRow;
use tokio_postgres::{Client, NoTls};

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
    coin: String,
    side: String,
    px: String,
    sz: String,
    hash: String,
    time: i64,
    tid: i64,
}

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

    pub async fn get_trades(
        &self,
        start_date: &i64,
        end_date: &i64,
        coin: &str,
    ) -> BTreeMap<i64, Trade> {
        let query_result = self
            .db
            .query(
                "select * from trade where coin=$1::TEXT and time between $2 and $3",
                &[&coin, &start_date, &end_date],
            )
            .await;

        let mut res = BTreeMap::new();
        if let Ok(rows) = query_result {
            for row in rows {
                if let Ok(trade) = Trade::from_row(row) {
                    res.insert(trade.time, trade);
                }
            }
        }
        res
    }

    pub async fn get_depth(
        &self,
        start_date: &i64,
        end_date: &i64,
        coin: &str,
    ) -> BTreeMap<i64, Depth> {
        let query_result = self
            .db
            .query(
                "select * from l2Book where coin=$1::TEXT and time between $2 and $3",
                &[&coin, &start_date, &end_date],
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
            //TODO: this should be take
            let depth: Depth = rows.clone().into();
            depth_result.insert(*date, depth);
        }

        depth_result
    }
}
