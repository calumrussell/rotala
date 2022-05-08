use rand::{thread_rng, Rng};
use rand_distr::Uniform;
use std::collections::HashMap;

use crate::perf::{PerfStruct, PortfolioPerformance};
use crate::portfolio::{Portfolio, PortfolioStats};
use crate::schedule::{LastBusinessDayTradingSchedule, TradingSchedule};
use crate::sim::portfolio::SimPortfolio;
use crate::strategy::Strategy;
use crate::universe::{DefinedUniverse, StaticUniverse};

#[derive(Clone)]
pub struct RandomStrategyRulesWithFakeDataSource {
    portfolio: SimPortfolio,
    date: i64,
    universe: StaticUniverse,
    perf: PortfolioPerformance,
}

impl Strategy for RandomStrategyRulesWithFakeDataSource {
    fn get_perf(&self) -> PerfStruct {
        self.perf.get_output()
    }

    fn set_date(&mut self, date: &i64) {
        let state = self.portfolio.set_date(date);
        self.date = *date;
        self.perf.update(&state)
    }

    fn init(&mut self, initital_cash: &u64) {
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

            let orders = self.portfolio.update_weights(&temp);
            if orders.len() > 0 {
                self.portfolio.execute_orders(orders);
            }
        }
        self.portfolio.get_total_value()
    }
}

impl RandomStrategyRulesWithFakeDataSource {
    fn fake_data_source(dist_top: &f64) -> f64 {
        let weight_dist = Uniform::new(0.01, dist_top);
        let mut rng = thread_rng();
        rng.sample(weight_dist)
    }

    pub fn new(
        portfolio: SimPortfolio,
        perf: PortfolioPerformance,
        universe: StaticUniverse,
    ) -> Self {
        RandomStrategyRulesWithFakeDataSource {
            portfolio,
            date: -1,
            universe,
            perf,
        }
    }
}

impl From<&RandomStrategyRulesWithFakeDataSource> for Box<RandomStrategyRulesWithFakeDataSource> {
    fn from(strat: &RandomStrategyRulesWithFakeDataSource) -> Self {
        let owned: RandomStrategyRulesWithFakeDataSource = strat.clone();
        Box::new(owned)
    }
}

impl From<&RandomStrategyRulesWithFakeDataSource> for Box<dyn Strategy> {
    fn from(strat: &RandomStrategyRulesWithFakeDataSource) -> Self {
        let owned: RandomStrategyRulesWithFakeDataSource = strat.clone();
        Box::new(owned)
    }
}
