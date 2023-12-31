//! Issues orders to exchange and tracks changes as exchange executes orders.
//!
//! ### Single-threaded detail
//!
//! [single::SingleBroker] holds a reference to an exchange to which orders are passed for
//! execution. Orders are not executed until the next tick so broker state has to be synchronized
//! with the exchange on every tick. Clients can trigger this synchronization by calling `check`.
//!
//! Exchange is expected to implement [alator_exchange::ExchangeSync] and the single-threaded impl
//! should be the only owner of the exchange during a backtest.
//!
//! Broker should be the only owner of a [crate::input::DefaultCorporateEventsSource] in a backtest.
//!
//! Trade costs are optional. If no trade costs are passed to the broker then no costs will be
//! taken when orders execute.
//!
//! ### General comments
//!
//! The broker can hold negative cash values due to the non-immediate execution of trades. Once a
//! broker has received the notification of completed trades and finds a negative value then
//! re-balancing is triggered automatically. Responsibility for moving the portfolio back to the
//! correct state is left with owner, broker implementations take responsibility for correcting
//! invalid internal state (like negative cash values).
//!
//! If a series has high levels of volatility between periods then performance will fail to
//! replicate the strategy due to continued rebalancing.
//!
//! To minimize the distortions due to rebalancing behaviour, the broker will target a minimum
//! cash value. This is currently set at an arbitrary level, 1_000, as this is something library
//! dependent and potentially difficult to explain.
//!
//! If a portfolio has a negative value, the current behaviour is to continue trading potentially
//! producing unexpected results. Previous versions would exit early when this happened but this
//! behaviour was removed.
//!
//! Default implementations support multiple [BrokerCost] models: Flat, PerShare, and PctOfValue.
//!
//! Cash balances are held in single currency which is assumed to be the same currency used across
//! the simulation.
//!
//! Keeps an internal log of trades executed and dividends received/paid. This is distinct from
//! performance calculations.

use std::{error::Error, fmt::{Display, Formatter}};

use log::info;

use crate::types::{CashValue, Price, PortfolioValues, PortfolioQty, PortfolioHoldings, PortfolioAllocation};

use self::types::BrokerCost;
pub mod types;
pub mod uist;

#[derive(Debug)]
enum BrokerState {
    Ready,
    Failed,
}

#[derive(Clone, Debug)]
pub enum BrokerEvent<O> {
    OrderSentToExchange(O),
    OrderInvalid(O),
    OrderCreated(O),
    OrderFailure(O),
}


#[derive(Clone, Debug)]
pub enum BrokerCashEvent {
    //Removed from [BrokerEvent] because there are situations when we want to handle these events
    //specifically and seperately
    WithdrawSuccess(CashValue),
    WithdrawFailure(CashValue),
    DepositSuccess(CashValue),
    OperationFailure(CashValue),
}


/// Broker has attempted to execute an order which cannot be completed due to insufficient cash.
#[derive(Debug, Clone)]
pub struct InsufficientCashError;

impl Error for InsufficientCashError {}

impl Display for InsufficientCashError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "Client has insufficient cash to execute order")
    }
}

#[derive(Debug, Clone)]
/// Broker has attempted to execute an order which cannot be completed due to a problem with the
/// order.
pub struct UnexecutableOrderError;

impl Error for UnexecutableOrderError {}

impl Display for UnexecutableOrderError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "Client has passed unexecutable order")
    }
}

trait Quote {
    type Value;

    fn get_quote(&self, symbol: &str) -> Option<Self::Value>;
    fn get_quotes(&self) -> Option<Vec<Self::Value>>;
}

trait SendOrder {
    type Value;

    fn send_order(&mut self, order: Self::Value) -> BrokerEvent;
    fn send_orders(&mut self, orders: &[Self::Value]) -> Vec<BrokerEvent>;
}

