use itertools::Itertools;
use std::collections::HashMap;
use std::error::Error;

use crate::broker::Quote;

type DataSourceResp = Result<Quote, Box<dyn Error>>;

/* Abstracts basic data operations for components that use data.
 */
pub trait SimSource {
    fn get_keys(&self) -> Vec<&i64>;
    fn get_date(&self, date: &i64) -> Option<&Vec<Quote>>;
    fn get_date_symbol(&self, date: &i64, symbol: &String) -> DataSourceResp;
    fn has_next(&self) -> bool;
    fn step(&mut self);
}

#[derive(Clone)]
pub struct DataSource {
    data: HashMap<i64, Vec<Quote>>,
    pos: usize,
    keys: Vec<i64>,
}

impl SimSource for DataSource {
    fn get_keys(&self) -> Vec<&i64> {
        self.data.keys().collect_vec()
    }

    fn get_date_symbol(&self, date: &i64, symbol: &String) -> DataSourceResp {
        let date = self.get_date(date);
        if date.is_none() {
            return Err("Date not found".into());
        }
        let match_symbol = date.unwrap().iter().find(|q| q.symbol.eq(symbol));
        if let Some(m) = match_symbol {
            return Ok(m.clone());
        }
        Err("Symbol not found".into())
    }

    fn get_date(&self, date: &i64) -> Option<&Vec<Quote>> {
        self.data.get(date)
    }

    fn step(&mut self) {
        self.pos += 1;
    }

    fn has_next(&self) -> bool {
        self.pos < self.keys.len()
    }
}

impl DataSource {
    pub fn from_hashmap(data: HashMap<i64, Vec<Quote>>) -> DataSource {
        let keys = data.keys().map(|k| k.clone()).collect();
        DataSource { data, pos: 0, keys }
    }
}
