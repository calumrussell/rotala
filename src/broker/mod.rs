use std::collections::HashMap;

pub mod order;
pub mod sim;

#[derive(Clone, Debug)]
pub struct Quote {
    pub bid: f64,
    pub ask: f64,
    pub date: i64,
    pub symbol: String,
}

#[derive(Clone, Debug)]
pub struct Trade {
    symbol: String,
    value: f64,
    quantity: f64,
}

pub struct Holdings(HashMap<String, f64>);

pub struct TradeRecord {
    history: Vec<Trade>,
}

pub enum BrokerEvent {
    TradeSuccess(Trade),
    TradeFailure(order::Order),
    OrderCreated(order::Order),
    OrderFailure(order::Order),
    SuccessfulWithdraw(f64),
    CashTransactionSuccess(f64),
    InsufficientCash(f64),
}

pub trait CashManager {
    fn withdraw_cash(&mut self, cash: f64) -> BrokerEvent;
    fn deposit_cash(&mut self, cash: f64) -> BrokerEvent;
    fn debit(&mut self, value: f64) -> BrokerEvent;
    fn credit(&mut self, value: f64) -> BrokerEvent;
    fn get_cash_balance(&self) -> f64;
}

pub trait PositionInfo {
    fn get_position_qty(&self, symbol: &String) -> Option<f64>;
    fn get_position_value(&self, symbol: &String) -> Option<f64>;
    fn get_position_cost(&self, symbol: &String) -> Option<f64>;
    fn get_position_profit(&self, symbol: &String) -> Option<f64>;
}

pub trait PriceQuote {
    fn get_quote(&self, symbol: &String) -> Option<Quote>;
}

pub trait ClientControlled {
    fn update_holdings(&mut self, symbol: &String, change: &f64);
    fn get_holdings(&self) -> &Holdings;
    fn get(&self, symbol: &String) -> Option<&f64>;
}

pub trait TradeLedger {
    fn record(&mut self, trade: &Trade);
    fn cost_basis(&self, symbol: &String) -> Option<f64>;
}

pub trait PendingOrders {
    fn insert_order(&mut self, order: &order::Order);
    fn delete_order(&mut self, id: &u8);
}

impl TradeLedger for TradeRecord {
    fn record(&mut self, trade: &Trade) {
        self.history.push(trade.clone());
    }

    fn cost_basis(&self, symbol: &String) -> Option<f64> {
        let mut cum_qty = 0.0;
        let mut cum_val = 0.0;
        for h in &self.history {
            if h.symbol.eq(symbol) {
                cum_qty += h.quantity;
                cum_val += h.value;

                //reset the value if we are back to zero
                if cum_qty == 0.0 {
                    cum_val = 0.0;
                }
            }
        }
        if cum_qty == 0.0 {
            return None;
        }
        Some(cum_val / cum_qty)
    }
}

impl TradeRecord {
    pub fn new() -> Self {
        let history = Vec::new();
        TradeRecord { history }
    }
}

#[cfg(test)]
mod tests {
    use super::TradeLedger;

    #[test]
    fn test_that_ledger_calculates_the_cost_basis_correctly() {
        let mut rec = super::TradeRecord::new();

        let t1 = super::Trade {
            symbol: String::from("ABC"),
            quantity: 10.00,
            value: 100.0,
        };
        let t2 = super::Trade {
            symbol: String::from("ABC"),
            quantity: 90.00,
            value: 500.0,
        };
        let t3 = super::Trade {
            symbol: String::from("BCD"),
            quantity: 100.00,
            value: 100.0,
        };
        let t4 = super::Trade {
            symbol: String::from("BCD"),
            quantity: -100.00,
            value: -500.0,
        };
        let t5 = super::Trade {
            symbol: String::from("BCD"),
            quantity: 50.00,
            value: 50.0,
        };

        rec.record(&t1);
        rec.record(&t2);
        rec.record(&t3);
        rec.record(&t4);
        rec.record(&t5);

        let abc_cost = rec.cost_basis(&String::from("ABC")).unwrap();
        let bcd_cost = rec.cost_basis(&String::from("BCD")).unwrap();

        assert_eq!(abc_cost, 6.0);
        assert_eq!(bcd_cost, 1.0);
    }
}
