use crate::types::{Frequency, StrategySnapshot};
use itertools::Itertools;

///Performance output from a single backtest run.
#[derive(Clone, Debug)]
pub struct BacktestOutput {
    pub ret: f64,
    pub cagr: f64,
    pub vol: f64,
    pub mdd: f64,
    pub sharpe: f64,
    pub values: Vec<f64>,
    pub returns: Vec<f64>,
    pub dates: Vec<i64>,
    pub cash_flows: Vec<f64>,
    pub first_date: i64,
    pub last_date: i64,
    pub dd_start_date: i64,
    pub dd_end_date: i64,
    pub best_return: f64,
    pub worst_return: f64,
    pub frequency: String,
}

struct CalculationAlgos;

impl CalculationAlgos {
    ///Returns a tuple containing (max drawdown, position of drawdown start, end position)
    fn maxdd(values: &Vec<f64>) -> (f64, usize, usize) {
        let mut maxdd = 0.0;
        let mut peak = 0.0;
        let mut peak_pos: usize = 0;
        let mut trough = 0.0;
        let mut trough_pos: usize = 0;
        let mut t2;
        let mut pos: usize = 0;
        for t1 in values {
            if t1 > &peak {
                peak = *t1;
                peak_pos = pos;
                trough = peak;
                trough_pos = peak_pos;
            } else if t1 < &trough {
                trough = *t1;
                trough_pos = pos;
                t2 = (trough / peak) - 1.0;
                if t2 < maxdd {
                    maxdd = t2
                }
            }
            pos += 1;
        }
        (maxdd, peak_pos, trough_pos)
    }

    fn var(values: &Vec<f64>) -> f64 {
        let count = values.len();
        let mean: f64 = values.iter().sum::<f64>() / (count as f64);
        let squared_diffs: Vec<f64> = values
            .iter()
            .map(|ret| ret - mean)
            .map(|diff| diff.powf(2.0))
            .collect_vec();
        let sum_of_diff = squared_diffs.iter().sum::<f64>();
        sum_of_diff / (count as f64)
    }

    fn vol(values: &Vec<f64>) -> f64 {
        //Accepts returns not raw portfolio values
        CalculationAlgos::var(values).sqrt()
    }
}

/// A set of calculations that relate to portfolios. For example, compounded annual
/// growth rate. These calculations depend on the underlying representation of the data,
/// such as asset class, so they are a higher-level than `Series` calculations.
/// Calculations are intentionally stateless as it is up to the client to decide when
/// the calculations are performed, and where the data for those calcs is stored.
pub struct PortfolioCalculations;

impl PortfolioCalculations {
    fn annualize_returns(ret: f64, periods: i32, frequency: &Frequency) -> f64 {
        match frequency {
            Frequency::Daily => ((1.0 + ret).powf(365_f64 / periods as f64)) - 1.0,
            Frequency::Monthly => ((1.0 + ret).powf(1.0 / (periods as f64 / 12_f64))) - 1.0,
            Frequency::Yearly => ((1.0 + ret).powf(1.0 / (periods as f64 / 1.0))) - 1.0,
            Frequency::Second => panic!("No performance stats by second"),
        }
    }

    fn annualize_volatility(vol: f64, frequency: &Frequency) -> f64 {
        match frequency {
            Frequency::Daily => (vol) * (252_f64).sqrt(),
            Frequency::Monthly => (vol) * (12_f64).sqrt(),
            Frequency::Yearly => vol,
            Frequency::Second => panic!("No performance stats by second"),
        }
    }

    fn get_vol(rets: &Vec<f64>, freq: &Frequency) -> f64 {
        let vol = CalculationAlgos::vol(rets);
        PortfolioCalculations::annualize_volatility(vol, freq)
    }

    fn get_sharpe(rets: &Vec<f64>, log_rets: &[f64], days: i32, freq: &Frequency) -> f64 {
        let vol = PortfolioCalculations::get_vol(rets, freq);
        let ret = PortfolioCalculations::get_cagr(log_rets, days, freq);
        if vol == 0.0 {
            if ret != 0.0 {
                return ret;
            } else {
                return 0.0;
            }
        }
        ret / vol
    }

    fn get_maxdd(rets: &Vec<f64>) -> (f64, usize, usize) {
        //Adds N to the runtime, can run faster but it isn't worth the time atm
        let mut values_with_cashflows = vec![100_000.0];
        for i in rets {
            //Because we add one value on creation, we can always unwrap safely
            let last_value = values_with_cashflows.last().unwrap();
            let new_value = last_value * (1.0 + i);
            values_with_cashflows.push(new_value);
        }
        CalculationAlgos::maxdd(&values_with_cashflows)
    }

