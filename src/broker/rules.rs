use crate::broker::{BrokerEvent, CashManager, ClientControlled, HasTime, Trade, TradeCosts};
use crate::broker::{Order, OrderType};

pub struct OrderExecutionRules;

impl OrderExecutionRules {
    pub fn client_has_sufficient_cash(
        order: &Order,
        price: &f64,
        brkr: &(impl CashManager + TradeCosts),
    ) -> Result<bool, u64> {
        let shares = order.get_shares();
        let value = price * shares;
        match order.get_order_type() {
            OrderType::MarketBuy => {
                if brkr.get_cash_balance() as f64 > value {
                    return Ok(true);
                }
                Err(value as u64)
            }
            OrderType::MarketSell => Ok(true),
            _ => unreachable!("Shouldn't hit unless something has gone wrong"),
        }
    }

    pub fn trade_logic(
        order: &Order,
        price: &f64,
        brkr: &mut (impl CashManager + ClientControlled + HasTime + TradeCosts),
    ) -> Trade {
        let value = price * order.get_shares();
        //Update holdings
        let curr = brkr.get(&order.get_symbol()).unwrap_or(&0.0);
        let updated = match order.get_order_type() {
            OrderType::MarketBuy => curr + order.get_shares() as f64,
            OrderType::MarketSell => curr - order.get_shares() as f64,
            _ => panic!("Cannot call trade_logic with a non-market order"),
        };
        brkr.update_holdings(&order.get_symbol(), &updated);

        //Update cash
        match order.get_order_type() {
            OrderType::MarketBuy => brkr.debit(value as u64),
            OrderType::MarketSell => brkr.credit(value as u64),
            _ => panic!("Cannot call trade_logic with a non-market order"),
        };

        let t = Trade {
            symbol: order.get_symbol().clone(),
            value,
            quantity: order.get_shares().clone() as f64,
            date: brkr.now(),
        };

        let costs = brkr.get_trade_costs(&t);
        brkr.debit(costs as u64);
        t
    }

    pub fn run_all<'a>(
        order: &Order,
        price: &f64,
        brkr: &'a mut (impl CashManager + ClientControlled + TradeCosts + HasTime),
    ) -> Result<Trade, BrokerEvent> {
        let has_cash = OrderExecutionRules::client_has_sufficient_cash(order, price, brkr);
        if has_cash.is_err() {
            return Err(BrokerEvent::TradeFailure(order.clone()));
        }
        let trade = OrderExecutionRules::trade_logic(order, price, brkr);
        Ok(trade)
    }
}
