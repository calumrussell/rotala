use itertools::Itertools;

use crate::broker::{CashManager, PositionInfo};
use crate::portfolio::PortfolioStats;

struct PortfolioSnapshot {
    value: f64,
}

pub struct PortfolioCalculator;

impl PortfolioCalculator {
    fn get_returns(values: Vec<f64>) -> Vec<f64> {
        let mut res: Vec<f64> = Vec::new();
        let mut temp = values[0];
        for i in values.iter().skip(1).into_iter() {
            res.push((i / temp) - 1.0);
            temp = i.clone();
        }
        res
    }
}

pub struct PortfolioPerformance {
    history: Vec<PortfolioSnapshot>,
}

impl PortfolioPerformance {
    fn to_values(&self) -> Vec<f64> {
        self.history
            .iter()
            .map(|snap| -> f64 { snap.value })
            .collect_vec()
    }

    pub fn get_portfolio_volatility(&self) -> f64 {
        let returns = PortfolioCalculator::get_returns(self.to_values());
        let mean_return: f64 = returns.iter().sum::<f64>() / (returns.len() as f64);
        let squared_sum: f64 =
            returns.iter().map(|ret| ret - mean_return).sum::<f64>() / (returns.len() as f64);
        squared_sum.sqrt()
    }

    pub fn get_portfolio_sharpe_ratio(&self) -> f64 {
        let vol = self.get_portfolio_volatility();
        let ret = self.get_portfolio_return();
        ret / vol
    }

    pub fn get_portfolio_max_dd(&self) -> f64 {
        let mut maxdd = 0.0;
        let mut peak = 0.0;
        let mut trough = 0.0;
        let mut t2 = 0.0;

        for t1 in &self.to_values() {
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

    pub fn get_portfolio_return(&self) -> f64 {
        let mut last = f64::INFINITY;
        let mut sum = 0.0;

        for snap in &self.history {
            if last.eq(&f64::INFINITY) {
                last = snap.value;
            }
            let pct_change = (snap.value / last) - 1.0;
            if pct_change != 0.0 {
                sum += (1.0 + pct_change).log10();
            }
        }
        (10_f64.powf(sum) - 1.0) * 100.0
    }

    pub fn update(&mut self, port: &impl PortfolioStats, brkr: &(impl PositionInfo + CashManager)) {
        let value = port.get_total_value(brkr);
        let snap = PortfolioSnapshot { value };
        self.history.push(snap);
    }

    pub fn new() -> Self {
        let history: Vec<PortfolioSnapshot> = Vec::new();
        PortfolioPerformance { history }
    }
}
