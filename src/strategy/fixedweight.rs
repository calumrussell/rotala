use crate::data::{CashValue, DateTime, PortfolioAllocation, PortfolioWeight};
use crate::perf::{PerfStruct, PortfolioPerformance};
use crate::portfolio::{Portfolio, PortfolioStats};
use crate::schedule::{DefaultTradingSchedule, TradingSchedule};
use crate::sim::portfolio::SimPortfolio;
use crate::strategy::Strategy;

#[derive(Clone)]
pub struct FixedWeightStrategy {
    portfolio: SimPortfolio,
    date: DateTime,
    target_weights: PortfolioAllocation<PortfolioWeight>,
    perf: PortfolioPerformance,
}

impl Strategy for FixedWeightStrategy {
    fn get_perf(&self) -> PerfStruct {
        self.perf.get_output()
    }

    fn set_date(&mut self, date: &DateTime) {
        let state = self.portfolio.set_date(date);
        self.date = *date;
        self.perf.update(&state)
    }

    fn init(&mut self, initital_cash: &CashValue) {
        self.portfolio.deposit_cash(initital_cash);
    }

    fn run(&mut self) -> CashValue {
        if DefaultTradingSchedule::should_trade(&self.date) {
            let orders = self.portfolio.update_weights(&self.target_weights);
            if !orders.is_empty() {
                self.portfolio.execute_orders(orders);
            }
        }
        self.portfolio.get_total_value()
    }
}

impl FixedWeightStrategy {
    pub fn new(
        portfolio: SimPortfolio,
        perf: PortfolioPerformance,
        target_weights: PortfolioAllocation<PortfolioWeight>,
    ) -> Self {
        FixedWeightStrategy {
            portfolio,
            date: DateTime::from(-1),
            target_weights,
            perf,
        }
    }
}

impl From<&FixedWeightStrategy> for Box<FixedWeightStrategy> {
    fn from(strat: &FixedWeightStrategy) -> Self {
        let owned: FixedWeightStrategy = strat.clone();
        Box::new(owned)
    }
}

impl From<&FixedWeightStrategy> for Box<dyn Strategy> {
    fn from(strat: &FixedWeightStrategy) -> Self {
        let owned: FixedWeightStrategy = strat.clone();
        Box::new(owned)
    }
}
