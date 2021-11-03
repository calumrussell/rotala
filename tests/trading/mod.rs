use std::collections::HashMap;
use std::rc::Rc;

use alator::data::universe::{DefinedUniverse, StaticUniverse};
use alator::trading::{DefaultTradingSchedule, TradingSchedule};
use alator::trading::{LastBusinessDayTradingSchedule, TradingSystem};
use rand::{thread_rng, Rng};
use rand_distr::Uniform;

pub struct FixedWeightTradingSystem {
    target_weights: HashMap<String, f64>,
}

impl TradingSystem for FixedWeightTradingSystem {
    fn calculate_weights(&mut self) -> HashMap<String, f64> {
        self.target_weights.clone()
    }

    fn should_trade_now(&self, date: &i64) -> bool {
        DefaultTradingSchedule::should_trade(date)
    }
}

impl FixedWeightTradingSystem {
    pub fn new(weights: HashMap<String, f64>) -> FixedWeightTradingSystem {
        FixedWeightTradingSystem {
            target_weights: weights,
        }
    }
}

pub struct MonthlyRebalancingFixedWeightTradingSystem {
    target_weights: HashMap<String, f64>,
}

impl TradingSystem for MonthlyRebalancingFixedWeightTradingSystem {
    fn calculate_weights(&mut self) -> HashMap<String, f64> {
        self.target_weights.clone()
    }

    fn should_trade_now(&self, date: &i64) -> bool {
        LastBusinessDayTradingSchedule::should_trade(date)
    }
}

impl MonthlyRebalancingFixedWeightTradingSystem {
    pub fn new(weights: HashMap<String, f64>) -> Self {
        MonthlyRebalancingFixedWeightTradingSystem {
            target_weights: weights,
        }
    }
}

pub struct MonthlyRebalancingStaticWeightTradingSystem {
    target_weights: Vec<HashMap<String, f64>>,
    count: usize,
}

impl TradingSystem for MonthlyRebalancingStaticWeightTradingSystem {
    fn calculate_weights(&mut self) -> HashMap<String, f64> {
        let weights = &self.target_weights[self.count];
        self.count += 1;
        weights.to_owned()
    }

    fn should_trade_now(&self, date: &i64) -> bool {
        LastBusinessDayTradingSchedule::should_trade(date)
    }
}

impl MonthlyRebalancingStaticWeightTradingSystem {
    pub fn new(weights: Vec<HashMap<String, f64>>) -> Self {
        MonthlyRebalancingStaticWeightTradingSystem {
            target_weights: weights,
            count: 0,
        }
    }
}

pub struct MonthlyRebalancingWithDataSourceTradingSystem {
    universe: Rc<StaticUniverse>,
}

impl TradingSystem for MonthlyRebalancingWithDataSourceTradingSystem {
    fn calculate_weights(&mut self) -> HashMap<String, f64> {
        let mut initial = 0.99;

        let mut temp: HashMap<String, f64> = HashMap::new();
        for asset in self.universe.get_assets() {
            let weight = MonthlyRebalancingWithDataSourceTradingSystem::fake_data_source(&initial);
            temp.insert(asset.to_owned(), weight.to_owned());
            initial += -weight
        }
        temp
    }

    fn should_trade_now(&self, date: &i64) -> bool {
        LastBusinessDayTradingSchedule::should_trade(date)
    }
}

impl MonthlyRebalancingWithDataSourceTradingSystem {
    fn fake_data_source(dist_top: &f64) -> f64 {
        let weight_dist = Uniform::new(0.01, dist_top);
        let mut rng = thread_rng();
        rng.sample(weight_dist)
    }

    pub fn new(universe: Rc<StaticUniverse>) -> Self {
        MonthlyRebalancingWithDataSourceTradingSystem { universe }
    }
}
