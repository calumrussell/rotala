use log::info;
use std::error::Error;
use std::fmt::Formatter;
use std::{cmp::Ordering, fmt::Display};

use crate::types::{
    CashValue, DateTime, PortfolioAllocation, PortfolioHoldings, PortfolioQty, PortfolioValues,
    Price,
};

pub mod record;

//Contains data structures and traits that refer solely to the data held and operations required
//for broker implementations.

/// Represents a point-in-time quote of both sides of the market (bid+offer) from an exchange.
///
/// Equality checked against ticker and date. Ordering against date only.
///
/// let q = Quote::new(
///   10.0,
///   11.0,
///   100,
///   "ABC"
/// );
///
#[derive(Clone, Debug)]
pub struct Quote {
    //TODO: more indirection is needed for this type, possibly implemented as trait
    pub bid: Price,
    pub ask: Price,
    pub date: DateTime,
    pub symbol: String,
}

impl Quote {
    pub fn new(
        bid: impl Into<Price>,
        ask: impl Into<Price>,
        date: impl Into<DateTime>,
        symbol: impl Into<String>,
    ) -> Self {
        Self {
            bid: bid.into(),
            ask: ask.into(),
            date: date.into(),
            symbol: symbol.into(),
        }
    }
}

impl Ord for Quote {
    fn cmp(&self, other: &Self) -> Ordering {
        self.date.cmp(&other.date)
    }
}

