use itertools::Itertools;

use crate::broker::{CashManager, PositionInfo};
use crate::portfolio::PortfolioStats;
use crate::trading;

enum DataFrequency {
    Daily,
    Monthly,
    Yearly,
}

struct PortfolioSnapshot {
    value: f64,
}

pub struct PortfolioCalculator;

impl PortfolioCalculator {
    fn get_log_returns(values: &Vec<f64>) -> Vec<f64> {
        let mut res: Vec<f64> = Vec::new();
        let mut temp = &values[0];
        for i in values.iter().skip(1).into_iter() {
            let pct_change = i / temp;
            res.push(pct_change.log10());
            temp = i
        }
        res
    }

    fn get_returns(values: &Vec<f64>) -> Vec<f64> {
        let mut res: Vec<f64> = Vec::new();
        let mut temp = values[0];
        for i in values.iter().skip(1).into_iter() {
            res.push((i / temp) - 1.0);
            temp = i.clone();
        }
        res
    }

    fn get_maxdd(values: &Vec<f64>) -> f64 {
        let mut maxdd = 0.0;
        let mut peak = 0.0;
        let mut trough = 0.0;
        let mut t2 = 0.0;

        for t1 in values {
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
        maxdd * 100.0
    }

    fn get_variance(values: &Vec<f64>) -> f64 {
        let mean: f64 = values.iter().sum::<f64>() / (values.len() as f64);
        let squared_diffs = values
            .iter()
            .map(|ret| ret - mean)
            .map(|diff| diff.powf(2.0))
            .collect_vec();
        let sum_of_diff = squared_diffs.iter().sum::<f64>();
        sum_of_diff / (values.len() as f64)
    }

    fn get_volatility(returns: &Vec<f64>) -> f64 {
        //Accepts returns not raw portfolio values
        (PortfolioCalculator::get_variance(returns).sqrt()) * 100.0
    }

    fn annualize_returns(ret: f64, trading_days: Option<i32>, frequency: DataFrequency) -> f64 {
        let mut days = 0.0;
        if trading_days.is_none() {
            days = 252.0;
        } else {
            days = trading_days.unwrap() as f64;
        }
        match frequency {
            DataFrequency::Daily => ((1.0 + (ret / 100.0)).powf(days) - 1.0) * 100.0,
            DataFrequency::Monthly => ((1.0 + (ret / 100.0)).powf(12.0) - 1.0) * 100.0,
            DataFrequency::Yearly => ret,
        }
    }

    fn annualize_volatility(vol: f64, trading_days: Option<i32>, frequency: DataFrequency) -> f64 {
        let mut days = 0.0;
        if trading_days.is_none() {
            days = 252.0;
        } else {
            days = trading_days.unwrap() as f64;
        }
        match frequency {
            DataFrequency::Daily => ((vol / 100.0) * days.sqrt()) * 100.0,
            DataFrequency::Monthly => ((vol / 100.0) * (12_f64).sqrt()) * 100.0,
            DataFrequency::Yearly => vol,
        }
    }
}

pub struct PortfolioPerformance {
    history: Vec<PortfolioSnapshot>,
    values: Option<Vec<f64>>,
}

impl PortfolioPerformance {
    fn to_values(&mut self) -> &Vec<f64> {
        if self.values.is_none() {
            let values = self
                .history
                .iter()
                .map(|snap| -> f64 { snap.value })
                .collect_vec();
            self.values = Some(values);
        }
        self.values.as_ref().unwrap()
    }

    pub fn get_portfolio_volatility(&mut self) -> f64 {
        let rets = PortfolioCalculator::get_returns(self.to_values());
        PortfolioCalculator::get_volatility(&rets)
    }

    pub fn get_portfolio_sharpe_ratio(&mut self) -> f64 {
        let vol = self.get_portfolio_volatility();
        let ret = self.get_portfolio_return();
        ret / vol
    }

    pub fn get_portfolio_max_dd(&mut self) -> f64 {
        PortfolioCalculator::get_maxdd(self.to_values())
    }

    pub fn get_portfolio_return(&mut self) -> f64 {
        let sum_log_rets = PortfolioCalculator::get_log_returns(&self.to_values())
            .iter()
            .sum();
        (10_f64.powf(sum_log_rets) - 1.0) * 100.0
    }

    pub fn update(&mut self, port: &impl PortfolioStats, brkr: &(impl PositionInfo + CashManager)) {
        let value = port.get_total_value(brkr);
        let snap = PortfolioSnapshot { value };
        self.history.push(snap);
    }

    pub fn new() -> Self {
        let history: Vec<PortfolioSnapshot> = Vec::new();
        PortfolioPerformance {
            history,
            values: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DataFrequency;
    use super::PortfolioCalculator;

    fn setup() -> Vec<f64> {
        let mut fake_prices: Vec<f64> = Vec::new();
        fake_prices.push(100.0);
        fake_prices.push(105.0);
        fake_prices.push(120.0);
        fake_prices.push(80.0);
        fake_prices.push(90.0);
        fake_prices
    }

    #[test]
    fn test_that_returns_calculates_correctly() {
        let values = setup();
        let rets = PortfolioCalculator::get_returns(&values);
        let sum = rets.iter().map(|v| (1.0 + v).log10()).sum::<f64>();
        let val = (10_f64.powf(sum) - 1.0) * 100.0;
        assert!(val.round() == -10.0)
    }

    #[test]
    fn test_that_vol_calculates_correctly() {
        let values = setup();
        let returns = PortfolioCalculator::get_returns(&values);
        let vol = PortfolioCalculator::get_volatility(&returns);

        let log_returns = PortfolioCalculator::get_log_returns(&values);
        let log_vol = PortfolioCalculator::get_volatility(&log_returns);

        assert!(vol.round() == 19.0);
        assert!(log_vol.round() == 10.0)
    }

    #[test]
    fn test_that_mdd_calculates_correctly() {
        let values = setup();
        let mdd = PortfolioCalculator::get_maxdd(&values);
        assert!(mdd.round() == -33.0);
    }

    #[test]
    fn test_that_annualizations_calculate_correctly() {
        assert_eq!(
            PortfolioCalculator::annualize_returns(0.1, None, DataFrequency::Daily).round(),
            29.0
        );
        assert_eq!(
            PortfolioCalculator::annualize_returns(2.0, None, DataFrequency::Monthly).round(),
            27.0
        );
        assert_eq!(
            PortfolioCalculator::annualize_returns(27.0, None, DataFrequency::Yearly).round(),
            27.0
        );

        assert_eq!(
            PortfolioCalculator::annualize_volatility(1.0, None, DataFrequency::Daily).round(),
            16.0
        );
        assert_eq!(
            PortfolioCalculator::annualize_volatility(5.0, None, DataFrequency::Monthly).round(),
            17.0
        );
        assert_eq!(
            PortfolioCalculator::annualize_volatility(27.0, None, DataFrequency::Yearly).round(),
            27.0
        );
    }
}
