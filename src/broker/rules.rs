use crate::broker::{
    BrokerEvent, CanUpdate, PositionInfo, Trade, TradeCost, TradeType, TransferCash,
};
use crate::broker::{Order, OrderType};
use crate::types::{DateTime, Price};

pub struct OrderExecutionRules;

impl OrderExecutionRules {
    fn client_has_sufficient_cash(
        order: &Order,
        price: &Price,
        brkr: &(impl TransferCash + TradeCost),
    ) -> Result<(), ()> {
        let shares = order.get_shares();
        let value = shares * *price;
        match order.get_order_type() {
            OrderType::MarketBuy => {
                if brkr.get_cash_balance() > value {
                    return Ok(());
                }
                Err(())
            }
            OrderType::MarketSell => Ok(()),
            _ => unreachable!("Shouldn't hit unless something has gone wrong"),
        }
    }

    fn client_has_sufficient_holdings_for_sale(
        order: &Order,
        brkr: &impl PositionInfo,
    ) -> Result<(), ()> {
        if let OrderType::MarketSell = order.get_order_type() {
            if let Some(holding) = brkr.get_position_qty(&order.get_symbol()) {
                if *holding >= order.shares {
                    return Ok(());
                }
            }
            Err(())
        } else {
            Ok(())
        }
    }

    fn trade_logic(
        order: &Order,
        price: &Price,
        date: &DateTime,
        brkr: &mut (impl PositionInfo + TransferCash + CanUpdate + TradeCost),
    ) -> Trade {
        let value = *price * order.get_shares();
        //Update holdings
        let curr = brkr
            .get_position_qty(&order.get_symbol())
            .unwrap_or_default();
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
            date: *date,
            typ: trade_type,
        };

        let costs = brkr.get_trade_costs(&t);
        brkr.debit(costs);
        t
    }

    pub fn run_all<'a>(
        order: &Order,
        price: &Price,
        date: &DateTime,
        brkr: &'a mut (impl PositionInfo + TransferCash + CanUpdate + TradeCost),
    ) -> Result<Trade, BrokerEvent> {
        if let Err(()) = OrderExecutionRules::client_has_sufficient_cash(order, price, brkr) {
            return Err(BrokerEvent::TradeFailure(order.clone()));
        }
        if let Err(()) = OrderExecutionRules::client_has_sufficient_holdings_for_sale(order, brkr) {
            return Err(BrokerEvent::TradeFailure(order.clone()));
        }
        let trade = OrderExecutionRules::trade_logic(order, price, date, brkr);
        Ok(trade)
    }
}
