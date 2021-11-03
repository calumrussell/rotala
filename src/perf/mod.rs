use crate::broker::{CashManager, PositionInfo};
use crate::portfolio::PortfolioStats;

struct PortfolioSnapshot {
    value: f64,
}

pub struct PortfolioPerformance {
    history: Vec<PortfolioSnapshot>,
}

impl PortfolioPerformance {
    pub fn get_portfolio_return(&self) -> f64 {
        let mut last = f64::INFINITY;
        let mut sum = 0.0;

        for snap in &self.history {
            if last.eq(&f64::INFINITY) {
                last = snap.value;
            }
            let pct_change = (snap.value / last) - 1.0;
            if pct_change != 0.0 {
                sum += (1.0 + pct_change).log10();
            }
        }
        (10_f64.powf(sum) - 1.0) * 100.0
    }

    pub fn update(&mut self, port: &impl PortfolioStats, brkr: &(impl PositionInfo + CashManager)) {
        let value = port.get_total_value(brkr);
        let snap = PortfolioSnapshot { value };
        self.history.push(snap);
    }

    pub fn new() -> Self {
        let history: Vec<PortfolioSnapshot> = Vec::new();
        PortfolioPerformance { history }
    }
}
