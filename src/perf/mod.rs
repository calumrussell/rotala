use itertools::Itertools;

use crate::broker::{CashManager, PositionInfo};
use crate::data::TimeSeries;
use crate::portfolio::PortfolioStats;

enum DataFrequency {
    Daily,
    Monthly,
    Yearly,
}

pub struct PortfolioCalculator;

impl PortfolioCalculator {
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
    portfolio_value: TimeSeries,
}

impl PortfolioPerformance {
    pub fn get_portfolio_volatility(&mut self) -> f64 {
        let rets = TimeSeries::new(None, self.portfolio_value.pct_change());
        rets.vol()
    }

    pub fn get_portfolio_sharpe_ratio(&mut self) -> f64 {
        let vol = self.get_portfolio_volatility();
        let ret = self.get_portfolio_return();
        ret / vol
    }

    pub fn get_portfolio_max_dd(&mut self) -> f64 {
        self.portfolio_value.maxdd()
    }

    pub fn get_portfolio_return(&mut self) -> f64 {
        let sum_log_rets = self.portfolio_value.pct_change_log().iter().sum();
        (10_f64.powf(sum_log_rets) - 1.0) * 100.0
    }

    pub fn update(&mut self, port: &impl PortfolioStats, brkr: &(impl PositionInfo + CashManager)) {
        let value = port.get_total_value(brkr);
        self.portfolio_value.append(None, value);
    }

    pub fn new() -> Self {
        PortfolioPerformance {
            portfolio_value: TimeSeries::new(None, Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DataFrequency;
    use super::PortfolioCalculator;

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
