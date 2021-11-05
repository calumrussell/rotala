pub mod universe;

use itertools::Itertools;
use std::collections::HashMap;
use std::error::Error;

use crate::broker::Quote;

type DataSourceResp = Result<Quote, Box<dyn Error>>;

pub trait Quotable {
    fn get_latest_quote(&self, symbol: &String) -> DataSourceResp;
}

pub struct DataSource<T: Quotable> {
    _source: T,
}

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

pub struct TimeSeries {
    index: Vec<f64>,
    values: Vec<f64>,
}

impl TimeSeries {
    pub fn pct_change_log(&self) -> Vec<f64> {
        let mut res: Vec<f64> = Vec::new();
        let mut temp = &self.values[0];
        for i in self.values.iter().skip(1).into_iter() {
            let pct_change = i / temp;
            res.push(pct_change.log10());
            temp = i
        }
        res
    }

    pub fn pct_change(&self) -> Vec<f64> {
        let mut res: Vec<f64> = Vec::new();
        let mut temp = &self.values[0];
        for i in self.values.iter().skip(1).into_iter() {
            res.push((i / temp) - 1.0);
            temp = &i;
        }
        res
    }

    pub fn maxdd(&self) -> f64 {
        let mut maxdd = 0.0;
        let mut peak = 0.0;
        let mut trough = 0.0;
        let mut t2 = 0.0;

        for t1 in &self.values {
            if t1 > &peak {
                peak = t1.clone();
                trough = peak;
            } else if t1 < &trough {
                trough = t1.clone();
                t2 = (trough / peak) - 1.0;
                if t2 < maxdd {
                    maxdd = t2
                }
            }
        }
        maxdd
    }

    pub fn count(&self) -> usize {
        self.values.len()
    }

    pub fn var(&self) -> f64 {
        let mean: f64 = self.values.iter().sum::<f64>() / (self.count() as f64);
        let squared_diffs = self
            .values
            .iter()
            .map(|ret| ret - mean)
            .map(|diff| diff.powf(2.0))
            .collect_vec();
        let sum_of_diff = squared_diffs.iter().sum::<f64>();
        sum_of_diff / (self.count() as f64)
    }

    pub fn vol(&self) -> f64 {
        //Accepts returns not raw portfolio values
        self.var().sqrt()
    }

    pub fn append(&mut self, idx: Option<f64>, value: f64) {
        if idx.is_some() {
            self.index.push(idx.unwrap());
            self.values.push(value);
        } else {
            let idx_last = self.index.last();
            if idx_last.is_none() {
                self.index.push(0.0);
            } else {
                self.index.push(idx_last.unwrap() + 1.0);
            }
            self.values.push(value)
        }
    }

    pub fn new(index: Option<Vec<f64>>, values: Vec<f64>) -> Self {
        if index.is_some() {
            TimeSeries {
                index: index.unwrap(),
                values,
            }
        } else {
            if values.len() == 0 {
                TimeSeries { index: Vec::new(), values }
            } else {
                let idx = (0..values.len() - 1).map(|v| v as f64).collect_vec();
                TimeSeries { index: idx, values }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TimeSeries;

    fn setup() -> TimeSeries {
        let mut fake_prices: Vec<f64> = Vec::new();
        fake_prices.push(100.0);
        fake_prices.push(105.0);
        fake_prices.push(120.0);
        fake_prices.push(80.0);
        fake_prices.push(90.0);
        TimeSeries::new(None, fake_prices)
    }

    #[test]
    fn test_that_returns_calculates_correctly() {
        let ts = setup();
        let rets = ts.pct_change();
        let sum = rets.iter().map(|v| (1.0 + v).log10()).sum::<f64>();
        let val = (10_f64.powf(sum) - 1.0) * 100.0;
        assert_eq!(val.round(), -10.0)
    }

    #[test]
    fn test_that_vol_calculates_correctly() {
        let ts = setup();
        let rets = TimeSeries::new(None, ts.pct_change());
        assert_eq!((rets.vol() * 100.0).round(), 19.0);

        let log_rets = TimeSeries::new(None, ts.pct_change_log());
        assert_eq!((log_rets.vol() * 100.0).round(), 10.0);
    }

    #[test]
    fn test_that_mdd_calculates_correctly() {
        let ts = setup();
        let mdd = ts.maxdd();
        assert_eq!((mdd * 100.0).round(), -33.0);
    }
}
