use itertools::Itertools;
use std::collections::HashMap;
use std::error::Error;

use crate::broker::Quote;

type DataSourceResp = Result<Quote, Box<dyn Error>>;

pub trait SimSource {
    fn get_keys(&self) -> Vec<&i64>;
    fn get_date(&self, date: &i64) -> Option<&Vec<Quote>>;
    fn get_date_symbol(&self, date: &i64, symbol: &String) -> DataSourceResp;
    fn has_next(&self) -> bool;
    fn step(&mut self);
}

pub struct DataSourceSim<T>
where
    T: SimSource,
{
    pub source: T,
}

impl<T> DataSourceSim<T>
where
    T: SimSource,
{
    pub fn from_hashmap(data: HashMap<i64, Vec<Quote>>) -> DataSourceSim<DefaultDataSource> {
        let source = DefaultDataSource::new(data);
        DataSourceSim { source }
    }
}

pub struct DefaultDataSource {
    data: HashMap<i64, Vec<Quote>>,
    pos: usize,
    keys: Vec<i64>,
}

impl SimSource for DefaultDataSource {
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

impl DefaultDataSource {
    pub fn new(data: HashMap<i64, Vec<Quote>>) -> Self {
        let keys = data.keys().map(|k| k.clone()).collect();
        DefaultDataSource { data, pos: 0, keys }
    }
}
