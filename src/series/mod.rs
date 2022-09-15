use itertools::Itertools;

/// Low level functions that apply over lists or matrices of numbers.
/// No state. No information relating to the specifics of backtests should
/// be stored here.
/// 
/// Values current hard-coded to f64 due to the prevalance of log operations.

//Initial impl of Series had state relating to backtest that results in
//confusing code relating to periodicity that repeated close elsewhere
//in the package. Minimal state, just operations on lists.
pub struct Series;

impl Series {

    pub fn pct_change_log(values: &Vec<f64>) -> Vec<f64> {
        let mut res: Vec<f64> = Vec::new();
        let mut temp = values[0];
        for i in values.iter().skip(1) {
            let pct_change = *i / temp;
            res.push(pct_change.ln());
            temp = *i
        }
        res
    }

    pub fn pct_change(values: &Vec<f64>) -> Vec<f64> {
        let mut res: Vec<f64> = Vec::new();
        let mut temp = values[0];
        for i in values.iter().skip(1) {
            res.push((i / temp) - 1.0);
            temp = *i;
        }
        res
    }

    pub fn maxdd(values: &Vec<f64>) -> f64 {
        let mut maxdd = 0.0;
        let mut peak = 0.0;
        let mut trough = 0.0;
        let mut t2;

        for t1 in values {
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

    pub fn var(values: &Vec<f64>) -> f64 {
        let count = values.len();
        let mean: f64 = values.iter().sum::<f64>() / (count as f64);
        let squared_diffs = values
            .iter()
            .map(|ret| ret - mean)
            .map(|diff| diff.powf(2.0))
            .collect_vec();
        let sum_of_diff = squared_diffs.iter().sum::<f64>();
        sum_of_diff / (count as f64)
    }

    pub fn vol(values: &Vec<f64>) -> f64 {
        //Accepts returns not raw portfolio values
        Series::var(values).sqrt()
    }

} 

#[cfg(test)]
mod tests {
    use super::Series;

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
        let ts = setup();
        let rets = Series::pct_change(&ts);
        let sum = rets.iter().map(|v| (1.0 + v).log10()).sum::<f64>();
        let val = (10_f64.powf(sum) - 1.0) * 100.0;
        assert_eq!(val.round(), -10.0)
    }

    #[test]
    fn test_that_vol_calculates_correctly() {
        let ts = setup();
        let rets = Series::pct_change(&ts);
        assert_eq!((Series::vol(&rets) * 100.0).round(), 19.0);

        let log_rets = Series::pct_change_log(&ts);
        assert_eq!((Series::vol(&log_rets) * 100.0).round(), 22.0);
    }

    #[test]
    fn test_that_mdd_calculates_correctly() {
        let ts = setup();
        let mdd = Series::maxdd(&ts);
        assert_eq!((mdd * 100.0).round(), -33.0);
    }
}