impl PartialOrd for Quote {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Quote {}

impl PartialEq for Quote {
    fn eq(&self, other: &Self) -> bool {
        self.date == other.date && self.symbol == other.symbol
    }
}

///Represents a single dividend payment in per-share terms.
///
///Equality checked against ticker and date. Ordering against date only.
///
///let d = Dividend::new(
///  0.1,
///  "ABC"
///  100,
///);
#[derive(Clone, Debug)]
pub struct Dividend {
    //Dividend value is expressed in terms of per share values
    pub value: Price,
    pub symbol: String,
    pub date: DateTime,
}

impl Dividend {
    pub fn new(
        value: impl Into<Price>,
        symbol: impl Into<String>,
        date: impl Into<DateTime>,
    ) -> Self {
        Self {
            value: value.into(),
            symbol: symbol.into(),
            date: date.into(),
        }
    }
}

impl Ord for Dividend {
    fn cmp(&self, other: &Self) -> Ordering {
        self.date.cmp(&other.date)
    }
}

impl PartialOrd for Dividend {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Dividend {}

impl PartialEq for Dividend {
    fn eq(&self, other: &Self) -> bool {
        self.date == other.date && self.symbol == other.symbol
    }
}

///Represents a single dividend payment in cash terms. Type is used internally within broker and
///is used only to credit the cash balance. Shouldn't be used outside a broker impl.
///
///Equality checked against ticker and date. Ordering against date only.
///
///let dp = DividendPayment::new(
///  0.1,
///  "ABC",
///  100,
///);
#[derive(Clone, Debug)]
pub struct DividendPayment {
    pub value: CashValue,
    pub symbol: String,
    pub date: DateTime,
}

impl DividendPayment {
    pub fn new(
        value: impl Into<CashValue>,
        symbol: impl Into<String>,
        date: impl Into<DateTime>,
    ) -> Self {
        Self {
            value: value.into(),
            symbol: symbol.into(),
            date: date.into(),
        }
    }
}

impl Ord for DividendPayment {
    fn cmp(&self, other: &Self) -> Ordering {
        self.date.cmp(&other.date)
    }
}

impl PartialOrd for DividendPayment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for DividendPayment {}

impl PartialEq for DividendPayment {
    fn eq(&self, other: &Self) -> bool {
        self.date == other.date && self.symbol == other.symbol
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TradeType {
    Buy,
    Sell,
}

///Represents a completed trade to be stored in the internal broker impl ledger or used by the
///client. This type is a pure internal representation, and clients do not pass trades to the
///broker to execute but pass an [Order] instaed.
///
///Equality checked against ticker, date, and quantity. Ordering against date only.
///
///let t = Trade::new(
///  "ABC",
///  100.0,
///  1000,
///  100,
///  TradeType::Buy,
///);
#[derive(Clone, Debug)]
pub struct Trade {
    //TODO: more indirection is needed for this type, possibly implemented as trait
    pub symbol: String,
    pub value: CashValue,
    pub quantity: PortfolioQty,
    pub date: DateTime,
    pub typ: TradeType,
}

impl Trade {
    pub fn new(
        symbol: impl Into<String>,
        value: impl Into<CashValue>,
        quantity: impl Into<PortfolioQty>,
        date: impl Into<DateTime>,
        typ: TradeType,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            value: value.into(),
            quantity: quantity.into(),
            date: date.into(),
            typ,
        }
    }
}

impl Ord for Trade {
    fn cmp(&self, other: &Self) -> Ordering {
        self.date.cmp(&other.date)
    }
}

impl PartialOrd for Trade {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Trade {}

impl PartialEq for Trade {
    fn eq(&self, other: &Self) -> bool {
        self.date == other.date && self.symbol == other.symbol
    }
}

///Events generated by broker in the course of executing transactions.
///
///Brokers have two sources of state: holdings of stock and cash. Events represent modifications of
///that state over time. The vast majority, but not all, of these events could be returned to client
///applications.
#[derive(Clone, Debug)]
pub enum BrokerEvent {
    OrderSentToExchange(Order),
    OrderInvalid(Order),
    OrderCreated(Order),
    OrderFailure(Order),
}

#[derive(Clone, Debug)]
pub enum BrokerCashEvent {
    //Removed from [BrokerEvent] because there are situations when we want to handle these events
    //specifically and seperately
    WithdrawSuccess(CashValue),
    WithdrawFailure(CashValue),
    DepositSuccess(CashValue),
}

///Events generated by broker in the course of executing internal transactions.
///
///These events will typically only be used internally to return information to clients. In
///practice, these are currently used to record taxable events.
#[derive(Clone, Debug)]
pub enum BrokerRecordedEvent {
    TradeCompleted(Trade),
    DividendPaid(DividendPayment),
}

impl From<Trade> for BrokerRecordedEvent {
    fn from(trade: Trade) -> Self {
        BrokerRecordedEvent::TradeCompleted(trade)
    }
}

impl From<DividendPayment> for BrokerRecordedEvent {
    fn from(divi: DividendPayment) -> Self {
        BrokerRecordedEvent::DividendPaid(divi)
    }
}

///Represents the order types that a broker implementation should support.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderType {
    MarketSell,
    MarketBuy,
    LimitSell,
    LimitBuy,
    StopSell,
    StopBuy,
}

///Represents an order that is sent to a broker to execute. Trading strategies can send orders to
///brokers to execute. In practice, trading strategies typically target [PortfolioAllocation] but
///these allocations are just wrappers around [Order] that we diff against with the trading logic.
///
///Current execution model is to execute orders instaneously so there is no functional difference
///between a trade and a order: all orders eventually become trades. At some point, it is likely
///that the library moves away from this model so it makes sense to distinguish here between orders
///and trades.
///
///Equality checked against ticker, order_type, and quantity. No ordering.
///
///let o = Order::market(
///  OrderType::MarketBuy,
///  "ABC",
///  100.0,
///);
///
///let o1 = Order::delayed(
///  OrderType::StopSell,
///  "ABC",
///  100.0,
///  10.0,
///);
#[derive(Clone, Debug)]
pub struct Order {
    order_type: OrderType,
    symbol: String,
    shares: PortfolioQty,
    price: Option<Price>,
}

impl Order {
    //TODO: should this be a trait?
    pub fn get_symbol(&self) -> &String {
        &self.symbol
    }

    pub fn get_shares(&self) -> &PortfolioQty {
        &self.shares
    }

    pub fn get_price(&self) -> &Option<Price> {
        &self.price
    }

    pub fn get_order_type(&self) -> &OrderType {
        &self.order_type
    }