trait Portfolio: Quote {
    fn get_position_profit(&self, symbol: &str) -> Option<CashValue> {
        if let Some(cost) = self.get_position_cost(symbol) {
            if let Some(qty) = self.get_position_qty(symbol) {
                if let Some(position_value) = self.get_position_value(symbol) {
                    let price = *position_value / *qty.clone();
                    let value = CashValue::from(*qty.clone() * (price - *cost));
                    return Some(value);
                }
            }
        }
        None
    }

    fn get_position_liquidation_value(&self, symbol: &str) -> Option<CashValue> {
        if let Some(position_value) = self.get_position_value(symbol) {
            if let Some(qty) = self.get_position_qty(symbol) {
                let price = Price::from(*position_value / *qty);
                let (value_after_costs, _price_after_costs) =
                    self.calc_trade_impact(&position_value, &price, false);
                return Some(value_after_costs);
            }
        }
        None
    }

    fn get_total_value(&self) -> CashValue {
        let assets = self.get_positions();
        let mut value = self.get_cash_balance();
        for a in assets {
            if let Some(position_value) = self.get_position_value(&a) {
                value = CashValue::from(*value + *position_value);
            }
        }
        value
    }

    fn get_liquidation_value(&self) -> CashValue {
        let mut value = self.get_cash_balance();
        for asset in self.get_positions() {
            if let Some(asset_value) = self.get_position_liquidation_value(&asset) {
                value = CashValue::from(*value + *asset_value);
            }
        }
        value
    }

    fn get_values(&self) -> PortfolioValues {
        let mut holdings = PortfolioValues::new();
        let assets = self.get_positions();
        for a in assets {
            if let Some(_qty) = self.get_position_qty(&a) {
                if let Some(value) = self.get_position_value(&a) {
                    holdings.insert(&a, &value);
                }
            }
        }
        holdings
    }

    fn get_position_qty(&self, symbol: &str) -> Option<PortfolioQty> {
        self.get_holdings().get(symbol).to_owned()
    }

    fn get_position_value(&self, symbol: &str) -> Option<CashValue> {
        if let Some(quote) = self.get_quote(symbol) {
            //We only have long positions so we only need to look at the bid
            let price = quote.get_bid();
            if let Some(qty) = self.get_position_qty(symbol) {
                let val = price * *qty;
                return Some(CashValue::from(val));
            }
        }
        //This should only occur in cases when the client erroneously asks for a security with no
        //current or historical prices
        //Likely represents an error in client code but we don't panic here in case data for this
        //symbol appears later
        None
    }

    fn get_positions(&self) -> Vec<String> {
        self.get_holdings().keys()
    }

    fn get_holdings_with_pending(&self) -> PortfolioHoldings {
        let mut merged_holdings = PortfolioHoldings::new();
        for (key, value) in self.get_holdings().0.iter() {
            if merged_holdings.0.contains_key(key) {
                if let Some(val) = merged_holdings.get(key) {
                    let new_val = PortfolioQty::from(*val + **value);
                    merged_holdings.insert(key, &new_val);
                }
            } else {
                merged_holdings.insert(key, value);
            }
        }

        for (key, value) in self.get_pending_orders().0.iter() {
            if merged_holdings.0.contains_key(key) {
                if let Some(val) = merged_holdings.get(key) {
                    let new_val = PortfolioQty::from(*val + **value);
                    merged_holdings.insert(key, &new_val);
                }
            } else {
                merged_holdings.insert(key, value);
            }
        }
        merged_holdings
    }

    fn calculate_trade_costs(&self, trade: &UistTrade) -> CashValue {
        let mut cost = CashValue::default();
        for trade_cost in &self.get_trade_costs() {
            cost = CashValue::from(*cost + *trade_cost.calc(trade.clone()));
        }
        cost
    }

    fn calc_trade_impact(&self, budget: &f64, price: &f64, is_buy: bool) -> (CashValue, Price) {
        BrokerCost::trade_impact_total(&self.get_trade_costs(), budget, price, is_buy)
    }

