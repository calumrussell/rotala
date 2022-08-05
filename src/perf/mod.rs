use crate::series::TimeSeries;
use crate::types::{CashValue, DateTime};

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

#[derive(Clone, Debug)]
pub struct StrategySnapshot {
    pub date: DateTime,
    pub value: CashValue,
    pub net_cash_flow: CashValue,
}

///Tracks the performance of a strategy and calculates simple performance statistics.
///
///Should be owned by a `Strategy` with the `Strategy` controlling and defining the update cycle.
///When the `Strategy` creates the Performance object though, the frequency has to be set for
///calculation which the update cycle must match.
///
///All of the calculation functions are private. Clients retrieve performance data through the
///`SimContext` which then queries `Strategy` which then queries this struct. The reasons for this
///are explained in the docs for `SimContext` but, essentially, this structure allows less
///error-prone initializations at the cost of more indirection.
#[derive(Clone)]
pub struct StrategyPerformance {
    values: TimeSeries,
    states: Vec<StrategySnapshot>,
    freq: DataFrequency,
    cash_flow: Vec<CashValue>,
}

#[derive(Clone)]
pub struct PerfStruct {
    pub ret: f64,
    pub cagr: f64,
    pub vol: f64,
    pub mdd: f64,
    pub sharpe: f64,
    pub values: Vec<f64>,
    pub returns: Vec<f64>,
    pub dates: Vec<f64>,
}

