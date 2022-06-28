use itertools::Itertools;

#[derive(Clone)]
pub struct TimeSeries {
    index: Vec<f64>,
    values: Vec<f64>,
}

impl TimeSeries {
    pub fn get_values(&self) -> Vec<f64> {
        self.values.clone()
    }

    pub fn get_index(&self) -> Vec<f64> {
        self.index.clone()
    }

    pub fn pct_change_log(&self) -> Vec<f64> {
        let mut res: Vec<f64> = Vec::new();
        let mut temp = &self.values[0];
        for i in self.values.iter().skip(1) {
            let pct_change = *i / *temp;
            res.push(pct_change.log10());
            temp = i
        }
        res
    }

    pub fn pct_change(&self) -> Vec<f64> {
        let mut res: Vec<f64> = Vec::new();
        let mut temp = &self.values[0];
        for i in self.values.iter().skip(1) {
            res.push((i / temp) - 1.0);
            temp = i;
        }
        res
    }

    pub fn maxdd(&self) -> f64 {
        let mut maxdd = 0.0;
        let mut peak = 0.0;
        let mut trough = 0.0;
        let mut t2;

        for t1 in &self.values {
            if t1 > &peak {
                peak = *t1;
                trough = peak;
            } else if t1 < &trough {
                trough = *t1;
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

    pub fn get(&self, idx: usize) -> Option<&f64> {
        self.values.get(idx)
    }

    pub fn append(&mut self, idx: Option<f64>, value: f64) {
        if let Some(idx_val) = idx {
            self.index.push(idx_val);
            self.values.push(value);
        } else {
            let mut last = 0.0;
            if let Some(idx_last) = self.index.last() {
                last = *idx_last;
            }
            self.index.push(last);
            self.values.push(value);
        }
    }

    pub fn new<T: Into<f64>>(index: Option<Vec<T>>, values: Vec<T>) -> Self {
        let mut val_converted: Vec<f64> = Vec::new();
        for v in values {
            let val = v.into();
            val_converted.push(val);
        }

        if let Some(index_val) = index {
            let mut idx_converted: Vec<f64> = Vec::new();
            for i in index_val {
                let val = i.into();
                idx_converted.push(val);
            }

            TimeSeries {
                index: idx_converted,
                values: val_converted,
            }
        } else if val_converted.is_empty() {
            TimeSeries {
                index: Vec::new(),
                values: Vec::new(),
            }
        } else {
            let idx = (0..val_converted.len() - 1).map(|v| v as f64).collect_vec();
            TimeSeries {
                index: idx,
                values: val_converted,
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
        TimeSeries::new(Option::None, fake_prices)
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