    fn get_cash_balance(&self) -> CashValue;
    fn update_cash_balance(&self, cash: CashValue);
    fn get_holdings(&self) -> PortfolioHoldings;
    fn update_holdings(&mut self, symbol: &str, change: PortfolioQty);
    fn get_position_cost(&self, symbol: &str) -> Option<Price>;
    fn get_pending_orders(&self) -> PortfolioHoldings;
    fn get_trade_costs(&self) -> Vec<BrokerCost>;
}

trait BrokerStates {
    fn get_broker_state(&self) -> BrokerState;
    fn update_broker_state(&self, state: BrokerState);
}

trait CashOperations: Portfolio + BrokerStates {
    fn withdraw_cash(&mut self, cash: &f64) -> BrokerCashEvent {
        match self.get_broker_state() {
            BrokerState::Failed => {
                info!(
                    "BROKER: Attempted cash withdraw of {:?} but broker in Failed State",
                    cash,
                );
                BrokerCashEvent::OperationFailure(CashValue::from(*cash))
            }
            BrokerState::Ready => {
                if cash > &self.get_cash_balance() {
                    info!(
                        "BROKER: Attempted cash withdraw of {:?} but only have {:?}",
                        cash,
                        self.get_cash_balance()
                    );
                    return BrokerCashEvent::WithdrawFailure(CashValue::from(*cash));
                }
                info!(
                    "BROKER: Successful cash withdraw of {:?}, {:?} left in cash",
                    cash,
                    self.get_cash_balance()
                );
                self.debit(cash);
                BrokerCashEvent::WithdrawSuccess(CashValue::from(*cash))
            }
        }
    }

    fn deposit_cash(&mut self, cash: &f64) -> BrokerCashEvent {
        match self.get_broker_state() {
            BrokerState::Failed => {
                info!(
                    "BROKER: Attempted cash deposit of {:?} but broker in Failed State",
                    cash,
                );
                BrokerCashEvent::OperationFailure(CashValue::from(*cash))
            }
            BrokerState::Ready => {
                info!(
                    "BROKER: Deposited {:?} cash, current balance of {:?}",
                    cash,
                    self.get_cash_balance()
                );
                self.credit(cash);
                BrokerCashEvent::DepositSuccess(CashValue::from(*cash))
            }
        }
    }

    //Identical to deposit_cash but is seperated to distinguish internal cash
    //transactions from external with no value returned to client
    fn credit(&mut self, value: &f64) -> BrokerCashEvent {
        info!(
            "BROKER: Credited {:?} cash, current balance of {:?}",
            value, self.get_cash_balance()
        );
        self.update_cash_balance(CashValue::from(*value + *self.get_cash_balance()));
        BrokerCashEvent::DepositSuccess(CashValue::from(*value))
    }

    //Looks similar to withdraw_cash but distinguished because it represents
    //failure of an internal transaction with no value returned to clients
    fn debit(&mut self, value: &f64) -> BrokerCashEvent {
        if value > &self.get_cash_balance() {
            info!(
                "BROKER: Debit failed of {:?} cash, current balance of {:?}",
                value, self.get_cash_balance()
            );
            return BrokerCashEvent::WithdrawFailure(CashValue::from(*value));
        }
        info!(
            "BROKER: Debited {:?} cash, current balance of {:?}",
            value, self.get_cash_balance()
        );
        self.update_cash_balance(CashValue::from(*self.get_cash_balance() - *value));
        BrokerCashEvent::WithdrawSuccess(CashValue::from(*value))
    }

    fn debit_force(&mut self, value: &f64) -> BrokerCashEvent {
        info!(
            "BROKER: Force debt {:?} cash, current balance of {:?}",
            value, self.get_cash_balance()
        );
        self.update_cash_balance(CashValue::from(*self.get_cash_balance() - *value));
        BrokerCashEvent::WithdrawSuccess(CashValue::from(*value))
    }
}