    fn get_cagr(log_rets: &[f64], days: i32, freq: &Frequency) -> f64 {
        let ret = PortfolioCalculations::get_portfolio_return(log_rets);
        PortfolioCalculations::annualize_returns(ret, days, freq)
    }

    fn get_portfolio_return(log_rets: &[f64]) -> f64 {
        let sum_log_rets: f64 = log_rets.iter().sum();
        sum_log_rets.exp() - 1.0
    }

    fn get_returns(
        portfolio_values: &Vec<f64>,
        cash_flows: &[f64],
        inflation: &[f64],
        is_log: bool,
    ) -> Vec<f64> {
        let count = portfolio_values.len();
        let mut rets: Vec<f64> = Vec::new();

        if count.ne(&0) {
            for i in 1..count {
                let end = portfolio_values.get(i).unwrap();
                let start = portfolio_values.get(i - 1).unwrap();

                let cash_flow = cash_flows.get(i).unwrap();

                let gain = end - (start + *cash_flow);
                let capital = start + *cash_flow;

                let inflation_value = inflation.get(i).unwrap();

                let ret: f64 = if capital == 0.0 {
                    0.0
                } else {
                    ((1.0 + (gain / capital)) / (1.0 + *inflation_value)) - 1.0
                };

                if is_log {
                    let log_ret = (1.0 + ret).ln();
                    rets.push(log_ret);
                } else {
                    rets.push(ret);
                }
            }
        }
        rets
    }
}

///Stateless calculations of performance statistics from [Vec<StrategySnapshot>]. Runs seperately
///after the simulation is completed.
#[derive(Debug, Clone)]
pub struct PerformanceCalculator;

