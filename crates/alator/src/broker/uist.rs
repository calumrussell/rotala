use std::{collections::HashMap, error::Error, fmt::{Display, Formatter}};
use itertools::Itertools;

use rotala::clock::DateTime;
use log::info;
use rotala::exchange::uist::{Uist, UistQuote, UistTradeType, UistTrade, UistOrderType, UistOrder};

use crate::types::{PortfolioHoldings, Price, CashValue, PortfolioQty, PortfolioValues, PortfolioAllocation};

use super::types::BrokerCost;

#[derive(Debug)]
enum BrokerState {
    Ready,
    Failed,
}

#[derive(Debug)]
pub struct UistBroker {
    cash: CashValue,
    exchange: Uist,
    holdings: PortfolioHoldings,
    //Kept distinct from holdings because some perf calculations may need to distinguish between
    //trades that we know are booked vs ones that we think should get booked
    pending_orders: PortfolioHoldings,
    //Used to mark last trade seen by broker when reconciling completed trades with exchange
    last_seen_trade: usize,
    latest_quotes: HashMap<String, UistQuote>,
    log: UistBrokerLog,
    trade_costs: Vec<BrokerCost>,
    broker_state: BrokerState,
}

impl UistBroker {
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

    pub fn get_total_value(&self) -> CashValue {
        let assets = self.get_positions();
        let mut value = self.get_cash_balance();
        for a in assets {
            if let Some(position_value) = self.get_position_value(&a) {
                value = CashValue::from(*value + *position_value);
            }
        }
        value
    }

