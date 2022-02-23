
/* A Strategy wraps around the broker and portfolio, the idea 
is to move most of the functionality into a trading strategy
and organize calls to the rest of the system through that.

One key point is that the Strategy should only be aware of an
overall portfolio, and not aware of how the portfolio executes
changes with the broker.

The components should be fully owned by the strategy and
are immutable.
*/

use std::collections::{HashMap};
use rand::{thread_rng, Rng};
use rand_distr::Uniform;

use crate::portfolio::{SimPortfolio, Portfolio, PortfolioStats};
use crate::trading::{LastBusinessDayTradingSchedule, DefaultTradingSchedule, TradingSchedule};
use crate::data::universe::{StaticUniverse, DefinedUniverse};

pub trait Strategy {
    fn run(&mut self) -> f64;
    fn set_date(&mut self, date: &i64);
    fn init(&mut self, initial_cash: &f64);
}

pub struct FixedWeightStrategy {
    portfolio: SimPortfolio,
    date: i64,
    universe: StaticUniverse,
    target_weights: HashMap<String, f64>,
}

impl Strategy for FixedWeightStrategy {
    fn set_date(&mut self, date: &i64) {
        self.portfolio.set_date(date);
        self.date = *date;
    }

    fn init(&mut self, initital_cash: &f64) {
        self.portfolio.deposit_cash(initital_cash);
    }

    fn run(&mut self) -> f64 {
        if DefaultTradingSchedule::should_trade(&self.date) {
            let orders = self.portfolio.update_weights(&self.target_weights, &self.universe);
            if orders.len() > 0 {
                self.portfolio.execute_orders(orders);
            }
        }
        self.portfolio.get_total_value(&self.universe)
    }
}

impl FixedWeightStrategy{
    pub fn new(portfolio: SimPortfolio, universe: StaticUniverse, target_weights: HashMap<String, f64>) -> Self {
        FixedWeightStrategy {
            portfolio,
            date: -1,
            universe,
            target_weights
        }
    }
}

pub struct StaticWeightStrategyRulesMonthlyRebalancing{
    portfolio: SimPortfolio,
    date: i64,
    universe: StaticUniverse,
    target_weights: Vec<HashMap<String, f64>>,
    count: usize,
}

impl Strategy for StaticWeightStrategyRulesMonthlyRebalancing{
    fn set_date(&mut self, date: &i64) {
        self.portfolio.set_date(date);
        self.date = *date;
    }

    fn init(&mut self, initital_cash: &f64) {
        self.portfolio.deposit_cash(initital_cash);
    }

    fn run(&mut self) -> f64 {
        if LastBusinessDayTradingSchedule::should_trade(&self.date) {
            let weights = &self.target_weights[self.count];
            self.count += 1;
            let orders = self.portfolio.update_weights(weights, &self.universe);
 
            if orders.len() > 0 {
                self.portfolio.execute_orders(orders);
            }
        }
        self.portfolio.get_total_value(&self.universe)
    }
}

impl StaticWeightStrategyRulesMonthlyRebalancing{
    pub fn new(portfolio: SimPortfolio, universe: StaticUniverse, target_weights: Vec<HashMap<String, f64>>) -> Self {
        StaticWeightStrategyRulesMonthlyRebalancing {
            portfolio,
            date: -1,
            universe,
            target_weights,
            count: 0
        }
    }
}

pub struct RandomStrategyRulesWithFakeDataSource{
    portfolio: SimPortfolio,
    date: i64,
    universe: StaticUniverse,
}

impl Strategy for RandomStrategyRulesWithFakeDataSource{
    fn set_date(&mut self, date: &i64) {
        self.portfolio.set_date(date);
        self.date = *date;
    }

    fn init(&mut self, initital_cash: &f64) {
        self.portfolio.deposit_cash(initital_cash);
    }

    fn run(&mut self) -> f64 {
        if LastBusinessDayTradingSchedule::should_trade(&self.date) {
            let mut initial = 0.99;

            let mut temp: HashMap<String, f64> = HashMap::new();
            for asset in self.universe.get_assets() {
                let weight = RandomStrategyRulesWithFakeDataSource::fake_data_source(&initial);
                temp.insert(asset.to_owned(), weight.to_owned());
                initial += -weight
            }

            let orders = self.portfolio.update_weights(&temp, &self.universe);
            if orders.len() > 0 {
                self.portfolio.execute_orders(orders);
            }
        }
        self.portfolio.get_total_value(&self.universe)
    }
}

impl RandomStrategyRulesWithFakeDataSource {
    fn fake_data_source(dist_top: &f64) -> f64 {
        let weight_dist = Uniform::new(0.01, dist_top);
        let mut rng = thread_rng();
        rng.sample(weight_dist)
    }

    pub fn new(portfolio: SimPortfolio, universe: StaticUniverse) -> Self {
        RandomStrategyRulesWithFakeDataSource {
            portfolio,
            date: -1,
            universe
        }
    }
}

