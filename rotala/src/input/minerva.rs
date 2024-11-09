use core::panic;
use std::collections::BTreeMap;

use anyhow::Result;
use tokio_pg_mapper::FromTokioPostgresRow;
use tokio_postgres::{Client, NoTls, Row};

pub struct Minerva {
    db: Client,
}

#[derive(tokio_pg_mapper::PostgresMapper, Debug)]
#[pg_mapper(table = "l2Book")]
pub struct L2Book {
    coin: String,
    side: bool,
    px: String,
    sz: String,
    time: i64,
}

#[derive(tokio_pg_mapper::PostgresMapper, Debug)]
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
    ) -> BTreeMap<i64, L2Book> {
        let query_result = self
            .db
            .query(
                "select * from l2Book where coin=$1::TEXT and time between $2 and $3",
                &[&coin, &start_date, &end_date],
            )
            .await;

        let mut res = BTreeMap::new();
        if let Ok(rows) = query_result {
            for row in rows {
                if let Ok(book) = L2Book::from_row(row) {
                    res.insert(book.time, book);
                }
            }
        }
        res
    }
}