    pub fn market(
        order_type: OrderType,
        symbol: impl Into<String>,
        shares: impl Into<PortfolioQty>,
    ) -> Self {
        Self {
            order_type,
            symbol: symbol.into(),
            shares: shares.into(),
            price: None,
        }
    }

    pub fn delayed(
        order_type: OrderType,
        symbol: impl Into<String>,
        shares: impl Into<PortfolioQty>,
        price: impl Into<Price>,
    ) -> Self {
        Self {
            order_type,
            symbol: symbol.into(),
            shares: shares.into(),
            price: Some(price.into()),
        }
    }
}

impl Eq for Order {}

impl PartialEq for Order {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
            && self.order_type == other.order_type
            && self.shares == other.shares
    }
}

///Implementation of various cost models for brokers. Broker implementations would either define or
///cost model or would provide the user the option of intializing one; the broker impl would then
///call the variant's calculation methods as trades are executed.
#[derive(Clone, Debug)]
pub enum BrokerCost {
    PerShare(Price),
    PctOfValue(f64),
    Flat(CashValue),
}

impl BrokerCost {
    pub fn per_share(val: f64) -> Self {
        BrokerCost::PerShare(Price::from(val))
    }

    pub fn pct_of_value(val: f64) -> Self {
        BrokerCost::PctOfValue(val)
    }

    pub fn flat(val: f64) -> Self {
        BrokerCost::Flat(CashValue::from(val))
    }

    pub fn calc(&self, trade: &Trade) -> CashValue {
        match self {
            BrokerCost::PerShare(cost) => CashValue::from(*cost.clone() * *trade.quantity.clone()),
            BrokerCost::PctOfValue(pct) => CashValue::from(*trade.value * *pct),
            BrokerCost::Flat(val) => val.clone(),
        }
    }

    //Returns a valid trade given trading costs given a current budget
    //and price of security
    pub fn trade_impact(
        &self,
        gross_budget: &f64,
        gross_price: &f64,
        is_buy: bool,
    ) -> (CashValue, Price) {
        let mut net_budget = *gross_budget;
        let mut net_price = *gross_price;
        match self {
            BrokerCost::PerShare(val) => {
                if is_buy {
                    net_price += *val.clone();
                } else {
                    net_price -= *val.clone();
                }
            }
            BrokerCost::PctOfValue(pct) => {
                net_budget *= 1.0 - pct;
            }
            BrokerCost::Flat(val) => net_budget -= *val.clone(),
        }
        (CashValue::from(net_budget), Price::from(net_price))
    }

    pub fn trade_impact_total(
        trade_costs: &[BrokerCost],
        gross_budget: &f64,
        gross_price: &f64,
        is_buy: bool,
    ) -> (CashValue, Price) {
        let mut res = (CashValue::from(*gross_budget), Price::from(*gross_price));
        for cost in trade_costs {
            res = cost.trade_impact(&res.0, &res.1, is_buy);
        }
        res
    }
}

//Key traits for broker implementations.
//
//Whilst broker is implemented within this package as a singular broker, the intention of these
//traits is to hide the implementation from the user so that it could be one or a combination of
//brokers returning the data. Similarly, strategy implementations should not create any
//dependencies on the underlying state of the broker.
//
///Represents functionality that all brokers have to support in order to perform any backtests at
///all. Implementations may choose not to implement some part of this functionality but this trait
///represents a general base case.
///
///In practice, the only optional trait that seems to be included often is [GetsQuote]. This is not
///included in the base definition because turning the broker into both a source and user of data
///is implementation-dependent.
///
///This confusion in the implementation is also why get_position_value requires mutation: broker
///not only asks for prices but keeps state about prices so that we can find a valuation for the
///security if we are missing a price for the current date. At some point, we make relax this
///mutability constraint.
///
///Clients should not be able to call debit or credit themselves. Deposits or withdrawals are
///implemented through [TransferCash] trait.
pub trait BacktestBroker {
    fn get_position_profit(&self, symbol: &str) -> Option<CashValue> {
        if let Some(cost) = self.get_position_cost(symbol) {
            if let Some(position_value) = self.get_position_value(symbol) {
                if let Some(qty) = self.get_position_qty(symbol) {
                    let price = *position_value / *qty.clone();
                    let value = CashValue::from(*qty.clone() * (price - *cost));
                    return Some(value);
                }
            }
        }
        None
    }

