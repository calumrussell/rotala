use super::order::{Order, OrderType};
use super::{BrokerEvent, CashManager, ClientControlled, Trade, TradeLedger};

pub struct OrderExecutionRules;

impl OrderExecutionRules {
    pub fn client_has_sufficient_cash(
        order: &Order,
        price: &f64,
        brkr: &impl CashManager,
    ) -> Result<bool, f64> {
        match order.order_type {
            OrderType::MarketBuy => {
                let value = price * order.shares as f64;
                if brkr.get_cash_balance() >= value {
                    return Ok(true);
                }
                Err(value)
            }
            _ => Ok(true),
        }
    }

    pub fn trade_logic(
        order: &Order,
        price: &f64,
        brkr: &mut (impl CashManager + ClientControlled + TradeLedger),
    ) -> Trade {
        let value = price * order.shares;
        //Update holdings
        let curr = brkr.get(&order.symbol).unwrap_or(&0.0);
        let updated = match order.order_type {
            OrderType::MarketBuy => curr + order.shares as f64,
            OrderType::MarketSell => curr - order.shares as f64,
            _ => panic!("Cannot call trade_logic with a non-market order"),
        };
        brkr.update_holdings(&order.symbol, &updated);

        //Update cash
        match order.order_type {
            OrderType::MarketBuy => brkr.debit(value),
            OrderType::MarketSell => brkr.credit(value),
            _ => panic!("Cannot call trade_logic with a non-market order"),
        };

        let t = Trade {
            symbol: order.symbol.clone(),
            value,
            quantity: order.shares.clone() as f64,
        };

        //Update trade ledger
        brkr.record(&t);
        t
    }

    pub fn run_all<'a>(
        order: &'a Order,
        price: &'a f64,
        brkr: &'a mut (impl CashManager + ClientControlled + TradeLedger),
    ) -> Result<impl FnOnce() -> Trade + 'a, BrokerEvent> {
        let has_cash = OrderExecutionRules::client_has_sufficient_cash(order, price, brkr);
        if has_cash.is_err() {
            return Err(BrokerEvent::InsufficientCash(has_cash.unwrap_err()));
        }
        let trade = move || OrderExecutionRules::trade_logic(order, price, brkr);
        //We return a function so that the caller has a chance to stop the trade
        //or control when it is executed
        Ok(trade)
    }
}
