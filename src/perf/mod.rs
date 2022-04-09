use crate::series::TimeSeries;

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

    pub fn update(&mut self, value: f64) {
        self.portfolio_value.append(None, value);
    }

    pub fn new() -> Self {
        PortfolioPerformance {
            portfolio_value: TimeSeries::new::<f64>(None, Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::broker::CashManager;
    use crate::broker::Quote;
    use crate::data::DataSource;
    use crate::perf::PortfolioPerformance;
    use crate::portfolio::Portfolio;
    use crate::portfolio::PortfolioStats;
    use crate::sim::broker::SimulatedBroker;
    use crate::sim::portfolio::SimPortfolio;

    use super::DataFrequency;
    use super::PortfolioCalculator;


    fn setup() -> SimulatedBroker {
        let mut raw_data: HashMap<i64, Vec<Quote>> = HashMap::new();

        let quote = Quote {
            symbol: String::from("ABC"),
            date: 100,
            bid: 101.0,
            ask: 102.0,
        };

        let quote1 = Quote {
            symbol: String::from("ABC"),
            date: 101,
            bid: 102.0,
            ask: 103.0,
        };

        let quote2 = Quote {
            symbol: String::from("BCD"),
            date: 100,
            bid: 501.0,
            ask: 502.0,
        };

        let quote3 = Quote {
            symbol: String::from("BCD"),
            date: 101,
            bid: 503.0,
            ask: 504.0,
        };

        raw_data.insert(100, vec![quote, quote2]);
        raw_data.insert(101, vec![quote1, quote3]);

        let source = DataSource::from_hashmap(raw_data);
        let sb = SimulatedBroker::new(source);
        sb
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

    #[test]
    fn test_that_portfolio_calculates_performance_accurately() {
        let mut perf = PortfolioPerformance::new();

        let mut brkr = setup();
        brkr.deposit_cash(100_000.00);

        let mut target_weights: HashMap<String, f64> = HashMap::new();
        target_weights.insert(String::from("ABC"), 0.5);
        target_weights.insert(String::from("BCD"), 0.5);

        let mut port = SimPortfolio::new(brkr);

        port.set_date(&100);
        let orders = port.update_weights(&target_weights);
        port.execute_orders(orders);
        perf.update(port.get_total_value());

        port.set_date(&101);
        let orders = port.update_weights(&target_weights);
        port.execute_orders(orders);
        perf.update(port.get_total_value());

        let portfolio_return = perf.get_portfolio_return();
        //We need to round up to cmp properly
        let to_comp = (portfolio_return * 100.0).round() as i64;
        assert!((to_comp as f64).eq(&69.0));
    }

}