    fn get_position_liquidation_value(&self, symbol: &str) -> Option<CashValue> {
        //TODO: we need to introduce some kind of distinction between short and long
        //      positions.
        if let Some(position_value) = self.get_position_value(symbol) {
            if let Some(qty) = self.get_position_qty(symbol) {
                let price = Price::from(*position_value / **qty);
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
            let value = self.get_position_value(&a);
            if let Some(v) = value {
                holdings.insert(&a, &v);
            }
        }
        holdings
    }

    fn get_cash_balance(&self) -> CashValue;
    //TODO: Position qty can always return a value, if we don't have the position then qty is 0
    fn get_position_qty(&self, symbol: &str) -> Option<&PortfolioQty>;
    //TODO: Position value can always return a value, if we don't have a position then value is 0
    fn get_position_value(&self, symbol: &str) -> Option<CashValue>;
    fn get_position_cost(&self, symbol: &str) -> Option<Price>;
    fn get_positions(&self) -> Vec<String>;
    fn get_holdings(&self) -> PortfolioHoldings;
    //This should only be called internally
    fn get_trade_costs(&self, trade: &Trade) -> CashValue;
    fn calc_trade_impact(&self, budget: &f64, price: &f64, is_buy: bool) -> (CashValue, Price);
    fn update_holdings(&mut self, symbol: &str, change: PortfolioQty);
    fn pay_dividends(&mut self);
    fn send_order(&mut self, order: Order) -> BrokerEvent;
    fn send_orders(&mut self, order: &[Order]) -> Vec<BrokerEvent>;
    fn clear_pending_market_orders_by_symbol(&mut self, symbol: &str);
    fn debit(&mut self, value: &f64) -> BrokerCashEvent;
    fn credit(&mut self, value: &f64) -> BrokerCashEvent;
    //Can leave the client with a negative cash balance
    fn debit_force(&mut self, value: &f64) -> BrokerCashEvent;
}

///Implementation allows clients to alter the cash balance through withdrawing or depositing cash.
///This does not come with base implementation because clients may wish to restrict this behaviour.
pub trait TransferCash: BacktestBroker {
    fn withdraw_cash(&mut self, cash: &f64) -> BrokerCashEvent {
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

    fn deposit_cash(&mut self, cash: &f64) -> BrokerCashEvent {
        info!(
            "BROKER: Deposited {:?} cash, current balance of {:?}",
            cash,
            self.get_cash_balance()
        );
        self.credit(cash);
        BrokerCashEvent::DepositSuccess(CashValue::from(*cash))
    }
}

//Implementation allows clients to retrieve prices. This trait may be used to retrieve prices
//internally too, and this confusion comes from broker implementations being both a consumer and
//source of data. So this trait is seperated out now but may disappear in future versions.
pub trait GetsQuote {
    fn get_quote(&self, symbol: &str) -> Option<&Quote>;
    fn get_quotes(&self) -> Option<&[Quote]>;
}

///Implementation allows clients to query properties of the transaction history of the broker.
///Again, this is an optional feature but is useful for things like tax calculations.
///
///When using this note that it offers operations that are distinct in purpose from a performance
///calculation. Performance statistics are created at the end of a backtest but the intention here
///is to provide a view into transactions whilst the simulation is still running i.e. for tax
///calculations.
pub trait EventLog {
    fn trades_between(&self, start: &i64, end: &i64) -> Vec<Trade>;
    fn dividends_between(&self, start: &i64, end: &i64) -> Vec<DividendPayment>;
}

///Implements functionality that is standard to most brokers. These calculations are generic so are
///compiled into functionality for the implementation at run-time. Brokers do not necessarily need
///to use this logic but it represents functionality that is common to implementations that we use
///now.
pub struct BrokerCalculations;

impl BrokerCalculations {
    //Withdrawing with liquidation will execute orders in order to generate the target amount of cash
    //required.
    //
    //This function should be used relatively sparingly because it breaks the update cycle between
    //`Strategy` and `Broker`: the orders are not executed in any particular order so the state within
    //`Broker` is left in a random state, which may not be immediately clear to clients and can cause
    //significant unexpected drift in performance if this function is called repeatedly with long
    //rebalance cycles.
    //
    //The primary use-case for this functionality is for clients that implement tax payments: these are
    //mandatory reductions in cash that have to be paid before the simulation can proceed to the next
    //valid state.
    pub fn withdraw_cash_with_liquidation<T: BacktestBroker + GetsQuote>(
        cash: &f64,
        brkr: &mut T,
    ) -> BrokerCashEvent {
        //TODO:should this execute any trades at all? Would it be better to return a sequence of orders
        //required to achieve the cash balance, and then leave it up to the calling function to decide
        //whether to execute?
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
            BrokerCashEvent::WithdrawFailure(CashValue::from(*cash))
        } else {
            //This holds how much we have left to generate from the portfolio to produce the cash
            //required
            let mut total_sold = *cash;

            let positions = brkr.get_positions();
            let mut sell_orders: Vec<Order> = Vec::new();
            for ticker in positions {
                let position_value = brkr.get_position_value(&ticker).unwrap_or_default();
                //Position won't generate enough cash to fulfill total order
                //Create orders for selling 100% of position, continue
                //to next position to see if we can generate enough cash
                //
                //Sell 100% of position
                if *position_value <= total_sold {
                    //Cannot be called without qty existing
                    let qty = brkr.get_position_qty(&ticker).unwrap();
                    let order = Order::market(OrderType::MarketSell, ticker, qty.clone());
                    info!("BROKER: Withdrawing {:?} with liquidation, queueing sale of {:?} shares of {:?}", cash, order.get_shares(), order.get_symbol());
                    sell_orders.push(order);
                    total_sold -= *position_value;
                } else {
                    //Position can generate all the cash we need
                    //Create orders to sell 100% of position, don't continue to next stock
                    //
                    //Cannot be called without quote existing so unwrap
                    let price = &brkr.get_quote(&ticker).unwrap().bid;
                    let shares_req = PortfolioQty::from((total_sold / **price).ceil());
                    let order = Order::market(OrderType::MarketSell, ticker, shares_req);
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
                BrokerCashEvent::WithdrawSuccess(CashValue::from(*cash))
            } else {
                //For whatever reason, we went through the above process and were unable to find
                //the cash. Don't send any orders, leave portfolio in invalid state for client to
                //potentially recover.
                brkr.debit(cash);
                info!(
                    "BROKER: Failed to withdraw {:?} with liquidation. Deducting value from cash.",
                    cash
                );
                BrokerCashEvent::WithdrawFailure(CashValue::from(*cash))
            }
        }
    }

