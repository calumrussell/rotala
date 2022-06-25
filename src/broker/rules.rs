use crate::broker::{
    BrokerEvent, CashManager, ClientControlled, HasTime, Trade, TradeCosts, TradeType,
};
use crate::broker::{Order, OrderType};
use crate::data::Price;

pub struct OrderExecutionRules;

impl OrderExecutionRules {
    pub fn client_has_sufficient_cash(
        order: &Order,
        price: &Price,
        brkr: &(impl CashManager + TradeCosts),
    ) -> Result<bool, f64> {
        let shares = order.get_shares();
        let value = shares * *price;
        match order.get_order_type() {
            OrderType::MarketBuy => {
                if brkr.get_cash_balance() > value {
                    return Ok(true);
                }
                Err(f64::from(value))
            }
            OrderType::MarketSell => Ok(true),
            _ => unreachable!("Shouldn't hit unless something has gone wrong"),
        }
    }

    pub fn trade_logic(
        order: &Order,
        price: &Price,
        brkr: &mut (impl CashManager + ClientControlled + HasTime + TradeCosts),
    ) -> Trade {
        let value = *price * order.get_shares();
        //Update holdings
        let curr = brkr.get_qty(&order.get_symbol()).unwrap_or_default();
        let updated = match order.get_order_type() {
            OrderType::MarketBuy => *curr + order.get_shares(),
            OrderType::MarketSell => *curr - order.get_shares(),
            _ => panic!("Cannot call trade_logic with a non-market order"),
        };
        brkr.update_holdings(&order.get_symbol(), &updated);

        //Update cash
        match order.get_order_type() {
            OrderType::MarketBuy => brkr.debit(value),
            OrderType::MarketSell => brkr.credit(value),
            _ => unreachable!("Will throw earlier with other ordertype"),
        };

        let trade_type = match order.get_order_type() {
            OrderType::MarketBuy => TradeType::Buy,
            OrderType::MarketSell => TradeType::Sell,
            _ => unreachable!("Will throw earlier with other ordertype"),
        };

        let t = Trade {
            symbol: order.get_symbol(),
            value,
            quantity: order.get_shares(),
            date: brkr.now(),
            typ: trade_type,
        };

        let costs = brkr.get_trade_costs(&t);
        brkr.debit(costs);
        t
    }

    pub fn run_all<'a>(
        order: &Order,
        price: &Price,
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