impl StrategyPerformance {
    pub fn get_output(&self) -> PerfStruct {
        PerfStruct {
            ret: self.get_ret(),
            cagr: self.get_cagr(),
            vol: self.get_vol(),
            mdd: self.get_maxdd(),
            sharpe: self.get_sharpe(),
            values: self.get_values(),
            returns: self.get_returns(None),
            dates: self.values.get_index(),
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

                let gain = end - start - f64::from(*cash_flow);
                let capital = start + f64::from(*cash_flow);
                let ret = gain / capital;
                if is_log.is_some() && is_log.unwrap() {
                    let log_ret = (1.0 + ret).ln();
                    rets.push(log_ret);
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
        let sum_log_rets: f64 = log_returns.iter().sum();
        sum_log_rets.exp() - 1.0
    }

    pub fn update(&mut self, state: &StrategySnapshot) {
        let mut last_net_cash_flow = CashValue::default();
        if !self.states.is_empty() {
            let prev: &StrategySnapshot = self.states.last().unwrap();
            last_net_cash_flow += prev.net_cash_flow;
        }

        //Adding portfolio value
        self.values
            .append(Some(i64::from(state.date) as f64), state.value.into());

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
        StrategyPerformance {
            values: TimeSeries::new::<f64>(None, Vec::new()),
            states: Vec::new(),
            freq: DataFrequency::Yearly,
            cash_flow: Vec::new(),
        }
    }

    pub fn monthly() -> Self {
        StrategyPerformance {
            values: TimeSeries::new::<f64>(None, Vec::new()),
            states: Vec::new(),
            freq: DataFrequency::Monthly,
            cash_flow: Vec::new(),
        }
    }

    pub fn daily() -> Self {
        StrategyPerformance {
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
    use std::rc::Rc;

    use crate::broker::{BrokerCost, Quote};
    use crate::clock::{Clock, ClockBuilder};
    use crate::input::{HashMapInput, HashMapInputBuilder};
    use crate::perf::StrategySnapshot;
    use crate::sim::broker::{SimulatedBroker, SimulatedBrokerBuilder};
    use crate::strategy::{StaticWeightStrategyBuilder, Strategy, TransferTo};
    use crate::types::{DateTime, PortfolioAllocation};

    use super::DataFrequency;
    use super::PortfolioCalculator;
    use super::StrategyPerformance;

    fn setup() -> (SimulatedBroker<HashMapInput>, Clock) {
        let mut raw_data: HashMap<DateTime, Vec<Quote>> = HashMap::new();

        let quote_a1 = Quote {
            symbol: String::from("ABC"),
            date: 100.into(),
            bid: 101.0.into(),
            ask: 102.0.into(),
        };

        let quote_a2 = Quote {
            symbol: String::from("ABC"),
            date: 101.into(),
            bid: 102.0.into(),
            ask: 103.0.into(),
        };

        let quote_a3 = Quote {
            symbol: String::from("ABC"),
            date: 102.into(),
            bid: 97.0.into(),
            ask: 98.0.into(),
        };

        let quote_a4 = Quote {
            symbol: String::from("ABC"),
            date: 103.into(),
            bid: 105.0.into(),
            ask: 106.0.into(),
        };

        let quote_b1 = Quote {
            symbol: String::from("BCD"),
            date: 100.into(),
            bid: 501.0.into(),
            ask: 502.0.into(),
        };

        let quote_b2 = Quote {
            symbol: String::from("BCD"),
            date: 101.into(),
            bid: 503.0.into(),
            ask: 504.0.into(),
        };

        let quote_b3 = Quote {
            symbol: String::from("BCD"),
            date: 102.into(),
            bid: 498.0.into(),
            ask: 499.0.into(),
        };

        let quote_b4 = Quote {
            symbol: String::from("BCD"),
            date: 103.into(),
            bid: 495.0.into(),
            ask: 496.0.into(),
        };

        raw_data.insert(100.into(), vec![quote_a1, quote_b1]);
        raw_data.insert(101.into(), vec![quote_a2, quote_b2]);
        raw_data.insert(102.into(), vec![quote_a3, quote_b3]);
        raw_data.insert(103.into(), vec![quote_a4, quote_b4]);

        let clock = ClockBuilder::from_fixed(100.into(), 103.into()).every_second();

        let source = HashMapInputBuilder::new()
            .with_quotes(raw_data)
            .with_clock(Rc::clone(&clock))
            .build();

        let sb = SimulatedBrokerBuilder::new()
            .with_data(source)
            .with_trade_costs(vec![BrokerCost::Flat(0.0.into())])
            .build();
        (sb, clock)
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
        let (brkr, clock) = setup();
        let mut target_weights = PortfolioAllocation::new();
        target_weights.insert(&String::from("ABC"), &0.5.into());
        target_weights.insert(&String::from("BCD"), &0.5.into());

        let mut strat = StaticWeightStrategyBuilder::new()
            .with_brkr(brkr)
            .with_weights(target_weights)
            .with_clock(Rc::clone(&clock))
            .yearly();

        strat.deposit_cash(&100_000.0.into());
        clock.borrow_mut().tick();
        strat.update();

        clock.borrow_mut().tick();
        strat.update();

        clock.borrow_mut().tick();
        strat.update();

        let output = strat.get_perf();

        let portfolio_return = output.ret;
        //We need to round up to cmp properly
        let to_comp = (portfolio_return * 1000.0).round();
        println!("{:?}", to_comp);
        assert!((to_comp).eq(&7.0));
    }

    #[test]
    fn test_that_returns_with_cash_flow_correct() {
        //Each period has a 10% return starting from the last period value + the value of the cash
        //flow. Adding the cash flow at the start is the most conservative calculation and should
        //reflect how operations are ordered in the client.

        let mut perf = StrategyPerformance::yearly();
        let snap0 = StrategySnapshot {
            date: 100.into(),
            value: 100.0.into(),
            net_cash_flow: 0.0.into(),
        };
        let snap1 = StrategySnapshot {
            date: 101.into(),
            value: 121.0.into(),
            net_cash_flow: 10.0.into(),
        };
        let snap2 = StrategySnapshot {
            date: 102.into(),
            value: 144.1.into(),
            net_cash_flow: 20.0.into(),
        };
        perf.update(&snap0);
        perf.update(&snap1);
        perf.update(&snap2);

        let mut perf1 = StrategyPerformance::yearly();
        let snap3 = StrategySnapshot {
            date: 100.into(),
            value: 100.0.into(),
            net_cash_flow: 0.0.into(),
        };
        let snap4 = StrategySnapshot {
            date: 101.into(),
            value: 110.0.into(),
            net_cash_flow: 0.0.into(),
        };
        let snap5 = StrategySnapshot {
            date: 102.into(),
            value: 121.0.into(),
            net_cash_flow: 0.0.into(),
        };
        perf1.update(&snap3);
        perf1.update(&snap4);
        perf1.update(&snap5);

        let rets = f64::round(perf.get_ret() * 100.0);
        let rets1 = f64::round(perf1.get_ret() * 100.0);
        println!("{:?}", rets);
        println!("{:?}", rets1);
        assert!(rets == rets1);
    }
}
