use itertools::Itertools;

use super::{BrokerRecordedEvents, Trade, TradeType};

//Records events executed by the broker.
//
//Should be available to clients, but is also need internally
//to calculate the cost basis of positions.
#[derive(Clone)]
pub struct BrokerLog {
    log: Vec<BrokerRecordedEvents>,
}

impl BrokerLog {
    pub fn record<E: Into<BrokerRecordedEvents>>(&mut self, event: E) {
        let brokerevent: BrokerRecordedEvents = event.into();
        self.log.push(brokerevent);
    }

    pub fn trades(&self) -> Vec<Trade> {
        let mut trades = Vec::new();
        for event in &self.log {
            match event {
                BrokerRecordedEvents::TradeCompleted(trade) => trades.push(trade.clone()),
                _ => (),
            }
        }
        trades
    }

    pub fn trades_between(&self, start: &i64, stop: &i64) -> Vec<Trade> {
        let trades = self.trades();
        trades
            .iter()
            .filter(|v| v.date >= *start && v.date <= *stop)
            .map(|v| v.clone())
            .collect_vec()
    }

    pub fn cost_basis(&self, symbol: &String) -> Option<f64> {
        let mut cum_qty = 0.0;
        let mut cum_val = 0.0;
        for event in &self.log {
            if let BrokerRecordedEvents::TradeCompleted(trade) = event {
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
                        cum_val = 0.0;
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

#[cfg(test)]
mod tests {
    use super::BrokerLog;

    use crate::broker::{Trade, TradeType};

    fn setup() -> BrokerLog {
        let mut rec = BrokerLog::new();

        let t1 = Trade {
            symbol: String::from("ABC"),
            quantity: 10.00,
            value: 100.0,
            date: 100,
            typ: TradeType::Buy,
        };
        let t2 = Trade {
            symbol: String::from("ABC"),
            quantity: 90.00,
            value: 500.0,
            date: 101,
            typ: TradeType::Buy,
        };
        let t3 = Trade {
            symbol: String::from("BCD"),
            quantity: 100.00,
            value: 100.0,
            date: 102,
            typ: TradeType::Buy,
        };
        let t4 = Trade {
            symbol: String::from("BCD"),
            quantity: 100.00,
            value: 500.0,
            date: 103,
            typ: TradeType::Sell,
        };
        let t5 = Trade {
            symbol: String::from("BCD"),
            quantity: 50.00,
            value: 50.0,
            date: 104,
            typ: TradeType::Buy,
        };

        rec.record(&t1);
        rec.record(&t2);
        rec.record(&t3);
        rec.record(&t4);
        rec.record(&t5);
        rec
    }

    #[test]
    fn test_that_log_filters_trades_between_dates() {
        let log = setup();
        let between = log.trades_between(&102, &104);
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