    //Calculates the diff between the current state of the portfolio within broker, and the
    //target_weights passed into the function.
    //Returns orders so calling function has control over when orders are executed
    //Requires mutable reference to brkr because it calls get_position_value
    pub fn diff_brkr_against_target_weights<T: BacktestBroker + GetsQuote>(
        target_weights: &PortfolioAllocation,
        brkr: &mut T,
    ) -> Vec<Order> {
        //Need liquidation value so we definitely have enough money to make all transactions after
        //costs
        info!("STRATEGY: Calculating diff of current allocation vs. target");
        let total_value = brkr.get_liquidation_value();
        if (*total_value).eq(&0.0) {
            panic!("Client is attempting to trade a portfolio with zero value");
        }
        let mut orders: Vec<Order> = Vec::new();

        let mut buy_orders: Vec<Order> = Vec::new();
        let mut sell_orders: Vec<Order> = Vec::new();

        //This returns a positive number for buy and negative for sell, this is necessary because
        //of calculations made later to find the net position of orders on the exchange.
        let calc_required_shares_with_costs = |diff_val: &f64, quote: &Quote, brkr: &T| -> f64 {
            if diff_val.lt(&0.0) {
                let price = &quote.bid;
                let costs = brkr.calc_trade_impact(&diff_val.abs(), price, false);
                let total = (*costs.0 / *costs.1).floor();
                -total
            } else {
                let price = &quote.ask;
                let costs = brkr.calc_trade_impact(&diff_val.abs(), price, true);
                (*costs.0 / *costs.1).floor()
            }
        };

        for symbol in target_weights.keys() {
            let curr_val = brkr.get_position_value(&symbol).unwrap_or_default();
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
                let required_shares = calc_required_shares_with_costs(&diff_val, quote, brkr);
                //Clear any pending orders on the exchange
                brkr.clear_pending_market_orders_by_symbol(&symbol);
                if required_shares.ne(&0.0) {
                    if required_shares.gt(&0.0) {
                        buy_orders.push(Order::market(
                            OrderType::MarketBuy,
                            symbol.clone(),
                            required_shares,
                        ));
                    } else {
                        sell_orders.push(Order::market(
                            OrderType::MarketSell,
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

    pub fn client_has_sufficient_cash(
        order: &Order,
        price: &Price,
        brkr: &impl BacktestBroker,
    ) -> Result<(), InsufficientCashError> {
        let shares = order.get_shares();
        let value = CashValue::from(**shares * **price);
        match order.get_order_type() {
            OrderType::MarketBuy => {
                if brkr.get_cash_balance() > value {
                    return Ok(());
                }
                Err(InsufficientCashError)
            }
            OrderType::MarketSell => Ok(()),
            _ => unreachable!("Shouldn't hit unless something has gone wrong"),
        }
    }

    pub fn client_has_sufficient_holdings_for_sale(
        order: &Order,
        brkr: &impl BacktestBroker,
    ) -> Result<(), UnexecutableOrderError> {
        if let OrderType::MarketSell = order.get_order_type() {
            if let Some(holding) = brkr.get_position_qty(order.get_symbol()) {
                if *holding >= order.shares {
                    return Ok(());
                }
            }
            Err(UnexecutableOrderError)
        } else {
            Ok(())
        }
    }

    pub fn client_is_issuing_nonsense_order(order: &Order) -> Result<(), UnexecutableOrderError> {
        let shares = **order.get_shares();
        if shares == 0.0 {
            return Err(UnexecutableOrderError);
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct InsufficientCashError;

impl Error for InsufficientCashError {}

impl Display for InsufficientCashError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "Client has insufficient cash to execute order")
    }
}

#[derive(Debug, Clone)]
pub struct UnexecutableOrderError;

impl Error for UnexecutableOrderError {}

impl Display for UnexecutableOrderError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "Client has passed unexecutable order")
    }
}

#[cfg(test)]
mod tests {

    use crate::broker::{BacktestBroker, OrderType};
    use crate::exchange::DefaultExchangeBuilder;
    use crate::input::{fake_data_generator, HashMapInputBuilder};
    use crate::sim::SimulatedBrokerBuilder;
    use crate::types::{DateTime, PortfolioAllocation};
    use crate::{clock::ClockBuilder, types::Frequency};
    use std::collections::HashMap;
    use std::rc::Rc;

    use super::{BrokerCalculations, BrokerCost, Quote, TransferCash};

    #[test]
    fn diff_direction_correct_if_need_to_buy() {
        let clock = ClockBuilder::with_length_in_days(0, 10)
            .with_frequency(&Frequency::Daily)
            .build();
        let input = fake_data_generator(Rc::clone(&clock));

        let exchange = DefaultExchangeBuilder::new()
            .with_data_source(input.clone())
            .with_clock(Rc::clone(&clock))
            .build();

        let mut brkr = SimulatedBrokerBuilder::new()
            .with_data(input)
            .with_exchange(exchange)
            .build();

        let mut weights = PortfolioAllocation::new();
        weights.insert("ABC", 1.0);

        brkr.deposit_cash(&100_000.0);
        clock.borrow_mut().tick();
        brkr.finish();

        let orders = BrokerCalculations::diff_brkr_against_target_weights(&weights, &mut brkr);
        println!("{:?}", orders);
        let first = orders.first().unwrap();
        assert!(matches!(first.order_type, OrderType::MarketBuy { .. }));
    }

    #[test]
    fn diff_direction_correct_if_need_to_sell() {
        //This is connected to the previous test, if the above fails then this will never pass.
        //However, if the above passes this could still fail.
        let clock = ClockBuilder::with_length_in_days(0, 10)
            .with_frequency(&Frequency::Daily)
            .build();

        let input = fake_data_generator(Rc::clone(&clock));

        let exchange = DefaultExchangeBuilder::new()
            .with_data_source(input.clone())
            .with_clock(Rc::clone(&clock))
            .build();

        let mut brkr = SimulatedBrokerBuilder::new()
            .with_data(input)
            .with_exchange(exchange)
            .build();

        let mut weights = PortfolioAllocation::new();
        weights.insert("ABC", 1.0);

        brkr.deposit_cash(&100_000.0);
        let orders = BrokerCalculations::diff_brkr_against_target_weights(&weights, &mut brkr);
        brkr.send_orders(&orders);
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();

        let mut weights1 = PortfolioAllocation::new();
        //This weight needs to very small because it is possible for the data generator to generate
        //a price that drops significantly meaning that rebalancing requires a buy not a sell. This
        //is unlikely but seems to happen eventually.
        weights1.insert("ABC", 0.01);
        let orders1 = BrokerCalculations::diff_brkr_against_target_weights(&weights1, &mut brkr);

        println!("{:?}", orders1);
        let first = orders1.first().unwrap();
        assert!(matches!(first.order_type, OrderType::MarketSell { .. }));
    }

    #[test]
    fn diff_continues_if_security_missing() {
        //In this scenario, the user has inserted incorrect information but this scenario can also occur if there is no quote
        //for a given security on a certain date. We are interested in the latter case, not the former but it is more
        //difficult to test for the latter, and the code should be the same.
        let clock = ClockBuilder::with_length_in_days(0, 10)
            .with_frequency(&Frequency::Daily)
            .build();

        let input = fake_data_generator(Rc::clone(&clock));

        let exchange = DefaultExchangeBuilder::new()
            .with_data_source(input.clone())
            .with_clock(Rc::clone(&clock))
            .build();

        let mut brkr = SimulatedBrokerBuilder::new()
            .with_data(input)
            .with_exchange(exchange)
            .build();

        let mut weights = PortfolioAllocation::new();
        weights.insert("ABC", 0.5);
        //There is no quote for this security in the underlying data, code should make the assumption (that doesn't apply here)
        //that there is some quote for this security at a later date and continues to generate order for ABC without throwing
        //error
        weights.insert("XYZ", 0.5);

        brkr.deposit_cash(&100_000.0);
        clock.borrow_mut().tick();
        let orders = BrokerCalculations::diff_brkr_against_target_weights(&weights, &mut brkr);
        assert!(orders.len() == 1);
    }

    #[test]
    #[should_panic]
    fn diff_panics_if_brkr_has_no_cash() {
        //If we get to a point where the client is diffing without cash, we can assume that no further operations are possible
        //and we should panic
        let clock = ClockBuilder::with_length_in_days(0, 10)
            .with_frequency(&Frequency::Daily)
            .build();
        let input = fake_data_generator(Rc::clone(&clock));

        let exchange = DefaultExchangeBuilder::new()
            .with_data_source(input.clone())
            .with_clock(Rc::clone(&clock))
            .build();

        let mut brkr = SimulatedBrokerBuilder::new()
            .with_data(input)
            .with_exchange(exchange)
            .build();

        let mut weights = PortfolioAllocation::new();
        weights.insert("ABC", 1.0);

        clock.borrow_mut().tick();
        BrokerCalculations::diff_brkr_against_target_weights(&weights, &mut brkr);
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
        let mut prices: HashMap<DateTime, Vec<Quote>> = HashMap::new();
        let quote = Quote::new(100.00, 100.00, 100, "ABC");
        let quote1 = Quote::new(100.00, 100.00, 101, "ABC");
        let quote2 = Quote::new(100.00, 100.00, 103, "ABC");

        prices.insert(100.into(), vec![quote]);
        prices.insert(101.into(), vec![quote1]);
        prices.insert(102.into(), vec![]);
        prices.insert(103.into(), vec![quote2]);

        let clock = ClockBuilder::with_length_in_seconds(100, 5)
            .with_frequency(&Frequency::Second)
            .build();

        let source = HashMapInputBuilder::new()
            .with_quotes(prices)
            .with_clock(Rc::clone(&clock))
            .build();

        let exchange = DefaultExchangeBuilder::new()
            .with_clock(Rc::clone(&clock))
            .with_data_source(source.clone())
            .build();

        let mut brkr = SimulatedBrokerBuilder::new()
            .with_data(source)
            .with_exchange(exchange)
            .build();

        brkr.deposit_cash(&100_000.0);
        brkr.finish();

        //No price for security so we haven't diffed correctly
        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();

        let mut target_weights = PortfolioAllocation::new();
        target_weights.insert("ABC", 0.9);

        let orders =
            BrokerCalculations::diff_brkr_against_target_weights(&target_weights, &mut brkr);
        brkr.send_orders(&orders);
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();

        let orders1 =
            BrokerCalculations::diff_brkr_against_target_weights(&target_weights, &mut brkr);

        brkr.send_orders(&orders1);
        brkr.finish();

        //If the logic isn't correct the orders will have doubled up to 1800
        assert_eq!(*(*brkr.get_position_qty("ABC").unwrap()), 900.0);
    }

    #[test]
    fn diff_handles_case_when_existing_order_requires_sell_to_rebalance() {
        //Tests similar scenario to previous test but for the situation in which the price is
        //missing, and we try to rebalance by buying but the pending order is for a significantly
        //greater amount of shares than we now need (e.g. we have a price of X, we miss a price,
        //and then it drops 20%).
        let mut prices: HashMap<DateTime, Vec<Quote>> = HashMap::new();
        let quote = Quote::new(100.00, 100.00, 100, "ABC");
        let quote2 = Quote::new(75.00, 75.00, 103, "ABC");
        let quote3 = Quote::new(75.00, 75.00, 104, "ABC");

        prices.insert(100.into(), vec![quote]);
        prices.insert(101.into(), vec![]);
        prices.insert(102.into(), vec![]);
        prices.insert(103.into(), vec![quote2]);
        prices.insert(104.into(), vec![quote3]);

        let clock = ClockBuilder::with_length_in_seconds(100, 5)
            .with_frequency(&Frequency::Second)
            .build();

        let source = HashMapInputBuilder::new()
            .with_quotes(prices)
            .with_clock(Rc::clone(&clock))
            .build();

        let exchange = DefaultExchangeBuilder::new()
            .with_clock(Rc::clone(&clock))
            .with_data_source(source.clone())
            .build();

        let mut brkr = SimulatedBrokerBuilder::new()
            .with_data(source)
            .with_exchange(exchange)
            .build();

        brkr.deposit_cash(&100_000.0);
        let mut target_weights = PortfolioAllocation::new();
        target_weights.insert("ABC", 0.9);
        let orders =
            BrokerCalculations::diff_brkr_against_target_weights(&target_weights, &mut brkr);
        println!("{:?}", orders);
        brkr.send_orders(&orders);
        brkr.finish();

        //No price for security so we haven't diffed correctly
        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();
        let orders1 =
            BrokerCalculations::diff_brkr_against_target_weights(&target_weights, &mut brkr);
        println!("{:?}", orders1);

        brkr.send_orders(&orders1);
        brkr.finish();

        clock.borrow_mut().tick();
        brkr.check();
        brkr.finish();

        println!("{:?}", brkr.get_holdings());
        //If the logic isn't correct then the order will be for less shares than is actually
        //required by the newest price
        assert_eq!(*(*brkr.get_position_qty("ABC").unwrap()), 1200.0);
    }
}
