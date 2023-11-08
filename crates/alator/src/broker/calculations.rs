use log::info;

use crate::{
    input::Quotable,
    types::{CashValue, PortfolioAllocation, PortfolioQty, Price},
};

use super::{BacktestBroker, ReceivesOrders, ReceivesOrdersAsync};

/// Groups calculations standard to most brokers. These are not bound into any other implementation
/// and operate on traits rather than specific implementations.
pub struct BrokerCalculations;

impl BrokerCalculations {
    /// Withdrawing with liquidation will queue orders to generate the expected amount of cash. No
    /// ordering to the assets that are sold, the broker is responsible for managing cash but not
    /// re-aligning to a target portfolio.
    ///
    /// Because orders are not executed instaneously this method can be the source of significant
    /// divergences in performance from the underlying in certain cases. For example, if prices are
    /// volatile, in the case of low-frequency data, then the broker will end up continuously
    /// re-balancing in a random way under certain price movements.
    pub fn withdraw_cash_with_liquidation<Q: Quotable, T: BacktestBroker<Q> + ReceivesOrders>(
        cash: &f64,
        brkr: &mut T,
    ) -> super::BrokerCashEvent {
        // TODO: is it better to return a sequence of orders to achieve a cash balance? Because
        // of the linkage with execution, we need seperate methods for sync/async.
        info!("BROKER: Withdrawing {:?} with liquidation", cash);
        let value = brkr.get_liquidation_value();
        if cash > &value {
            //There is no way for the portfolio to recover, we leave the portfolio in an invalid
            //state because the client may be able to recover later
            brkr.debit(cash);
            info!(
                "BROKER: Failed to withdraw {:?} with liquidation. Deducting value from cash.",
                cash
            );
            super::BrokerCashEvent::WithdrawFailure(CashValue::from(*cash))
        } else {
            //This holds how much we have left to generate from the portfolio to produce the cash
            //required
            let mut total_sold = *cash;

            let positions = brkr.get_positions();
            let mut sell_orders: Vec<super::Order> = Vec::new();
            for ticker in positions {
                let position_value = brkr
                    .get_position_value(&ticker)
                    .unwrap_or(CashValue::from(0.0));
                //Position won't generate enough cash to fulfill total order
                //Create orders for selling 100% of position, continue
                //to next position to see if we can generate enough cash
                //
                //Sell 100% of position
                if *position_value <= total_sold {
                    //Cannot be called without qty existing
                    if let Some(qty) = brkr.get_position_qty(&ticker) {
                        let order =
                            super::Order::market(super::OrderType::MarketSell, ticker, qty.clone());
                        info!("BROKER: Withdrawing {:?} with liquidation, queueing sale of {:?} shares of {:?}", cash, order.get_shares(), order.get_symbol());
                        sell_orders.push(order);
                        total_sold -= *position_value;
                    }
                } else {
                    //Position can generate all the cash we need
                    //Create orders to sell 100% of position, don't continue to next stock
                    //
                    //Cannot be called without quote existing so unwrap
                    let quote = brkr.get_quote(&ticker).unwrap();
                    let price = quote.get_bid();
                    let shares_req = PortfolioQty::from((total_sold / **price).ceil());
                    let order =
                        super::Order::market(super::OrderType::MarketSell, ticker, shares_req);
                    info!("BROKER: Withdrawing {:?} with liquidation, queueing sale of {:?} shares of {:?}", cash, order.get_shares(), order.get_symbol());
                    sell_orders.push(order);
                    total_sold = 0.0;
                    break;
                }
            }
            if (total_sold).eq(&0.0) {
                //The portfolio can provide enough cash so we can execute the sell orders
                //We leave the portfolio in the wrong state for the client to deal with
                brkr.send_orders(&sell_orders);
                info!("BROKER: Succesfully withdrew {:?} with liquidation", cash);
                super::BrokerCashEvent::WithdrawSuccess(CashValue::from(*cash))
            } else {
                //For whatever reason, we went through the above process and were unable to find
                //the cash. Don't send any orders, leave portfolio in invalid state for client to
                //potentially recover.
                brkr.debit(cash);
                info!(
                    "BROKER: Failed to withdraw {:?} with liquidation. Deducting value from cash.",
                    cash
                );
                super::BrokerCashEvent::WithdrawFailure(CashValue::from(*cash))
            }
        }
    }

