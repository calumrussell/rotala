//! Generates performance stats for backtest

use crate::types::StrategySnapshot;
use itertools::Itertools;
use rotala::clock::Frequency;

/// Output for single backtest run.
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

/// Group of functions common to portfolio performance calculations.
struct CalculationAlgos;

impl CalculationAlgos {
    /// Returns a tuple containing (max drawdown, position of drawdown start, end position)
    fn maxdd(values: &[f64]) -> (f64, usize, usize) {
        let mut maxdd = 0.0;
        let mut peak = 0.0;
        let mut peak_pos: usize = 0;
        let mut trough = 0.0;
        let mut trough_pos: usize = 0;
        let mut t2;
        for (pos, t1) in values.iter().enumerate() {
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
        }
        (maxdd, peak_pos, trough_pos)
    }

    fn var(values: &[f64]) -> f64 {
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

    fn vol(values: &[f64]) -> f64 {
        //Accepts returns not raw portfolio values
        CalculationAlgos::var(values).sqrt()
    }
}

pub struct PortfolioCalculations;

impl PortfolioCalculations {
    //This calculation returns the annualized return from the cumulative return given the number of
    //daily/monthly/yearly periods.
    //Most calculations of annualized returns start from a per-period, as opposed to cumulative
    //return, so the calculations will be more simple. The reason why we need to make additional
    //calculations with the exponent is because we need to convert from the cumulative return.
    fn annualize_returns(ret: f64, periods: i32, frequency: &Frequency) -> f64 {
        match frequency {
            Frequency::Daily => ((1_f64 + ret).powf(365_f64 / periods as f64)) - 1_f64,
            Frequency::Second => panic!("No performance stats by second"),
            Frequency::Fixed => panic!("No performance stats by fixed"),
        }
    }

    fn annualize_volatility(vol: f64, frequency: &Frequency) -> f64 {
        match frequency {
            Frequency::Daily => (vol) * (252_f64).sqrt(),
            Frequency::Second => panic!("No performance stats by second"),
            Frequency::Fixed => panic!("No performance stats by fixed"),
        }
    }

    fn get_vol(rets: &[f64], freq: &Frequency) -> f64 {
        let vol = CalculationAlgos::vol(rets);
        PortfolioCalculations::annualize_volatility(vol, freq)
    }

