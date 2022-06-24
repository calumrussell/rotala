use itertools::Itertools;

use super::{BrokerRecordedEvent, DividendPayment, Trade, TradeType};
use crate::data::{CashValue, DateTime, PortfolioQty, Price};

//Records events executed by the broker.
//
//Should be available to clients, but is also need internally
//to calculate the cost basis of positions.
#[derive(Clone)]
pub struct BrokerLog {
    log: Vec<BrokerRecordedEvent>,
}

impl BrokerLog {
    pub fn record<E: Into<BrokerRecordedEvent>>(&mut self, event: E) {
        let brokerevent: BrokerRecordedEvent = event.into();
        self.log.push(brokerevent);
    }

    pub fn trades(&self) -> Vec<Trade> {
        let mut trades = Vec::new();
        for event in &self.log {
            if let BrokerRecordedEvent::TradeCompleted(trade) = event {
                trades.push(trade.clone());
            }
        }
        trades
    }

    pub fn dividends(&self) -> Vec<DividendPayment> {
        let mut dividends = Vec::new();
        for event in &self.log {
            if let BrokerRecordedEvent::DividendPaid(dividend) = event {
                dividends.push(dividend.clone());
            }
        }
        dividends
    }

    pub fn dividends_between(&self, start: &DateTime, stop: &DateTime) -> Vec<DividendPayment> {
        let dividends = self.dividends();
        dividends
            .iter()
            .filter(|v| v.date >= *start && v.date <= *stop)
            .cloned()
            .collect_vec()
    }

    pub fn trades_between(&self, start: &DateTime, stop: &DateTime) -> Vec<Trade> {
        let trades = self.trades();
        trades
            .iter()
            .filter(|v| v.date >= *start && v.date <= *stop)
            .cloned()
            .collect_vec()
    }

    pub fn cost_basis(&self, symbol: &str) -> Option<Price> {
        let mut cum_qty = PortfolioQty::default();
        let mut cum_val = CashValue::default();
        for event in &self.log {
            if let BrokerRecordedEvent::TradeCompleted(trade) = event {
                if trade.symbol.eq(symbol) {
                    match trade.typ {
                        TradeType::Buy => {
                            cum_qty += trade.quantity;
                            cum_val += trade.value;
                        }
                        TradeType::Sell => {
                            cum_qty -= trade.quantity;
                            cum_val -= trade.value;
                        }
                    }
                    //reset the value if we are back to zero
                    if cum_qty == 0.0 {
                        cum_val = CashValue::default();
                    }
                }
            }
        }
        if cum_qty == 0.0 {
            return None;
        }
        Some(cum_val / cum_qty)
    }
}

impl BrokerLog {
    pub fn new() -> Self {
        BrokerLog { log: Vec::new() }
    }
}

impl Default for BrokerLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::BrokerLog;

    use crate::broker::{Trade, TradeType};

    fn setup() -> BrokerLog {
        let mut rec = BrokerLog::new();

        let t1 = Trade {
            symbol: String::from("ABC"),
            quantity: 10.00.into(),
            value: 100.0.into(),
            date: 100.into(),
            typ: TradeType::Buy,
        };
        let t2 = Trade {
            symbol: String::from("ABC"),
            quantity: 90.00.into(),
            value: 500.0.into(),
            date: 101.into(),
            typ: TradeType::Buy,
        };
        let t3 = Trade {
            symbol: String::from("BCD"),
            quantity: 100.00.into(),
            value: 100.0.into(),
            date: 102.into(),
            typ: TradeType::Buy,
        };
        let t4 = Trade {
            symbol: String::from("BCD"),
            quantity: 100.00.into(),
            value: 500.0.into(),
            date: 103.into(),
            typ: TradeType::Sell,
        };
        let t5 = Trade {
            symbol: String::from("BCD"),
            quantity: 50.00.into(),
            value: 50.0.into(),
            date: 104.into(),
            typ: TradeType::Buy,
        };

        rec.record(t1);
        rec.record(t2);
        rec.record(t3);
        rec.record(t4);
        rec.record(t5);
        rec
    }

    #[test]
    fn test_that_log_filters_trades_between_dates() {
        let log = setup();
        let between = log.trades_between(&102.into(), &104.into());
        assert!(between.len() == 3);
    }

    #[test]
    fn test_that_log_calculates_the_cost_basis() {
        let log = setup();
        let abc_cost = log.cost_basis(&String::from("ABC")).unwrap();
        let bcd_cost = log.cost_basis(&String::from("BCD")).unwrap();

        assert_eq!(abc_cost, 6.0);
        assert_eq!(bcd_cost, 1.0);
    }
}