    /// Withdrawing with liquidation will queue orders to generate the expected amount of cash. No
    /// ordering to the assets that are sold, the broker is responsible for managing cash but not
    /// re-aligning to a target portfolio.
    ///
    /// Because orders are not executed instaneously this method can be the source of significant
    /// divergences in performance from the underlying in certain cases. For example, if prices are
    /// volatile, in the case of low-frequency data, then the broker will end up continuously
    /// re-balancing in a random way under certain price movements.
    pub async fn withdraw_cash_with_liquidation_async<
        Q: Quotable,
        T: BacktestBroker<Q> + ReceivesOrdersAsync,
    >(
        cash: &f64,
        brkr: &mut T,
    ) -> super::BrokerCashEvent {
        info!("BROKER: Withdrawing {:?} with liquidation", cash);
        let value = brkr.get_liquidation_value();
        if cash > &value {
            //There is no way for the portfolio to recover, we leave the portfolio in an invalid
            //state because the client may be able to recover later
            brkr.debit(cash);
            info!(
                "BROKER: Failed to withdraw {:?} with liquidation. Deducting value from cash.",
                cash
            );
            super::BrokerCashEvent::WithdrawFailure(CashValue::from(*cash))
        } else {
            //This holds how much we have left to generate from the portfolio to produce the cash
            //required
            let mut total_sold = *cash;

            let positions = brkr.get_positions();
            let mut sell_orders: Vec<super::Order> = Vec::new();
            for ticker in positions {
                let position_value = brkr
                    .get_position_value(&ticker)
                    .unwrap_or(CashValue::from(0.0));
                //Position won't generate enough cash to fulfill total order
                //Create orders for selling 100% of position, continue
                //to next position to see if we can generate enough cash
                //
                //Sell 100% of position
                if *position_value <= total_sold {
                    //Cannot be called without qty existing
                    if let Some(qty) = brkr.get_position_qty(&ticker) {
                        let order =
                            super::Order::market(super::OrderType::MarketSell, ticker, qty.clone());
                        info!("BROKER: Withdrawing {:?} with liquidation, queueing sale of {:?} shares of {:?}", cash, order.get_shares(), order.get_symbol());
                        sell_orders.push(order);
                        total_sold -= *position_value;
                    }
                } else {
                    //Position can generate all the cash we need
                    //Create orders to sell 100% of position, don't continue to next stock
                    //
                    //Cannot be called without quote existing so unwrap
                    let quote = brkr.get_quote(&ticker).unwrap();
                    let price = quote.get_bid();
                    let shares_req = PortfolioQty::from((total_sold / **price).ceil());
                    let order =
                        super::Order::market(super::OrderType::MarketSell, ticker, shares_req);
                    info!("BROKER: Withdrawing {:?} with liquidation, queueing sale of {:?} shares of {:?}", cash, order.get_shares(), order.get_symbol());
                    sell_orders.push(order);
                    total_sold = 0.0;
                    break;
                }
            }
            if (total_sold).eq(&0.0) {
                //The portfolio can provide enough cash so we can execute the sell orders
                //We leave the portfolio in the wrong state for the client to deal with
                brkr.send_orders(&sell_orders).await;
                info!("BROKER: Succesfully withdrew {:?} with liquidation", cash);
                super::BrokerCashEvent::WithdrawSuccess(CashValue::from(*cash))
            } else {
                //For whatever reason, we went through the above process and were unable to find
                //the cash. Don't send any orders, leave portfolio in invalid state for client to
                //potentially recover.
                brkr.debit(cash);
                info!(
                    "BROKER: Failed to withdraw {:?} with liquidation. Deducting value from cash.",
                    cash
                );
                super::BrokerCashEvent::WithdrawFailure(CashValue::from(*cash))
            }
        }
    }

