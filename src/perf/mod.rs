use crate::{portfolio::PortfolioState, series::TimeSeries};

#[derive(Clone)]
enum DataFrequency {
    Daily,
    Monthly,
    Yearly,
}

pub struct PortfolioCalculator;

impl PortfolioCalculator {
    fn annualize_returns(ret: f64, periods: i32, frequency: &DataFrequency) -> f64 {
        match frequency {
            DataFrequency::Daily => ((1.0 + ret).powf(252_f64 / periods as f64)) - 1.0,
            DataFrequency::Monthly => ((1.0 + ret).powf(1.0 / (periods as f64 / 12_f64))) - 1.0,
            DataFrequency::Yearly => ((1.0 + ret).powf(1.0 / (periods as f64 / 1.0))) - 1.0,
        }
    }

    fn annualize_volatility(vol: f64, frequency: &DataFrequency) -> f64 {
        match frequency {
            DataFrequency::Daily => (vol) * (252_f64).sqrt(),
            DataFrequency::Monthly => (vol) * (12_f64).sqrt(),
            DataFrequency::Yearly => vol,
        }
    }
}

#[derive(Clone)]
pub struct PortfolioPerformance {
    values: TimeSeries,
    states: Vec<PortfolioState>,
    freq: DataFrequency,
    cash_flow: Vec<f64>,
}

pub struct PerfStruct {
    pub ret: f64,
    pub cagr: f64,
    pub vol: f64,
    pub mdd: f64,
    pub sharpe: f64,
    pub values: Vec<f64>,
    pub returns: Vec<f64>,
}

impl PortfolioPerformance {
    pub fn get_output(&self) -> PerfStruct {
        PerfStruct {
            ret: self.get_ret(),
            cagr: self.get_cagr(),
            vol: self.get_vol(),
            mdd: self.get_maxdd(),
            sharpe: self.get_sharpe(),
            values: self.get_values(),
            returns: self.get_returns(None),
        }
    }

    fn get_values(&self) -> Vec<f64> {
        self.values.get_values()
    }

    fn get_returns(&self, is_log: Option<bool>) -> Vec<f64> {
        let portfolio_values = self.get_values();
        let cash_flows = &self.cash_flow;
        let count = portfolio_values.len();
        let mut rets: Vec<f64> = Vec::new();

        if count.ne(&0) {
            for i in 1..count {
                let end = portfolio_values.get(i).unwrap();
                let start = portfolio_values.get(i - 1).unwrap();

                let cash_flow = cash_flows.get(i).unwrap();

                let gain = end - start - cash_flow;
                let avg_capital = start + cash_flow;
                let ret = gain / avg_capital;
                if is_log.is_some() && is_log.unwrap() {
                    rets.push((ret + 1_f64).log10());
                } else {
                    rets.push(ret);
                }
            }
        }
        rets
    }

    fn get_vol(&self) -> f64 {
        let rets = TimeSeries::new(None, self.get_returns(None));
        PortfolioCalculator::annualize_volatility(rets.vol(), &self.freq)
    }

    fn get_sharpe(&self) -> f64 {
        let vol = self.get_vol();
        let ret = self.get_ret();
        ret / vol
    }

    fn get_maxdd(&self) -> f64 {
        self.values.maxdd()
    }

    fn get_cagr(&self) -> f64 {
        let ret = self.get_ret();
        let days = self.values.count() as i32;
        PortfolioCalculator::annualize_returns(ret, days, &self.freq)
    }

    fn get_ret(&self) -> f64 {
        let log_returns = self.get_returns(Some(true));
        let sum_log_rets = log_returns.iter().sum();
        10_f64.powf(sum_log_rets) - 1.0
    }

    pub fn update(&mut self, state: &PortfolioState) {
        let mut last_net_cash_flow = 0_f64;
        if self.states.len() > 0 {
            let prev: &PortfolioState = self.states.last().unwrap();
            last_net_cash_flow += prev.net_cash_flow;
        }

        //Adding portfolio value
        self.values.append(None, state.value);

        //Copying whole portfolio state
        let copy_state = state.clone();
        self.states.push(copy_state);

        //Adding cash flow within period
        //Can calculate this from change in state but
        //this makes it explicit and saves an iteration
        let chg_cash_flow = state.net_cash_flow - last_net_cash_flow;
        self.cash_flow.push(chg_cash_flow);
    }