trait BrokerOperations: Portfolio + BrokerStates + SendOrder + CashOperations {
    /// If current round of trades have caused broker to run out of cash then this will rebalance.
    ///
    /// Has a fixed value buffer, currently set to 1000, to reduce the probability of the broker
    /// moving into an insufficient cash state.
    fn rebalance_cash(&mut self) {
        //Has to be less than, we can have zero value without needing to liquidate if we initialize
        //the portfolio but exchange doesn't execute any trades. This can happen if we are missing
        //prices at the start of the series
        if *self.get_cash_balance() < 0.0 {
            let shortfall = *self.get_cash_balance() * -1.0;
            //When we raise cash, we try to raise a small amount more to stop continuous
            //rebalancing, this amount is arbitrary atm
            let plus_buffer = shortfall + 1000.0;

            let res = self.withdraw_cash_with_liquidation(&plus_buffer);
            if let BrokerCashEvent::WithdrawFailure(_val) = res {
                //The broker tried to generate cash required but was unable to do so. Stop all
                //further mutations, and run out the current portfolio state to return some
                //value to strategy
                self.update_broker_state(BrokerState::Failed);
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
    fn withdraw_cash_with_liquidation(&mut self, cash: &f64) -> BrokerCashEvent {
        // TODO: is it better to return a sequence of orders to achieve a cash balance? Because
        // of the linkage with execution, we need seperate methods for sync/async.
        info!("BROKER: Withdrawing {:?} with liquidation", cash);
        let value = self.get_liquidation_value();
        if cash > &value {
            //There is no way for the portfolio to recover, we leave the portfolio in an invalid
            //state because the client may be able to recover later
            self.debit(cash);
            info!(
                "BROKER: Failed to withdraw {:?} with liquidation. Deducting value from cash.",
                cash
            );
            BrokerCashEvent::WithdrawFailure(CashValue::from(*cash))
        } else {
            //This holds how much we have left to generate from the portfolio to produce the cash
            //required
            let mut total_sold = *cash;

            let positions = self.get_positions();
            let mut sell_orders: Vec<UistOrder> = Vec::new();
            for ticker in positions {
                let position_value = self
                    .get_position_value(&ticker)
                    .unwrap_or(CashValue::from(0.0));
                //Position won't generate enough cash to fulfill total order
                //Create orders for selling 100% of position, continue
                //to next position to see if we can generate enough cash
                //
                //Sell 100% of position
                if *position_value <= total_sold {
                    //Cannot be called without qty existing
                    if let Some(qty) = self.get_position_qty(&ticker) {
                        let order = UistOrder::market_sell(ticker, *qty);
                        info!("BROKER: Withdrawing {:?} with liquidation, queueing sale of {:?} shares of {:?}", cash, order.get_shares(), order.get_symbol());
                        sell_orders.push(order);
                        total_sold -= *position_value;
                    }
                } else {
                    //Position can generate all the cash we need
                    //Create orders to sell 100% of position, don't continue to next stock
                    //
                    //Cannot be called without quote existing so unwrap
                    let quote = self.get_quote(&ticker).unwrap();
                    let price = quote.get_bid();
                    let shares_req = PortfolioQty::from((total_sold / price).ceil());
                    let order = UistOrder::market_sell(ticker, *shares_req);
                    info!("BROKER: Withdrawing {:?} with liquidation, queueing sale of {:?} shares of {:?}", cash, order.get_shares(), order.get_symbol());
                    sell_orders.push(order);
                    total_sold = 0.0;
                    break;
                }
            }
            if (total_sold).eq(&0.0) {
                //The portfolio can provide enough cash so we can execute the sell orders
                //We leave the portfolio in the wrong state for the client to deal with
                self.send_orders(&sell_orders);
                info!("BROKER: Succesfully withdrew {:?} with liquidation", cash);
                BrokerCashEvent::WithdrawSuccess(CashValue::from(*cash))
            } else {
                //For whatever reason, we went through the above process and were unable to find
                //the cash. Don't send any orders, leave portfolio in invalid state for client to
                //potentially recover.
                self.debit(cash);
                info!(
                    "BROKER: Failed to withdraw {:?} with liquidation. Deducting value from cash.",
                    cash
                );
                BrokerCashEvent::WithdrawFailure(CashValue::from(*cash))
            }
        }
    }

    fn client_has_sufficient_cash(
        &self,
        order: &UistOrder,
        price: &Price,
    ) -> Result<(), InsufficientCashError> {
        let shares = order.get_shares();
        let value = CashValue::from(shares * **price);
        match order.get_order_type() {
            UistOrderType::MarketBuy => {
                if self.get_cash_balance() > value {
                    return Ok(());
                }
                Err(InsufficientCashError)
            }
            UistOrderType::MarketSell => Ok(()),
            _ => unreachable!("Shouldn't hit unless something has gone wrong"),
        }
    }

    fn client_has_sufficient_holdings_for_sale(
        &self,
        order: &UistOrder,
    ) -> Result<(), UnexecutableOrderError> {
        if let UistOrderType::MarketSell = order.get_order_type() {
            if let Some(holding) = self.get_position_qty(order.get_symbol()) {
                if *holding >= order.get_shares() {
                    return Ok(());
                } else {
                    return Err(UnexecutableOrderError);
                }
            }
        }
        Ok(())
    }

    fn client_is_issuing_nonsense_order(
        &self,
        order: &UistOrder,
    ) -> Result<(), UnexecutableOrderError> {
        let shares = order.get_shares();
        if shares == 0.0 {
            return Err(UnexecutableOrderError);
        }
        Ok(())
    }

    /// Calculates difference between current broker state and a target allocation, the latter
    /// typically passed from a strategy.
    ///
    /// Brokers do not expect target wights, they merely respond to orders so this structure
    /// is not required to create backtests.
    fn diff_brkr_against_target_weights(
        &mut self,
        target_weights: &PortfolioAllocation,
    ) -> Vec<UistOrder> {
        //Returns orders so calling function has control over when orders are executed
        //Requires mutable reference to brkr because it calls get_position_value
        //Need liquidation value so we definitely have enough money to make all transactions after
        //costs
        info!("STRATEGY: Calculating diff of current allocation vs. target");
        let total_value = self.get_liquidation_value();
        if (*total_value).eq(&0.0) {
            panic!("Client is attempting to trade a portfolio with zero value");
        }
        let mut orders: Vec<UistOrder> = Vec::new();

        let mut buy_orders: Vec<UistOrder> = Vec::new();
        let mut sell_orders: Vec<UistOrder> = Vec::new();

        //This returns a positive number for buy and negative for sell, this is necessary because
        //of calculations made later to find the net position of orders on the exchange.
        let calc_required_shares_with_costs =
            |diff_val: &f64, quote: &UistQuote, brkr: &Self| -> f64 {
                if diff_val.lt(&0.0) {
                    let price = quote.get_bid();
                    let costs = brkr.calc_trade_impact(&diff_val.abs(), &price, false);
                    let total = (*costs.0 / *costs.1).floor();
                    -total
                } else {
                    let price = quote.get_ask();
                    let costs = brkr.calc_trade_impact(&diff_val.abs(), &price, true);
                    (*costs.0 / *costs.1).floor()
                }
            };

        for symbol in target_weights.keys() {
            let curr_val = self
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
            if let Some(quote) = self.get_quote(&symbol) {
                //This will be negative if the net is selling
                let required_shares = calc_required_shares_with_costs(&diff_val, &quote, self);
                //TODO: must be able to clear pending orders
                //Clear any pending orders on the exchange
                //self.clear_pending_market_orders_by_symbol(&symbol);
                if required_shares.ne(&0.0) {
                    if required_shares.gt(&0.0) {
                        buy_orders.push(UistOrder::market_buy(symbol.clone(), required_shares));
                    } else {
                        sell_orders.push(UistOrder::market_sell(
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
}