pub mod universe;

use std::collections::HashMap;
use std::error::Error;
use std::rc::Rc;

use itertools::Itertools;

use crate::types::StockQuote;

type DataSourceResp = Result<StockQuote, Box<dyn Error>>;

pub trait DataSource {
    fn get_latest_quote(&self, symbol: &String) -> DataSourceResp;
}

pub trait DataSourceSim {
    fn get_latest_quote(&self, symbol: &String, date: &i64) -> DataSourceResp;
    fn get_keys(&self) -> Vec<&i64>;
    fn get_date(&self, date: &i64) -> Option<&Vec<StockQuote>>;
}

pub struct CSVDataSource {
    pub data: HashMap<i64, Vec<StockQuote>>,
}

pub struct CSVDataSourceWrapper {
    source: Rc<CSVDataSource>,
}

impl DataSourceSim for CSVDataSourceWrapper {
    fn get_latest_quote(&self, symbol: &String, date: &i64) -> DataSourceResp {
        let row = self.source.data.get(date);
        if row.is_none() {
            Err("Date not found".into())
        } else {
            let find = row.unwrap().iter().find(|r| r.symbol.eq(symbol));
            if find.is_some() {
                Ok(find.unwrap().clone())
            } else {
                Err("Symbol not found".into())
            }
        }
    }

    fn get_keys(&self) -> Vec<&i64> {
        self.source.data.keys().collect_vec()
    }

    fn get_date(&self, date: &i64) -> Option<&Vec<StockQuote>> {
        self.source.data.get(date)
    }
}

impl CSVDataSourceWrapper {
    pub fn new(data: Rc<CSVDataSource>) -> CSVDataSourceWrapper {
        CSVDataSourceWrapper { source: data }
    }
}