    pub fn get_liquidation_value(&self) -> CashValue {
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
    pub fn cost_basis(&self, symbol: &str) -> Option<Price> {
        self.log.cost_basis(symbol)
    }

    pub fn get_holdings_with_pending(&self) -> PortfolioHoldings {
        let mut merged_holdings = PortfolioHoldings::new();
        for (key, value) in self.holdings.0.iter() {
            if merged_holdings.0.contains_key(key) {
                if let Some(val) = merged_holdings.get(key) {
                    let new_val = PortfolioQty::from(*val + **value);
                    merged_holdings.insert(key, &new_val);
                }
            } else {
                merged_holdings.insert(key, value);
            }
        }

        for (key, value) in self.pending_orders.0.iter() {
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

    /// Called on every tick of clock to ensure that state is synchronized with other components.
    ///
    /// * Calls `check` on exchange
    /// * Updates last seen prices for exchange tick
    /// * Reconciles internal state against trades completed on current tick
    /// * Rebalances cash, which can trigger new trades if broker is in invalid state
    pub fn check(&mut self) {
        self.exchange.check();

        //Update prices, these prices are not tradable
        for quote in &self.exchange.fetch_quotes() {
            self.latest_quotes
                .insert(quote.get_symbol().to_string(), quote.clone());
        }

        //Reconcile broker against executed trades
        let completed_trades = self.exchange.fetch_trades(self.last_seen_trade).to_owned();
        for trade in completed_trades {
            match trade.typ {
                //Force debit so we can end up with negative cash here
                UistTradeType::Buy => self.debit_force(&trade.value),
                UistTradeType::Sell => self.credit(&trade.value),
            };
            self.log.record::<UistTrade>(trade.clone().into());

            let curr_position = self.get_position_qty(&trade.symbol).unwrap_or_default();

            let updated = match trade.typ {
                UistTradeType::Buy => *curr_position + trade.quantity,
                UistTradeType::Sell => *curr_position - trade.quantity,
            };
            self.update_holdings(&trade.symbol, PortfolioQty::from(updated));

            //Because the order has completed, we should be able to unwrap pending_orders safetly
            //If this fails then there must be an application bug and panic is required.
            let pending = self.pending_orders.get(&trade.symbol).unwrap_or_default();

            let updated_pending = match trade.typ {
                UistTradeType::Buy => *pending - trade.quantity,
                UistTradeType::Sell => *pending + trade.quantity,
            };
            if updated_pending == 0.0 {
                self.pending_orders.remove(&trade.symbol);
            } else {
                self.pending_orders
                    .insert(&trade.symbol, &PortfolioQty::from(updated_pending));
            }

            self.last_seen_trade += 1;
        }
        //Previous step can cause negative cash balance so we have to rebalance here, this
        //is not instant so will never balance properly if the series is very volatile
        self.rebalance_cash();
    }

    /// If current round of trades have caused broker to run out of cash then this will rebalance.
    ///
    /// Has a fixed value buffer, currently set to 1000, to reduce the probability of the broker
    /// moving into an insufficient cash state.
    fn rebalance_cash(&mut self) {
        //Has to be less than, we can have zero value without needing to liquidate if we initialize
        //the portfolio but exchange doesn't execute any trades. This can happen if we are missing
        //prices at the start of the series
        if *self.cash < 0.0 {
            let shortfall = *self.cash * -1.0;
            //When we raise cash, we try to raise a small amount more to stop continuous
            //rebalancing, this amount is arbitrary atm
            let plus_buffer = shortfall + 1000.0;

            let res = self.withdraw_cash_with_liquidation(&plus_buffer);
            if let UistBrokerCashEvent::WithdrawFailure(_val) = res {
                //The broker tried to generate cash required but was unable to do so. Stop all
                //further mutations, and run out the current portfolio state to return some
                //value to strategy
                self.broker_state = BrokerState::Failed;
            }
        }
    }

    fn get_quote(&self, symbol: &str) -> Option<UistQuote> {
        self.latest_quotes.get(symbol).cloned()
    }

    fn get_quotes(&self) -> Option<Vec<UistQuote>> {
        if self.latest_quotes.is_empty() {
            return None;
        }

        let mut tmp = Vec::new();
        for quote in self.latest_quotes.values() {
            tmp.push(quote.clone());
        }
        Some(tmp)
    }

    fn update_holdings(&mut self, symbol: &str, change: PortfolioQty) {
        //We have to take ownership for logging but it is easier just to use ref for symbol as that
        //is used throughout
        let symbol_own = symbol.to_string();
        info!(
            "BROKER: Incrementing holdings in {:?} by {:?}",
            symbol_own, change
        );
        if (*change).eq(&0.0) {
            self.holdings.remove(symbol.as_ref());
        } else {
            self.holdings.insert(symbol.as_ref(), &change);
        }
    }

    fn get_position_cost(&self, symbol: &str) -> Option<Price> {
        self.log.cost_basis(symbol)
    }

    pub fn withdraw_cash(&mut self, cash: &f64) -> UistBrokerCashEvent {
        match self.broker_state {
            BrokerState::Failed => {
                info!(
                    "BROKER: Attempted cash withdraw of {:?} but broker in Failed State",
                    cash,
                );
                UistBrokerCashEvent::OperationFailure(CashValue::from(*cash))
            }
            BrokerState::Ready => {
                if cash > &self.get_cash_balance() {
                    info!(
                        "BROKER: Attempted cash withdraw of {:?} but only have {:?}",
                        cash,
                        self.get_cash_balance()
                    );
                    return UistBrokerCashEvent::WithdrawFailure(CashValue::from(*cash));
                }
                info!(
                    "BROKER: Successful cash withdraw of {:?}, {:?} left in cash",
                    cash,
                    self.get_cash_balance()
                );
                self.debit(cash);
                UistBrokerCashEvent::WithdrawSuccess(CashValue::from(*cash))
            }
        }
    }

    pub fn deposit_cash(&mut self, cash: &f64) -> UistBrokerCashEvent {
        match self.broker_state {
            BrokerState::Failed => {
                info!(
                    "BROKER: Attempted cash deposit of {:?} but broker in Failed State",
                    cash,
                );
                UistBrokerCashEvent::OperationFailure(CashValue::from(*cash))
            }
            BrokerState::Ready => {
                info!(
                    "BROKER: Deposited {:?} cash, current balance of {:?}",
                    cash,
                    self.get_cash_balance()
                );
                self.credit(cash);
                UistBrokerCashEvent::DepositSuccess(CashValue::from(*cash))
            }
        }
    }

    //Identical to deposit_cash but is seperated to distinguish internal cash
    //transactions from external with no value returned to client
    fn credit(&mut self, value: &f64) -> UistBrokerCashEvent {
        info!(
            "BROKER: Credited {:?} cash, current balance of {:?}",
            value, self.cash
        );
        self.cash = CashValue::from(*value + *self.cash);
        UistBrokerCashEvent::DepositSuccess(CashValue::from(*value))
    }

    //Looks similar to withdraw_cash but distinguished because it represents
    //failure of an internal transaction with no value returned to clients
    fn debit(&mut self, value: &f64) -> UistBrokerCashEvent {
        if value > &self.cash {
            info!(
                "BROKER: Debit failed of {:?} cash, current balance of {:?}",
                value, self.cash
            );
            return UistBrokerCashEvent::WithdrawFailure(CashValue::from(*value));
        }
        info!(
            "BROKER: Debited {:?} cash, current balance of {:?}",
            value, self.cash
        );
        self.cash = CashValue::from(*self.cash - *value);
        UistBrokerCashEvent::WithdrawSuccess(CashValue::from(*value))
    }

    fn debit_force(&mut self, value: &f64) -> UistBrokerCashEvent {
        info!(
            "BROKER: Force debt {:?} cash, current balance of {:?}",
            value, self.cash
        );
        self.cash = CashValue::from(*self.cash - *value);
        UistBrokerCashEvent::WithdrawSuccess(CashValue::from(*value))
    }

    fn get_cash_balance(&self) -> CashValue {
        self.cash.clone()
    }

    fn get_holdings(&self) -> PortfolioHoldings {
        self.holdings.clone()
    }

    fn get_trade_costs(&self, trade: &UistTrade) -> CashValue {
        let mut cost = CashValue::default();
        for trade_cost in &self.trade_costs {
            cost = CashValue::from(*cost + *trade_cost.calc(trade.clone()));
        }
        cost
    }

    fn calc_trade_impact(&self, budget: &f64, price: &f64, is_buy: bool) -> (CashValue, Price) {
        BrokerCost::trade_impact_total(&self.trade_costs, budget, price, is_buy)
    }

    pub fn send_order(&mut self, order: UistOrder) -> UistBrokerEvent {
        //This is an estimate of the cost based on the current price, can still end with negative
        //balance when we reconcile with actuals, may also reject valid orders at the margin
        match self.broker_state {
            BrokerState::Failed => {
                info!(
                    "BROKER: Unable to send {:?} order for {:?} shares of {:?} to exchange as broker in Failed state",
                    order.get_order_type(),
                    order.get_shares(),
                    order.get_symbol()
                );
                UistBrokerEvent::OrderInvalid(order.clone())
            }
            BrokerState::Ready => {
                info!(
                    "BROKER: Attempting to send {:?} order for {:?} shares of {:?} to the exchange",
                    order.get_order_type(),
                    order.get_shares(),
                    order.get_symbol()
                );

                let quote = self.get_quote(order.get_symbol()).unwrap();
                let price = match order.get_order_type() {
                    UistOrderType::MarketBuy | UistOrderType::LimitBuy | UistOrderType::StopBuy => {
                        quote.get_ask()
                    }
                    UistOrderType::MarketSell | UistOrderType::LimitSell | UistOrderType::StopSell => {
                        quote.get_bid()
                    }
                };

                if let Err(_err) = self.client_has_sufficient_cash( &order, &Price::from(price)) {
                    info!(
                        "BROKER: Unable to send {:?} order for {:?} shares of {:?} to exchange",
                        order.get_order_type(),
                        order.get_shares(),
                        order.get_symbol()
                    );
                    return UistBrokerEvent::OrderInvalid(order.clone());
                }
                if let Err(_err) = self.client_has_sufficient_holdings_for_sale(&order)
                {
                    info!(
                        "BROKER: Unable to send {:?} order for {:?} shares of {:?} to exchange",
                        order.get_order_type(),
                        order.get_shares(),
                        order.get_symbol()
                    );
                    return UistBrokerEvent::OrderInvalid(order.clone());
                }
                if let Err(_err) = self.client_is_issuing_nonsense_order(&order) {
                    info!(
                        "BROKER: Unable to send {:?} order for {:?} shares of {:?} to exchange",
                        order.get_order_type(),
                        order.get_shares(),
                        order.get_symbol()
                    );
                    return UistBrokerEvent::OrderInvalid(order.clone());
                }

                self.exchange.insert_order(order.clone());
                //From the point of view of strategy, an order pending is the same as an order
                //executed. If the order is executed, then it is executed. If the order isn't
                //executed then the strategy must wait but all the strategy's work has been
                //done. So once we send the order, we need some way for clients to work out
                //what orders are pending and whether they need to do more work.
                let order_effect = match order.get_order_type() {
                    UistOrderType::MarketBuy | UistOrderType::LimitBuy | UistOrderType::StopBuy => {
                        order.get_shares()
                    }

                    UistOrderType::MarketSell | UistOrderType::LimitSell | UistOrderType::StopSell => {
                        -order.get_shares()
                    }
                };

                if let Some(position) = self.pending_orders.get(order.get_symbol()) {
                    let existing = *position + order_effect;
                    self.pending_orders
                        .insert(order.get_symbol(), &PortfolioQty::from(existing));
                } else {
                    self.pending_orders
                        .insert(order.get_symbol(), &PortfolioQty::from(order_effect));
                }
                info!(
                    "BROKER: Successfully sent {:?} order for {:?} shares of {:?} to exchange",
                    order.get_order_type(),
                    order.get_shares(),
                    order.get_symbol()
                );
                UistBrokerEvent::OrderSentToExchange(order)
            }
        }
    }

    pub fn send_orders(&mut self, orders: &[UistOrder]) -> Vec<UistBrokerEvent> {
        let mut res = Vec::new();
        for o in orders {
            let trade = self.send_order(o.clone());
            res.push(trade);
        }
        res
    }

    /// Withdrawing with liquidation will queue orders to generate the expected amount of cash. No
    /// ordering to the assets that are sold, the broker is responsible for managing cash but not
    /// re-aligning to a target portfolio.
    ///
    /// Because orders are not executed instaneously this method can be the source of significant
    /// divergences in performance from the underlying in certain cases. For example, if prices are
    /// volatile, in the case of low-frequency data, then the broker will end up continuously
    /// re-balancing in a random way under certain price movements.
    pub fn withdraw_cash_with_liquidation(&mut self, cash: &f64) -> UistBrokerCashEvent {
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
            UistBrokerCashEvent::WithdrawFailure(CashValue::from(*cash))
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
                    let order =
                        UistOrder::market_sell(ticker, *shares_req);
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
                UistBrokerCashEvent::WithdrawSuccess(CashValue::from(*cash))
            } else {
                //For whatever reason, we went through the above process and were unable to find
                //the cash. Don't send any orders, leave portfolio in invalid state for client to
                //potentially recover.
                self.debit(cash);
                info!(
                    "BROKER: Failed to withdraw {:?} with liquidation. Deducting value from cash.",
                    cash
                );
                UistBrokerCashEvent::WithdrawFailure(CashValue::from(*cash))
            }
        }
    }

    /// Calculates difference between current broker state and a target allocation, the latter
    /// typically passed from a strategy.
    ///
    /// Brokers do not expect target wights, they merely respond to orders so this structure
    /// is not required to create backtests.
    pub fn diff_brkr_against_target_weights( &mut self, target_weights: &PortfolioAllocation) -> Vec<UistOrder> {
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
        let calc_required_shares_with_costs = |diff_val: &f64, quote: &UistQuote, brkr: &UistBroker| -> f64 {
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
                        buy_orders.push(UistOrder::market_buy(
                            symbol.clone(),
                            required_shares,
                        ));
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

    pub fn client_has_sufficient_cash( &self, order: &UistOrder, price: &Price) -> Result<(), InsufficientCashError> {
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

    pub fn client_has_sufficient_holdings_for_sale(&self, order: &UistOrder) -> Result<(), UnexecutableOrderError> {
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

    pub fn client_is_issuing_nonsense_order(&self, order: &UistOrder,) -> Result<(), UnexecutableOrderError> {
        let shares = order.get_shares();
        if shares == 0.0 {
            return Err(UnexecutableOrderError);
        }
        Ok(())
    }

    pub fn trades_between(&self, start: &i64, stop: &i64) -> Vec<UistTrade> {
        self.log.trades_between(start, stop)
    }

}

pub struct UistBrokerBuilder {
    trade_costs: Vec<BrokerCost>,
    exchange: Option<Uist>,
}

impl UistBrokerBuilder {
    pub fn build(&mut self) -> UistBroker {
        if self.exchange.is_none() {
            panic!("Cannot build broker without exchange");
        }

        //If we don't have quotes on first tick, we shouldn't error but we should expect every
        //`DataSource` to provide a first tick
        let mut first_quotes = HashMap::new();
        let quotes = self.exchange.as_ref().unwrap().fetch_quotes();
        for quote in &quotes {
            first_quotes.insert(quote.get_symbol().to_string(), quote.clone());
        }

        let holdings = PortfolioHoldings::new();
        let pending_orders = PortfolioHoldings::new();
        let log = UistBrokerLog::new();

        let exchange = std::mem::take(&mut self.exchange).unwrap();

        UistBroker {
            //Intialised as invalid so errors throw if client tries to run before init
            holdings,
            pending_orders,
            cash: CashValue::from(0.0),
            log,
            last_seen_trade: 0,
            exchange,
            trade_costs: self.trade_costs.clone(),
            latest_quotes: first_quotes,
            broker_state: BrokerState::Ready,
        }
    }

    pub fn with_exchange(&mut self, exchange: Uist) -> &mut Self {
        self.exchange = Some(exchange);
        self
    }

    pub fn with_trade_costs(&mut self, trade_costs: Vec<BrokerCost>) -> &mut Self {
        self.trade_costs = trade_costs;
        self
    }

    pub fn new() -> Self {
        UistBrokerBuilder {
            trade_costs: Vec::new(),
            exchange: None,
        }
    }
}

impl Default for UistBrokerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub enum UistRecordedEvent {
    TradeCompleted(UistTrade)
}

impl From<UistTrade> for UistRecordedEvent {
    fn from(value: UistTrade) -> Self {
        UistRecordedEvent::TradeCompleted(value)
    } 
}

//Records events generated by brokers. Used for internal calculations but is public for tax
//calculations.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct UistBrokerLog {
    log: Vec<UistRecordedEvent>,
}

impl UistBrokerLog {
    pub fn record<E: Into<UistRecordedEvent>>(&mut self, event: E) {
        let brokerevent: UistRecordedEvent = event.into();
        self.log.push(brokerevent);
    }

    pub fn trades(&self) -> Vec<UistTrade> {
        let mut trades = Vec::new();
        for event in &self.log {
            if let UistRecordedEvent::TradeCompleted(trade) = event {
                trades.push(trade.clone());
            }
        }
        trades
    }

    pub fn trades_between(&self, start: &i64, stop: &i64) -> Vec<UistTrade> {
        let trades = self.trades();
        trades
            .iter()
            .filter(|v| v.date >= *DateTime::from(*start) && v.date <= *DateTime::from(*stop))
            .cloned()
            .collect_vec()
    }

    pub fn cost_basis(&self, symbol: &str) -> Option<Price> {
        let mut cum_qty = PortfolioQty::default();
        let mut cum_val = CashValue::default();
        for event in &self.log {
            if let UistRecordedEvent::TradeCompleted(trade) = event {
                if trade.symbol.eq(symbol) {
                    match trade.typ {
                        UistTradeType::Buy => {
                            cum_qty = PortfolioQty::from(*cum_qty + trade.quantity.clone());
                            cum_val = CashValue::from(*cum_val + trade.value.clone());
                        }
                        UistTradeType::Sell => {
                            cum_qty = PortfolioQty::from(*cum_qty - trade.quantity.clone());
                            cum_val = CashValue::from(*cum_val - trade.value.clone());
                        }
                    }
                    //reset the value if we are back to zero
                    if (*cum_qty).eq(&0.0) {
                        cum_val = CashValue::default();
                    }
                }
            }
        }
        if (*cum_qty).eq(&0.0) {
            return None;
        }
        Some(Price::from(*cum_val / *cum_qty))
    }
}

impl UistBrokerLog {
    pub fn new() -> Self {
        UistBrokerLog { log: Vec::new() }
    }
}

impl Default for UistBrokerLog {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub enum UistBrokerEvent {
    OrderSentToExchange(UistOrder),
    OrderInvalid(UistOrder),
    OrderCreated(UistOrder),
    OrderFailure(UistOrder),
}

#[derive(Clone, Debug)]
pub enum UistBrokerCashEvent {
    //Removed from [UistBrokerEvent] because there are situations when we want to handle these events
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

#[cfg(test)]
mod tests {

    use crate::broker::types::BrokerCost;
    use crate::types::{CashValue, PortfolioQty, PortfolioAllocation};
    use rotala::clock::{ClockBuilder, Frequency};
    use rotala::exchange::uist::{Uist, UistTrade, UistTradeType, UistOrder, random_uist_generator, UistOrderType};
    use rotala::input::penelope::PenelopeBuilder;

    use super::{UistBroker, UistBrokerBuilder, UistBrokerLog, UistBrokerEvent, UistBrokerCashEvent};

    fn setup() -> UistBroker {
        let mut source_builder = PenelopeBuilder::new();

        source_builder.add_quote(100.00, 101.00, 100, "ABC");
        source_builder.add_quote(10.00, 11.00, 100, "BCD");

        source_builder.add_quote(104.00, 105.00, 101, "ABC");
        source_builder.add_quote(14.00, 15.00, 101, "BCD");

        source_builder.add_quote(95.00, 96.00, 102, "ABC");
        source_builder.add_quote(10.00, 11.00, 102, "BCD");

        source_builder.add_quote(95.00, 96.00, 103, "ABC");
        source_builder.add_quote(10.00, 11.00, 103, "BCD");

        let (price_source, clock) = source_builder.build_with_frequency(rotala::clock::Frequency::Second);
        let uist = Uist::new(clock, price_source);

        let brkr = UistBrokerBuilder::new()
            .with_exchange(uist)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build();

        brkr
    }

    #[test]
    fn test_cash_deposit_withdraw() {
        let mut brkr = setup();
        brkr.deposit_cash(&100.0);

        brkr.check();

        //Test cash
        assert!(matches!(
            brkr.withdraw_cash(&50.0),
            UistBrokerCashEvent::WithdrawSuccess(..)
        ));
        assert!(matches!(
            brkr.withdraw_cash(&51.0),
            UistBrokerCashEvent::WithdrawFailure(..)
        ));
        assert!(matches!(
            brkr.deposit_cash(&50.0),
            UistBrokerCashEvent::DepositSuccess(..)
        ));

        //Test transactions
        assert!(matches!(
            brkr.debit(&50.0),
            UistBrokerCashEvent::WithdrawSuccess(..)
        ));
        assert!(matches!(
            brkr.debit(&51.0),
            UistBrokerCashEvent::WithdrawFailure(..)
        ));
        assert!(matches!(
            brkr.credit(&50.0),
            UistBrokerCashEvent::DepositSuccess(..)
        ));
    }

    #[test]
    fn test_that_buy_order_reduces_cash_and_increases_holdings() {
        let mut brkr = setup();
        brkr.deposit_cash(&100_000.0);

        let res = brkr.send_order(UistOrder::market_buy("ABC", 495.0));
        println!("{:?}", res);
        assert!(matches!(res, UistBrokerEvent::OrderSentToExchange(..)));

        brkr.check();

        let cash = brkr.get_cash_balance();
        assert!(*cash < 100_000.0);

        let qty = brkr
            .get_position_qty("ABC")
            .unwrap_or(PortfolioQty::from(0.0));
        assert_eq!(*qty.clone(), 495.00);
    }

    #[test]
    fn test_that_buy_order_larger_than_cash_fails_with_error_returned_without_panic() {
        let mut brkr = setup();
        brkr.deposit_cash(&100.0);
        //Order value is greater than cash balance
        let res = brkr.send_order(UistOrder::market_buy("ABC", 495.0));

        assert!(matches!(res, UistBrokerEvent::OrderInvalid(..)));
        brkr.check();

        let cash = brkr.get_cash_balance();
        assert!(*cash == 100.0);
    }

    #[test]
    fn test_that_sell_order_larger_than_holding_fails_with_error_returned_without_panic() {
        let mut brkr = setup();
        brkr.deposit_cash(&100_000.0);

        let res = brkr.send_order(UistOrder::market_buy("ABC", 100.0));
        assert!(matches!(res, UistBrokerEvent::OrderSentToExchange(..)));
        brkr.check();

        //Order greater than current holding
        brkr.check();

        let res = brkr.send_order(UistOrder::market_sell("ABC", 105.0));
        assert!(matches!(res, UistBrokerEvent::OrderInvalid(..)));

        //Checking that
        let qty = brkr.get_position_qty("ABC").unwrap_or_default();
        println!("{:?}", qty);
        assert!((*qty.clone()).eq(&100.0));
    }

    #[test]
    fn test_that_market_sell_increases_cash_and_decreases_holdings() {
        let mut brkr = setup();
        brkr.deposit_cash(&100_000.0);
        let res = brkr.send_order(UistOrder::market_buy("ABC", 495.0));
        assert!(matches!(res, UistBrokerEvent::OrderSentToExchange(..)));
        brkr.check();
        let cash = brkr.get_cash_balance();

        brkr.check();

        let res = brkr.send_order(UistOrder::market_sell("ABC", 295.0));
        assert!(matches!(res, UistBrokerEvent::OrderSentToExchange(..)));

        brkr.check();
        let cash0 = brkr.get_cash_balance();

        let qty = brkr.get_position_qty("ABC").unwrap_or_default();
        assert_eq!(*qty, 200.0);
        assert!(*cash0 > *cash);
    }

    #[test]
    fn test_that_valuation_updates_in_next_period() {
        let mut brkr = setup();
        brkr.deposit_cash(&100_000.0);

        brkr.send_order(UistOrder::market_buy("ABC", 495.0));
        brkr.check();

        let val = brkr.get_position_value("ABC");

        brkr.check();
        let val1 = brkr.get_position_value("ABC");
        assert_ne!(val, val1);
    }

    #[test]
    fn test_that_profit_calculation_is_accurate() {
        let mut brkr = setup();
        brkr.deposit_cash(&100_000.0);
        brkr.send_order(UistOrder::market_buy("ABC", 495.0));
        brkr.check();

        brkr.check();

        let profit = brkr.get_position_profit("ABC").unwrap();
        assert_eq!(*profit, -4950.00);
    }

    #[test]
    fn test_that_broker_build_passes_without_trade_costs() {
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(100.00, 101.00, 100, "ABC");
        source_builder.add_quote(104.00, 105.00, 101, "ABC");
        source_builder.add_quote(95.00, 96.00, 102, "ABC");

        let (price_source, clock) = source_builder.build_with_frequency(rotala::clock::Frequency::Second);
        let uist = Uist::new(clock, price_source);

        let _brkr = UistBrokerBuilder::new()
            .with_exchange(uist)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build();
    }

    #[test]
    fn test_that_broker_uses_last_value_if_it_fails_to_find_quote() {
        //If the broker cannot find a quote in the current period for a stock, it automatically
        //uses a value of zero. This is a problem because the current time could a weekend or
        //bank holiday, and if the broker is attempting to value the portfolio on that day
        //they will ask for a quote, not find one, and then use a value of zero which is
        //incorrect.
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(100.00, 101.00, 100, "ABC");
        source_builder.add_quote(10.00, 11.00, 100, "BCD");

        //Trades execute here
        source_builder.add_quote(100.00, 101.00, 101, "ABC");
        source_builder.add_quote(10.00, 11.00, 101, "BCD");

        //We are missing a quote for BCD on 101, but the broker should return the last seen value
        source_builder.add_quote(104.00, 105.00, 102, "ABC");

        //And when we check the next date, it updates correctly
        source_builder.add_quote(104.00, 105.00, 103, "ABC");
        source_builder.add_quote(12.00, 13.00, 103, "BCD");

        let (price_source, clock) = source_builder.build_with_frequency(rotala::clock::Frequency::Second);
        let uist = Uist::new(clock, price_source);

        let mut brkr = UistBrokerBuilder::new()
            .with_exchange(uist)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build();

        brkr.deposit_cash(&100_000.0);

        brkr.send_order(UistOrder::market_buy("ABC", 100.0));
        brkr.send_order(UistOrder::market_buy("BCD", 100.0));

        brkr.check();

        //Missing live quote for BCD
        brkr.check();
        let value = brkr
            .get_position_value("BCD")
            .unwrap_or(CashValue::from(0.0));
        println!("{:?}", value);
        //We test against the bid price, which gives us the value exclusive of the price paid at ask
        assert!(*value == 10.0 * 100.0);

        //BCD has quote again
        brkr.check();

        let value1 = brkr
            .get_position_value("BCD")
            .unwrap_or(CashValue::from(0.0));
        println!("{:?}", value1);
        assert!(*value1 == 12.0 * 100.0);
    }

    #[test]
    fn test_that_broker_handles_negative_cash_balance_due_to_volatility() {
        //Because orders sent to the exchange are not executed instantaneously it is possible for a
        //broker to issue an order for a stock, the price to fall/rise before the trade gets
        //executed, and the broker end up with more/less cash than expected.
        //
        //For example, if orders are issued for 100% of the portfolio then if prices rises then we
        //can end up with negative balances.

        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(100.00, 101.00, 100, "ABC");
        source_builder.add_quote(150.00, 151.00, 101, "ABC");
        source_builder.add_quote(150.00, 151.00, 102, "ABC");

        let (price_source, clock) = source_builder.build_with_frequency(rotala::clock::Frequency::Second);
        let uist = Uist::new(clock, price_source);

        let mut brkr = UistBrokerBuilder::new()
            .with_exchange(uist)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build();

        brkr.deposit_cash(&100_000.0);
        //Because the price of ABC rises after this order is sent, we will end up with a negative
        //cash balance after the order is executed
        brkr.send_order(UistOrder::market_buy("ABC", 700.0));

        //Trades execute
        brkr.check();

        let cash = brkr.get_cash_balance();
        assert!(*cash < 0.0);

        //Broker rebalances to raise cash
        brkr.check();
        let cash1 = brkr.get_cash_balance();
        assert!(*cash1 > 0.0);
    }

    #[test]
    fn test_that_broker_stops_when_liquidation_fails() {
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(100.00, 101.00, 100, "ABC");
        //Price doubles over one tick so that the broker is trading on information that has become
        //very inaccurate
        source_builder.add_quote(200.00, 201.00, 101, "ABC");
        source_builder.add_quote(200.00, 201.00, 101, "ABC");

        let (price_source, clock) = source_builder.build_with_frequency(rotala::clock::Frequency::Second);
        let uist = Uist::new(clock, price_source);

        let mut brkr = UistBrokerBuilder::new()
            .with_exchange(uist)
            .with_trade_costs(vec![BrokerCost::PctOfValue(0.01)])
            .build();

        brkr.deposit_cash(&100_000.0);
        //This will use all the available cash balance, the market price doubles so the broker ends
        //up with a shortfall of -100_000.

        brkr.send_order(UistOrder::market_buy("ABC", 990.0));

        brkr.check();
        brkr.check();
        brkr.check();

        let cash = brkr.get_cash_balance();
        assert!(*cash < 0.0);

        let res = brkr.send_order(UistOrder::market_buy("ABC", 100.0));
        assert!(matches!(res, UistBrokerEvent::OrderInvalid { .. }));

        assert!(matches!(
            brkr.deposit_cash(&100_000.0),
            UistBrokerCashEvent::OperationFailure { .. }
        ));
        assert!(matches!(
            brkr.withdraw_cash(&100_000.0),
            UistBrokerCashEvent::OperationFailure { .. }
        ));
    }

    #[test]
    fn test_that_holdings_updates_correctly() {
        let mut brkr = setup();
        brkr.deposit_cash(&100_000.0);
        let res = brkr.send_order(UistOrder::market_buy("ABC", 50.0));
        assert!(matches!(res, UistBrokerEvent::OrderSentToExchange(..)));
        assert_eq!(
            *brkr
                .get_holdings_with_pending()
                .get("ABC")
                .unwrap_or_default(),
            50.0
        );
        brkr.check();
        assert_eq!(*brkr.get_holdings().get("ABC").unwrap_or_default(), 50.0);

        let res = brkr.send_order(UistOrder::market_sell("ABC", 10.0));
        assert!(matches!(res, UistBrokerEvent::OrderSentToExchange(..)));
        assert_eq!(
            *brkr
                .get_holdings_with_pending()
                .get("ABC")
                .unwrap_or_default(),
            40.0
        );
        brkr.check();
        assert_eq!(*brkr.get_holdings().get("ABC").unwrap_or_default(), 40.0);

        let res = brkr.send_order(UistOrder::market_buy("ABC", 50.0));
        assert!(matches!(res, UistBrokerEvent::OrderSentToExchange(..)));
        assert_eq!(
            *brkr
                .get_holdings_with_pending()
                .get("ABC")
                .unwrap_or_default(),
            90.0
        );
        brkr.check();
        assert_eq!(*brkr.get_holdings().get("ABC").unwrap_or_default(), 90.0)
    }

    fn setup_log() -> UistBrokerLog {
        let mut rec = UistBrokerLog::new();

        let t1 = UistTrade::new("ABC", 100.0, 10.00, 100, UistTradeType::Buy);
        let t2 = UistTrade::new("ABC", 500.0, 90.00, 101, UistTradeType::Buy);
        let t3 = UistTrade::new("BCD", 100.0, 100.0, 102, UistTradeType::Buy);
        let t4 = UistTrade::new("BCD", 500.0, 100.00, 103, UistTradeType::Sell);
        let t5 = UistTrade::new("BCD", 50.0, 50.00, 104, UistTradeType::Buy);

        rec.record(t1);
        rec.record(t2);
        rec.record(t3);
        rec.record(t4);
        rec.record(t5);
        rec
    }

    #[test]
    fn test_that_log_filters_trades_between_dates() {
        let log = setup_log();
        let between = log.trades_between(&102.into(), &104.into());
        assert!(between.len() == 3);
    }

    #[test]
    fn test_that_log_calculates_the_cost_basis() {
        let log = setup_log();
        let abc_cost = log.cost_basis("ABC").unwrap();
        let bcd_cost = log.cost_basis("BCD").unwrap();

        assert_eq!(*abc_cost, 6.0);
        assert_eq!(*bcd_cost, 1.0);
    }

    #[test]
    fn diff_direction_correct_if_need_to_buy() {
        let uist = random_uist_generator(100);
        let mut brkr = UistBrokerBuilder::new()
            .with_trade_costs(vec![BrokerCost::flat(1.0)])
            .with_exchange(uist)
            .build();

        let mut weights = PortfolioAllocation::new();
        weights.insert("ABC", 1.0);

        brkr.deposit_cash(&100_000.0);
        brkr.check();

        let orders = brkr.diff_brkr_against_target_weights(&weights);

        println!("{:?}", orders);
        let first = orders.first().unwrap();
        assert!(matches!(
            first.get_order_type(),
            UistOrderType::MarketBuy { .. }
        ));
    }

    #[test]
    fn diff_direction_correct_if_need_to_sell() {
        //This is connected to the previous test, if the above fails then this will never pass.
        //However, if the above passes this could still fail.

        let uist = random_uist_generator(100);
        let mut brkr = UistBrokerBuilder::new()
            .with_trade_costs(vec![BrokerCost::flat(1.0)])
            .with_exchange(uist)
            .build();

        let mut weights = PortfolioAllocation::new();
        weights.insert("ABC", 1.0);

        brkr.deposit_cash(&100_000.0);
        let orders = brkr.diff_brkr_against_target_weights(&weights);
        brkr.send_orders(&orders);

        brkr.check();

        brkr.check();

        let mut weights1 = PortfolioAllocation::new();
        //This weight needs to very small because it is possible for the data generator to generate
        //a price that drops significantly meaning that rebalancing requires a buy not a sell. This
        //is unlikely but seems to happen eventually.
        weights1.insert("ABC", 0.01);
        let orders1 = brkr.diff_brkr_against_target_weights(&weights1);

        println!("{:?}", orders1);
        let first = orders1.first().unwrap();
        assert!(matches!(
            first.get_order_type(),
            UistOrderType::MarketSell { .. }
        ));
    }

    #[test]
    fn diff_continues_if_security_missing() {
        //In this scenario, the user has inserted incorrect information but this scenario can also occur if there is no quote
        //for a given security on a certain date. We are interested in the latter case, not the former but it is more
        //difficult to test for the latter, and the code should be the same.
        let uist = random_uist_generator(100);
        let mut brkr = UistBrokerBuilder::new()
            .with_trade_costs(vec![BrokerCost::flat(1.0)])
            .with_exchange(uist)
            .build();

        let mut weights = PortfolioAllocation::new();
        weights.insert("ABC", 0.5);
        //There is no quote for this security in the underlying data, code should make the assumption (that doesn't apply here)
        //that there is some quote for this security at a later date and continues to generate order for ABC without throwing
        //error
        weights.insert("XYZ", 0.5);

        brkr.deposit_cash(&100_000.0);
        brkr.check();
        let orders = brkr.diff_brkr_against_target_weights(&weights);
        assert!(orders.len() == 1);
    }

    #[test]
    #[should_panic]
    fn diff_panics_if_brkr_has_no_cash() {
        //If we get to a point where the client is diffing without cash, we can assume that no further operations are possible
        //and we should panic
        let uist = random_uist_generator(100);
        let mut brkr = UistBrokerBuilder::new()
            .with_trade_costs(vec![BrokerCost::flat(1.0)])
            .with_exchange(uist)
            .build();

        let mut weights = PortfolioAllocation::new();
        weights.insert("ABC", 1.0);

        brkr.check();
        brkr.diff_brkr_against_target_weights(&weights);
    }

    #[test]
    fn can_estimate_trade_costs_of_proposed_trade() {
        let pershare = BrokerCost::per_share(0.1);
        let flat = BrokerCost::flat(10.0);
        let pct = BrokerCost::pct_of_value(0.01);

        let res = pershare.trade_impact(&1000.0, &1.0, true);
        assert!((*res.1).eq(&1.1));

        let res = pershare.trade_impact(&1000.0, &1.0, false);
        assert!((*res.1).eq(&0.9));

        let res = flat.trade_impact(&1000.0, &1.0, true);
        assert!((*res.0).eq(&990.00));

        let res = pct.trade_impact(&100.0, &1.0, true);
        assert!((*res.0).eq(&99.0));

        let costs = vec![pershare, flat];
        let initial = BrokerCost::trade_impact_total(&costs, &1000.0, &1.0, true);
        assert!((*initial.0).eq(&990.00));
        assert!((*initial.1).eq(&1.1));
    }

    #[test]
    fn diff_handles_sent_but_unexecuted_orders() {
        //It is possible for the client to issue orders for infinitely increasing numbers of shares
        //if there is a gap between orders being issued and executed. For example, if we are
        //missing price data the client could think we need 100 shares, that order doesn't get
        //executed on the next tick, and the client then issues orders for another 100 shares.
        //
        //This is not possible without earlier price data either. If there is no price data then
        //the diff will be unable to work out how many shares are required. So the test case is
        //some price but no price for the execution period.
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(100.00, 100.00, 100, "ABC");
        source_builder.add_quote(100.00, 100.00, 101, "ABC");
        source_builder.add_quote(100.00, 100.00, 103, "ABC");

        let (price_source, clock) = source_builder.build_with_frequency(rotala::clock::Frequency::Second);
        let uist = Uist::new(clock, price_source);

        let mut brkr = UistBrokerBuilder::new()
            .with_exchange(uist)
            .build();

        brkr.deposit_cash(&100_000.0);

        //No price for security so we haven't diffed correctly
        brkr.check();

        brkr.check();

        let mut target_weights = PortfolioAllocation::new();
        target_weights.insert("ABC", 0.9);

        let orders = brkr.diff_brkr_against_target_weights(&target_weights);
        brkr.send_orders(&orders);

        brkr.check();

        let orders1 = brkr.diff_brkr_against_target_weights(&target_weights);

        brkr.send_orders(&orders1);
        brkr.check();

        dbg!(brkr.get_position_qty("ABC"));
        //If the logic isn't correct the orders will have doubled up to 1800
        assert_eq!(*brkr.get_position_qty("ABC").unwrap(), 900.0);
    }

    #[tokio::test]
    async fn diff_handles_case_when_existing_order_requires_sell_to_rebalance() {
        //Tests similar scenario to previous test but for the situation in which the price is
        //missing, and we try to rebalance by buying but the pending order is for a significantly
        //greater amount of shares than we now need (e.g. we have a price of X, we miss a price,
        //and then it drops 20%).
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(100.00, 100.00, 100, "ABC");
        source_builder.add_quote(75.00, 75.00, 103, "ABC");
        source_builder.add_quote(75.00, 75.00, 104, "ABC");

        let (price_source, clock) = source_builder.build_with_frequency(rotala::clock::Frequency::Second);
        let uist = Uist::new(clock, price_source);

        let mut brkr = UistBrokerBuilder::new()
            .with_exchange(uist)
            .build();

        brkr.deposit_cash(&100_000.0);

        let mut target_weights = PortfolioAllocation::new();
        target_weights.insert("ABC", 0.9);
        let orders = brkr.diff_brkr_against_target_weights(&target_weights);
        println!("{:?}", orders);

        brkr.send_orders(&orders);

        //No price for security so we haven't diffed correctly
        brkr.check();

        brkr.check();

        brkr.check();

        let orders1 = brkr.diff_brkr_against_target_weights(&target_weights);
        println!("{:?}", orders1);

        brkr.send_orders(&orders1);

        brkr.check();

        println!("{:?}", brkr.get_holdings());
        //If the logic isn't correct then the order will be for less shares than is actually
        //required by the newest price
        assert_eq!(*brkr.get_position_qty("ABC").unwrap(), 1200.0);
    }
}