impl PerformanceCalculator {
    pub fn calculate(freq: Frequency, states: Vec<StrategySnapshot>) -> BacktestOutput {
        //Cash flow on [StrategySnapshot] is the sum of cash flows to that date, so we need to
        //calculate the difference in cash flows at each stage.
        let mut cash_flows: Vec<f64> = Vec::new();
        let mut dates: Vec<i64> = Vec::new();
        let mut total_values: Vec<f64> = Vec::new();
        cash_flows.push(0.0);

        for i in 0..states.len() {
            dates.push(*states.get(i).unwrap().date);
            total_values.push(*states.get(i).unwrap().portfolio_value);
            if i != 0 {
                let last = *states.get(i - 1).unwrap().net_cash_flow.clone();
                let curr = *states.get(i).unwrap().net_cash_flow.clone();
                let diff = curr - last;
                cash_flows.push(diff)
            }
        }

        let inflation: Vec<f64> = states.iter().map(|v| v.inflation).collect();

        let returns =
            PortfolioCalculations::get_returns(&total_values, &cash_flows, &inflation, false);

        let log_returns =
            PortfolioCalculations::get_returns(&total_values, &cash_flows, &inflation, true);

        let (mdd, drawdown_start_pos, drawdown_end_pos) =
            PortfolioCalculations::get_maxdd(&returns);
        //This can error but shouldn't because we are querying into the same-length array
        let dd_start_date = dates[drawdown_start_pos];
        let dd_end_date = dates[drawdown_end_pos];

        let best_return = *returns
            .iter()
            .max_by(|x, y| x.partial_cmp(y).unwrap())
            .unwrap();
        let worst_return = *returns
            .iter()
            .min_by(|x, y| x.partial_cmp(y).unwrap())
            .unwrap();

        BacktestOutput {
            ret: PortfolioCalculations::get_portfolio_return(&log_returns),
            cagr: PortfolioCalculations::get_cagr(&log_returns, dates.len() as i32, &freq),
            vol: PortfolioCalculations::get_vol(&returns, &freq),
            mdd,
            sharpe: PortfolioCalculations::get_sharpe(
                &returns,
                &log_returns,
                dates.len() as i32,
                &freq,
            ),
            values: total_values.clone(),
            returns,
            dates: dates.clone(),
            cash_flows,
            first_date: *dates.first().unwrap(),
            last_date: *dates.last().unwrap(),
            dd_start_date,
            dd_end_date,
            best_return,
            worst_return,
            frequency: freq.to_str(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::rc::Rc;

    use crate::broker::{BrokerCost, Quote};
    use crate::clock::{Clock, ClockBuilder};
    use crate::exchange::DefaultExchangeBuilder;
    use crate::input::{HashMapInput, HashMapInputBuilder};
    use crate::perf::StrategySnapshot;
    use crate::sim::{SimulatedBroker, SimulatedBrokerBuilder};
    use crate::strategy::{History, StaticWeightStrategyBuilder, Strategy};
    use crate::types::{DateTime, PortfolioAllocation};

    use super::Frequency;
    use super::PerformanceCalculator;
    use super::PortfolioCalculations;

    fn setup() -> (SimulatedBroker<HashMapInput>, Clock) {
        let mut raw_data: HashMap<DateTime, Vec<Quote>> = HashMap::new();

        let quote_a1 = Quote::new(101.0, 102.0, 100, "ABC");
        let quote_a2 = Quote::new(102.0, 103.0, 101, "ABC");
        let quote_a3 = Quote::new(97.0, 98.0, 102, "ABC");
        let quote_a4 = Quote::new(105.0, 106.0, 103, "ABC");

        let quote_b1 = Quote::new(501.0, 502.0, 100, "BCD");
        let quote_b2 = Quote::new(503.0, 504.0, 101, "BCD");
        let quote_b3 = Quote::new(498.0, 499.0, 102, "BCD");
        let quote_b4 = Quote::new(495.0, 496.0, 103, "BCD");

        raw_data.insert(100.into(), vec![quote_a1, quote_b1]);
        raw_data.insert(101.into(), vec![quote_a2, quote_b2]);
        raw_data.insert(102.into(), vec![quote_a3, quote_b3]);
        raw_data.insert(103.into(), vec![quote_a4, quote_b4]);

        let clock = ClockBuilder::with_length_in_dates(100, 103)
            .with_frequency(&Frequency::Second)
            .build();

        let source = HashMapInputBuilder::new()
            .with_quotes(raw_data)
            .with_clock(Rc::clone(&clock))
            .build();

        let exchange = DefaultExchangeBuilder::new()
            .with_data_source(source.clone())
            .with_clock(Rc::clone(&clock))
            .build();

        let sb = SimulatedBrokerBuilder::new()
            .with_data(source)
            .with_exchange(exchange)
            .with_trade_costs(vec![BrokerCost::Flat(0.0.into())])
            .build();
        (sb, clock)
    }

    #[test]
    fn test_that_annualizations_calculate_correctly() {
        assert_eq!(
            (PortfolioCalculations::annualize_returns(0.29, 252, &Frequency::Daily) * 100.0)
                .round(),
            45.0
        );
        assert_eq!(
            (PortfolioCalculations::annualize_returns(0.10, 4, &Frequency::Monthly) * 100.0)
                .round(),
            33.0
        );
        assert_eq!(
            (PortfolioCalculations::annualize_returns(0.30, 3, &Frequency::Yearly) * 100.0).round(),
            9.0
        );
        assert_eq!(
            (PortfolioCalculations::annualize_returns(0.05, 126, &Frequency::Daily) * 100.0)
                .round(),
            15.0
        );

        assert_eq!(
            (PortfolioCalculations::annualize_volatility(0.01, &Frequency::Daily) * 100.0).round(),
            16.0
        );
        assert_eq!(
            (PortfolioCalculations::annualize_volatility(0.05, &Frequency::Monthly) * 100.0)
                .round(),
            17.0
        );
        assert_eq!(
            (PortfolioCalculations::annualize_volatility(0.27, &Frequency::Yearly) * 100.0).round(),
            27.0
        );
    }

    #[test]
    fn test_that_portfolio_calculates_performance_accurately() {
        let (brkr, clock) = setup();
        //We use less than 100% because some bugs become possible when you are allocating the full
        //portfolio which perturb the order of operations leading to different perf outputs.
        let mut target_weights = PortfolioAllocation::new();
        target_weights.insert("ABC", 0.4);
        target_weights.insert("BCD", 0.4);

        let mut strat = StaticWeightStrategyBuilder::new()
            .with_brkr(brkr)
            .with_weights(target_weights)
            .with_clock(Rc::clone(&clock))
            .default();

        strat.init(&100_000.0);

        clock.borrow_mut().tick();
        strat.update();

        clock.borrow_mut().tick();
        strat.update();

        clock.borrow_mut().tick();
        strat.update();

        let output = strat.get_history();
        println!("{:?}", output);
        let perf = PerformanceCalculator::calculate(Frequency::Daily, output);

        let portfolio_return = perf.ret;
        //We need to round up to cmp properly
        let to_comp = (portfolio_return * 1000.0).round();
        println!("{:?}", to_comp);
        assert_eq!(to_comp, 5.0);
    }

    #[test]
    fn test_that_returns_with_cash_flow_correct() {
        //Each period has a 10% return starting from the last period value + the value of the cash
        //flow. Adding the cash flow at the start is the most conservative calculation and should
        //reflect how operations are ordered in the client.

        let snap0 = StrategySnapshot {
            date: 100.into(),
            portfolio_value: 100.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0.into(),
        };
        let snap1 = StrategySnapshot {
            date: 101.into(),
            portfolio_value: 121.0.into(),
            net_cash_flow: 10.0.into(),
            inflation: 0.0.into(),
        };
        let snap2 = StrategySnapshot {
            date: 102.into(),
            portfolio_value: 126.9.into(),
            net_cash_flow: 30.0.into(),
            inflation: 0.0.into(),
        };
        let snap3 = StrategySnapshot {
            date: 103.into(),
            portfolio_value: 150.59.into(),
            net_cash_flow: 40.0.into(),
            inflation: 0.0.into(),
        };
        let with_cash_flows = vec![snap0, snap1, snap2, snap3];

        let snap3 = StrategySnapshot {
            date: 100.into(),
            portfolio_value: 100.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0.into(),
        };
        let snap4 = StrategySnapshot {
            date: 101.into(),
            portfolio_value: 110.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0.into(),
        };
        let snap5 = StrategySnapshot {
            date: 102.into(),
            portfolio_value: 99.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0.into(),
        };
        let snap6 = StrategySnapshot {
            date: 103.into(),
            portfolio_value: 108.9.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0.into(),
        };
        let without_cash_flows = vec![snap3, snap4, snap5, snap6];

        let perf0 = PerformanceCalculator::calculate(Frequency::Yearly, with_cash_flows);
        let perf1 = PerformanceCalculator::calculate(Frequency::Yearly, without_cash_flows);

        let ret0 = f64::round(perf0.ret * 100.0);
        let ret1 = f64::round(perf1.ret * 100.0);

        println!("{:?}", perf0);
        println!("{:?}", perf1);
        assert_eq!(ret0, ret1);
    }

    #[test]
    fn test_that_returns_with_inflation_correct() {
        let snap1 = StrategySnapshot {
            date: 100.into(),
            portfolio_value: 100.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0.into(),
        };
        let snap2 = StrategySnapshot {
            date: 101.into(),
            portfolio_value: 110.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.10.into(),
        };
        let snap3 = StrategySnapshot {
            date: 102.into(),
            portfolio_value: 121.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.10.into(),
        };

        let with_inflation = vec![snap1, snap2, snap3];

        let perf = PerformanceCalculator::calculate(Frequency::Yearly, with_inflation);

        dbg!(&perf.returns);
        assert!(perf.returns == vec![0.0, 0.0])
    }

    #[test]
    fn test_that_perf_completes_with_zeros() {
        let snap1 = StrategySnapshot {
            date: 100.into(),
            portfolio_value: 0.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0.into(),
        };
        let snap2 = StrategySnapshot {
            date: 101.into(),
            portfolio_value: 0.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0.into(),
        };
        let snap3 = StrategySnapshot {
            date: 102.into(),
            portfolio_value: 0.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0.into(),
        };

        let with_zeros = vec![snap1, snap2, snap3];

        let perf = PerformanceCalculator::calculate(Frequency::Yearly, with_zeros);

        dbg!(&perf.returns);
        assert!(perf.returns == vec![0.0, 0.0])
    }

    #[test]
    fn test_that_perf_orders_best_and_worst() {
        let snap1 = StrategySnapshot {
            date: 100.into(),
            portfolio_value: 110.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0.into(),
        };
        let snap2 = StrategySnapshot {
            date: 101.into(),
            portfolio_value: 90.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0.into(),
        };
        let snap3 = StrategySnapshot {
            date: 102.into(),
            portfolio_value: 110.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0.into(),
        };

        let snaps = vec![snap1, snap2, snap3];

        let perf = PerformanceCalculator::calculate(Frequency::Yearly, snaps);
        assert!(perf.best_return > perf.worst_return);
    }
}