    fn get_sharpe(rets: &[f64], log_rets: &[f64], days: i32, freq: &Frequency) -> f64 {
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

    fn get_maxdd(rets: &[f64]) -> (f64, usize, usize) {
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
        portfolio_values: &[f64],
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

/// Calculates performance statistics from [`Vec<StrategySnapshot>`].
///
/// Intended to be run after the simulation is completed.
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
            frequency: freq.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rotala::clock::Clock;
    use rotala::exchange::uist_v1::UistV1;
    use rotala::http::uist::uistv1_client::TestClient;
    use rotala::http::uist::uistv1_client::UistClient;
    use rotala::input::penelope::PenelopeBuilder;

    use crate::broker::uist::UistBroker;
    use crate::broker::uist::UistBrokerBuilder;
    use crate::broker::BrokerCost;
    use crate::perf::StrategySnapshot;
    use crate::strategy::staticweight::StaticWeightStrategyBuilder;
    use crate::types::PortfolioAllocation;

    use super::Frequency;
    use super::PerformanceCalculator;
    use super::PortfolioCalculations;

    async fn setup() -> (UistBroker<TestClient>, Clock) {
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(101.0, 102.0, 100, "ABC");
        source_builder.add_quote(102.0, 103.0, 101, "ABC");
        source_builder.add_quote(97.0, 98.0, 102, "ABC");
        source_builder.add_quote(105.0, 106.0, 103, "ABC");

        source_builder.add_quote(501.0, 502.0, 100, "BCD");
        source_builder.add_quote(503.0, 504.0, 101, "BCD");
        source_builder.add_quote(498.0, 499.0, 102, "BCD");
        source_builder.add_quote(495.0, 496.0, 103, "BCD");

        let (price_source, clock) =
            source_builder.build_with_frequency(rotala::clock::Frequency::Second);
        let exchange = UistV1::new(clock.clone(), price_source, "Random");
        let mut datasets = HashMap::new();
        datasets.insert("Random".to_string(), exchange);
        let mut client = TestClient::new(&mut datasets);
        let resp = client.init("Random".to_string()).await.unwrap();

        let brkr = UistBrokerBuilder::new()
            .with_client(client, resp.backtest_id)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build()
            .await;

        (brkr, clock)
    }

    #[test]
    fn test_that_annualizations_calculate_correctly() {
        assert_eq!(
            (PortfolioCalculations::annualize_returns(0.29, 252, &Frequency::Daily) * 100.0)
                .round(),
            45.0
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
    }

    #[tokio::test]
    async fn test_that_portfolio_calculates_performance_accurately() {
        let (brkr, clock) = setup().await;
        //We use less than 100% because some bugs become possible when you are allocating the full
        //portfolio which perturb the order of operations leading to different perf outputs.
        let mut target_weights = PortfolioAllocation::new();
        target_weights.insert("ABC", 0.4);
        target_weights.insert("BCD", 0.4);

        let mut strat = StaticWeightStrategyBuilder::new()
            .with_brkr(brkr)
            .with_weights(target_weights)
            .with_clock(clock.clone())
            .default();

        strat.init(&100_000.0);

        strat.update();

        strat.update();

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
            inflation: 0.0,
        };
        let snap1 = StrategySnapshot {
            date: 101.into(),
            portfolio_value: 121.0.into(),
            net_cash_flow: 10.0.into(),
            inflation: 0.0,
        };
        let snap2 = StrategySnapshot {
            date: 102.into(),
            portfolio_value: 126.9.into(),
            net_cash_flow: 30.0.into(),
            inflation: 0.0,
        };
        let snap3 = StrategySnapshot {
            date: 103.into(),
            portfolio_value: 150.59.into(),
            net_cash_flow: 40.0.into(),
            inflation: 0.0,
        };
        let with_cash_flows = vec![snap0, snap1, snap2, snap3];

        let snap3 = StrategySnapshot {
            date: 100.into(),
            portfolio_value: 100.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0,
        };
        let snap4 = StrategySnapshot {
            date: 101.into(),
            portfolio_value: 110.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0,
        };
        let snap5 = StrategySnapshot {
            date: 102.into(),
            portfolio_value: 99.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0,
        };
        let snap6 = StrategySnapshot {
            date: 103.into(),
            portfolio_value: 108.9.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0,
        };
        let without_cash_flows = vec![snap3, snap4, snap5, snap6];

        let perf0 = PerformanceCalculator::calculate(Frequency::Daily, with_cash_flows);
        let perf1 = PerformanceCalculator::calculate(Frequency::Daily, without_cash_flows);

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
            inflation: 0.0,
        };
        let snap2 = StrategySnapshot {
            date: 101.into(),
            portfolio_value: 110.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.10,
        };
        let snap3 = StrategySnapshot {
            date: 102.into(),
            portfolio_value: 121.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.10,
        };

        let with_inflation = vec![snap1, snap2, snap3];

        let perf = PerformanceCalculator::calculate(Frequency::Daily, with_inflation);

        dbg!(&perf.returns);
        assert!(perf.returns == vec![0.0, 0.0])
    }

    #[test]
    fn test_that_perf_completes_with_zeros() {
        let snap1 = StrategySnapshot {
            date: 100.into(),
            portfolio_value: 0.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0,
        };
        let snap2 = StrategySnapshot {
            date: 101.into(),
            portfolio_value: 0.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0,
        };
        let snap3 = StrategySnapshot {
            date: 102.into(),
            portfolio_value: 0.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0,
        };

        let with_zeros = vec![snap1, snap2, snap3];

        let perf = PerformanceCalculator::calculate(Frequency::Daily, with_zeros);

        dbg!(&perf.returns);
        assert!(perf.returns == vec![0.0, 0.0])
    }

    #[test]
    fn test_that_perf_orders_best_and_worst() {
        let snap1 = StrategySnapshot {
            date: 100.into(),
            portfolio_value: 110.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0,
        };
        let snap2 = StrategySnapshot {
            date: 101.into(),
            portfolio_value: 90.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0,
        };
        let snap3 = StrategySnapshot {
            date: 102.into(),
            portfolio_value: 110.0.into(),
            net_cash_flow: 0.0.into(),
            inflation: 0.0,
        };

        let snaps = vec![snap1, snap2, snap3];

        let perf = PerformanceCalculator::calculate(Frequency::Daily, snaps);
        assert!(perf.best_return > perf.worst_return);
    }
}