    pub fn yearly() -> Self {
        PortfolioPerformance {
            values: TimeSeries::new::<f64>(None, Vec::new()),
            states: Vec::new(),
            freq: DataFrequency::Yearly,
            cash_flow: Vec::new(),
        }
    }

    pub fn monthly() -> Self {
        PortfolioPerformance {
            values: TimeSeries::new::<f64>(None, Vec::new()),
            states: Vec::new(),
            freq: DataFrequency::Monthly,
            cash_flow: Vec::new(),
        }
    }

    pub fn daily() -> Self {
        PortfolioPerformance {
            values: TimeSeries::new::<f64>(None, Vec::new()),
            states: Vec::new(),
            freq: DataFrequency::Daily,
            cash_flow: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::broker::{BrokerCost, Dividend, Quote};
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
        let dividends: HashMap<i64, Vec<Dividend>> = HashMap::new();

        let quote_a1 = Quote {
            symbol: String::from("ABC"),
            date: 100,
            bid: 101.0,
            ask: 102.0,
        };

        let quote_a2 = Quote {
            symbol: String::from("ABC"),
            date: 101,
            bid: 102.0,
            ask: 103.0,
        };

        let quote_a3 = Quote {
            symbol: String::from("ABC"),
            date: 102,
            bid: 97.0,
            ask: 98.0,
        };

        let quote_a4 = Quote {
            symbol: String::from("ABC"),
            date: 103,
            bid: 105.0,
            ask: 106.0,
        };

        let quote_b1 = Quote {
            symbol: String::from("BCD"),
            date: 100,
            bid: 501.0,
            ask: 502.0,
        };

        let quote_b2 = Quote {
            symbol: String::from("BCD"),
            date: 101,
            bid: 503.0,
            ask: 504.0,
        };

        let quote_b3 = Quote {
            symbol: String::from("BCD"),
            date: 102,
            bid: 498.0,
            ask: 499.0,
        };

        let quote_b4 = Quote {
            symbol: String::from("BCD"),
            date: 103,
            bid: 495.0,
            ask: 496.0,
        };

        raw_data.insert(100, vec![quote_a1, quote_b1]);
        raw_data.insert(101, vec![quote_a2, quote_b2]);
        raw_data.insert(102, vec![quote_a3, quote_b3]);
        raw_data.insert(103, vec![quote_a4, quote_b4]);

        let source = DataSource::from_hashmap(raw_data, dividends);
        let sb = SimulatedBroker::new(source, vec![BrokerCost::Flat(1.0)]);
        sb
    }

    #[test]
    fn test_that_annualizations_calculate_correctly() {
        assert_eq!(
            (PortfolioCalculator::annualize_returns(0.29, 252, &DataFrequency::Daily) * 100.0)
                .round(),
            29.0
        );
        assert_eq!(
            (PortfolioCalculator::annualize_returns(0.10, 4, &DataFrequency::Monthly) * 100.0)
                .round(),
            33.0
        );
        assert_eq!(
            (PortfolioCalculator::annualize_returns(0.30, 3, &DataFrequency::Yearly) * 100.0)
                .round(),
            9.0
        );
        assert_eq!(
            (PortfolioCalculator::annualize_returns(0.05, 126, &DataFrequency::Daily) * 100.0)
                .round(),
            10.0
        );

        assert_eq!(
            (PortfolioCalculator::annualize_volatility(0.01, &DataFrequency::Daily) * 100.0)
                .round(),
            16.0
        );
        assert_eq!(
            (PortfolioCalculator::annualize_volatility(0.05, &DataFrequency::Monthly) * 100.0)
                .round(),
            17.0
        );
        assert_eq!(
            (PortfolioCalculator::annualize_volatility(0.27, &DataFrequency::Yearly) * 100.0)
                .round(),
            27.0
        );
    }

    #[test]
    fn test_that_portfolio_calculates_performance_accurately() {
        let mut perf = PortfolioPerformance::yearly();

        let brkr = setup();
        let mut target_weights: HashMap<String, f64> = HashMap::new();
        target_weights.insert(String::from("ABC"), 0.5);
        target_weights.insert(String::from("BCD"), 0.5);

        let mut port = SimPortfolio::new(brkr);
        port.deposit_cash(&100_000_u64);

        port.set_date(&100);
        let orders = port.update_weights(&target_weights);
        port.execute_orders(orders);
        perf.update(&port.get_current_state());

        port.set_date(&101);
        let orders = port.update_weights(&target_weights);
        port.execute_orders(orders);
        perf.update(&port.get_current_state());

        let output = perf.get_output();

        let portfolio_return = output.ret;
        //We need to round up to cmp properly
        let to_comp = (portfolio_return * 1000.0).round() as i64;
        assert!((to_comp as f64).eq(&7.0));
    }

    #[test]
    fn test_that_net_cash_flows_recorded_correctly() {
        let mut perf = PortfolioPerformance::yearly();

        let brkr = setup();
        let mut target_weights: HashMap<String, f64> = HashMap::new();
        target_weights.insert(String::from("ABC"), 0.5);
        target_weights.insert(String::from("BCD"), 0.5);

        let mut port = SimPortfolio::new(brkr);
        port.deposit_cash(&100_000_u64);

        port.set_date(&100);
        let orders = port.update_weights(&target_weights);
        port.execute_orders(orders);
        perf.update(&port.get_current_state());

        port.set_date(&101);
        port.deposit_cash(&10_000_u64);
        let orders = port.update_weights(&target_weights);
        port.execute_orders(orders);
        perf.update(&port.get_current_state());

        port.set_date(&102);
        port.withdraw_cash_with_liquidation(&20_000_u64);
        let orders = port.update_weights(&target_weights);
        port.execute_orders(orders);
        perf.update(&port.get_current_state());

        port.set_date(&103);
        port.deposit_cash(&5_000_u64);
        let orders = port.update_weights(&target_weights);
        port.execute_orders(orders);
        perf.update(&port.get_current_state());

        let cf = perf.cash_flow;

        assert!(cf.get(0).unwrap().eq(&100_000_f64));
        assert!(cf.get(1).unwrap().eq(&10_000_f64));
        assert!(cf.get(2).unwrap().eq(&-20_000_f64));
        assert!(cf.get(3).unwrap().eq(&5_000_f64));
    }

    #[test]
    fn test_that_returns_with_cash_flow_correct() {
        let mut perf = PortfolioPerformance::yearly();

        let brkr = setup();
        let mut target_weights: HashMap<String, f64> = HashMap::new();
        target_weights.insert(String::from("ABC"), 0.5);
        target_weights.insert(String::from("BCD"), 0.5);

        let mut port = SimPortfolio::new(brkr);
        port.deposit_cash(&100_000_u64);

        port.set_date(&100);
        let orders = port.update_weights(&target_weights);
        port.execute_orders(orders);
        perf.update(&port.get_current_state());

        port.set_date(&101);
        port.deposit_cash(&10_000_u64);
        let orders = port.update_weights(&target_weights);
        port.execute_orders(orders);
        perf.update(&port.get_current_state());

        port.set_date(&102);
        port.withdraw_cash(&20_000_u64);
        let orders = port.update_weights(&target_weights);
        port.execute_orders(orders);
        perf.update(&port.get_current_state());

        port.set_date(&103);
        port.deposit_cash(&5_000_u64);
        let orders = port.update_weights(&target_weights);
        port.execute_orders(orders);
        perf.update(&port.get_current_state());

        let mut perf1 = PortfolioPerformance::yearly();

        let brkr1 = setup();
        let mut target_weights1: HashMap<String, f64> = HashMap::new();
        target_weights1.insert(String::from("ABC"), 0.5);
        target_weights1.insert(String::from("BCD"), 0.5);

        let mut port1 = SimPortfolio::new(brkr1);
        port1.deposit_cash(&100_000_u64);

        port1.set_date(&100);
        let orders = port1.update_weights(&target_weights1);
        port1.execute_orders(orders);
        perf.update(&port1.get_current_state());

        port1.set_date(&101);
        let orders = port1.update_weights(&target_weights1);
        port1.execute_orders(orders);
        perf1.update(&port1.get_current_state());

        port1.set_date(&102);
        let orders = port1.update_weights(&target_weights1);
        port1.execute_orders(orders);
        perf1.update(&port1.get_current_state());

        port1.set_date(&103);
        port1.deposit_cash(&5_000_u64);
        let orders = port1.update_weights(&target_weights1);
        port1.execute_orders(orders);
        perf1.update(&port1.get_current_state());

        let rets = perf.get_returns(None);
        let rets1 = perf.get_returns(None);

        assert!(rets.eq(&rets1));
    }
}