    /// Calculates difference between current broker state and a target allocation, the latter
    /// typically passed from a strategy.
    ///
    /// Brokers do not expect target wights, they merely respond to orders so this structure
    /// is not required to create backtests.
    pub fn diff_brkr_against_target_weights<Q: Quotable, T: BacktestBroker<Q>>(
        target_weights: &PortfolioAllocation,
        brkr: &mut T,
    ) -> Vec<super::Order> {
        //Returns orders so calling function has control over when orders are executed
        //Requires mutable reference to brkr because it calls get_position_value
        //Need liquidation value so we definitely have enough money to make all transactions after
        //costs
        info!("STRATEGY: Calculating diff of current allocation vs. target");
        let total_value = brkr.get_liquidation_value();
        if (*total_value).eq(&0.0) {
            panic!("Client is attempting to trade a portfolio with zero value");
        }
        let mut orders: Vec<super::Order> = Vec::new();

        let mut buy_orders: Vec<super::Order> = Vec::new();
        let mut sell_orders: Vec<super::Order> = Vec::new();

        //This returns a positive number for buy and negative for sell, this is necessary because
        //of calculations made later to find the net position of orders on the exchange.
        let calc_required_shares_with_costs = |diff_val: &f64, quote: &Q, brkr: &T| -> f64 {
            if diff_val.lt(&0.0) {
                let price = **quote.get_bid();
                let costs = brkr.calc_trade_impact(&diff_val.abs(), &price, false);
                let total = (*costs.0 / *costs.1).floor();
                -total
            } else {
                let price = **quote.get_ask();
                let costs = brkr.calc_trade_impact(&diff_val.abs(), &price, true);
                (*costs.0 / *costs.1).floor()
            }
        };

        for symbol in target_weights.keys() {
            let curr_val = brkr
                .get_position_value(&symbol)
                .unwrap_or(CashValue::from(0.0));
            //Iterating over target_weights so will always find value
            let target_val = CashValue::from(*total_value * **target_weights.get(&symbol).unwrap());
            let diff_val = CashValue::from(*target_val - *curr_val);
            if (*diff_val).eq(&0.0) {
                break;
            }

            //We do not throw an error here, we just proceed assuming that the client has passed in data that will
            //eventually prove correct if we are missing quotes for the current time.
            if let Some(quote) = brkr.get_quote(&symbol) {
                //This will be negative if the net is selling
                let required_shares = calc_required_shares_with_costs(&diff_val, &quote, brkr);
                //TODO: must be able to clear pending orders
                //Clear any pending orders on the exchange
                //self.clear_pending_market_orders_by_symbol(&symbol);
                if required_shares.ne(&0.0) {
                    if required_shares.gt(&0.0) {
                        buy_orders.push(super::Order::market(
                            super::OrderType::MarketBuy,
                            symbol.clone(),
                            required_shares,
                        ));
                    } else {
                        sell_orders.push(super::Order::market(
                            super::OrderType::MarketSell,
                            symbol.clone(),
                            //Order stores quantity as non-negative
                            required_shares.abs(),
                        ));
                    }
                }
            }
        }
        //Sell orders have to be executed before buy orders
        orders.extend(sell_orders);
        orders.extend(buy_orders);
        orders
    }

    pub fn client_has_sufficient_cash<Q: Quotable>(
        order: &super::Order,
        price: &Price,
        brkr: &impl BacktestBroker<Q>,
    ) -> Result<(), super::InsufficientCashError> {
        let shares = order.get_shares();
        let value = CashValue::from(**shares * **price);
        match order.get_order_type() {
            super::OrderType::MarketBuy => {
                if brkr.get_cash_balance() > value {
                    return Ok(());
                }
                Err(super::InsufficientCashError)
            }
            super::OrderType::MarketSell => Ok(()),
            _ => unreachable!("Shouldn't hit unless something has gone wrong"),
        }
    }

    pub fn client_has_sufficient_holdings_for_sale<Q: Quotable>(
        order: &super::Order,
        brkr: &impl BacktestBroker<Q>,
    ) -> Result<(), super::UnexecutableOrderError> {
        if let super::OrderType::MarketSell = order.get_order_type() {
            if let Some(holding) = brkr.get_position_qty(order.get_symbol()) {
                if *holding >= **order.get_shares() {
                    return Ok(());
                } else {
                    return Err(super::UnexecutableOrderError);
                }
            }
        }
        Ok(())
    }

    pub fn client_is_issuing_nonsense_order(
        order: &super::Order,
    ) -> Result<(), super::UnexecutableOrderError> {
        let shares = **order.get_shares();
        if shares == 0.0 {
            return Err(super::UnexecutableOrderError);
        }
        Ok(())
    }
}
