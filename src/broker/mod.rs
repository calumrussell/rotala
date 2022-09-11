use log::info;

use crate::types::{
    CashValue, DateTime, PortfolioAllocation, PortfolioHoldings, PortfolioQty, PortfolioValues,
    PortfolioWeight, Price,
};

pub mod record;
pub mod rules;

//Contains data structures and traits that refer solely to the data held and operations required
//for broker implementations.
#[derive(Clone, Debug)]
pub struct Quote {
    //TODO: more indirection is needed for this type, possibly implemented as trait
    pub bid: Price,
    pub ask: Price,
    pub date: DateTime,
    pub symbol: String,
}

#[derive(Clone, Debug)]
pub struct Dividend {
    //Dividend value is expressed in terms of per share values
    pub value: Price,
    pub symbol: String,
    pub date: DateTime,
}

#[derive(Clone, Debug)]
pub struct DividendPayment {
    pub value: CashValue,
    pub symbol: String,
    pub date: DateTime,
}

#[derive(Clone, Debug)]
pub enum TradeType {
    Buy,
    Sell,
}

#[derive(Clone, Debug)]
pub struct Trade {
    //TODO: more indirection is needed for this type, possibly implemented as trait
    pub symbol: String,
    pub value: CashValue,
    pub quantity: PortfolioQty,
    pub date: DateTime,
    pub typ: TradeType,
}

#[derive(Clone, Debug)]
pub enum BrokerEvent {
    TradeSuccess(Trade),
    TradeFailure(Order),
    OrderCreated(Order),
    OrderFailure(Order),
    WithdrawSuccess(CashValue),
    WithdrawFailure(CashValue),
    DepositSuccess(CashValue),
    //No value is returned for these variants because transactions are internal to broker
    TransactionSuccess,
    TransactionFailure,
}

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

#[derive(Clone, Copy, Debug)]
pub enum OrderType {
    MarketSell,
    MarketBuy,
    LimitSell,
    LimitBuy,
    StopSell,
    StopBuy,
}

#[derive(Clone, Debug)]
pub struct Order {
    order_type: OrderType,
    symbol: String,
    shares: PortfolioQty,
    price: Option<Price>,
}

impl Order {
    //TODO: should this be a trait?
    pub fn get_symbol(&self) -> String {
        self.symbol.clone()
    }

    pub fn get_shares(&self) -> PortfolioQty {
        self.shares
    }

    pub fn get_price(&self) -> Option<Price> {
        self.price
    }

    pub fn get_order_type(&self) -> OrderType {
        self.order_type
    }

    pub fn new(
        order_type: OrderType,
        symbol: String,
        shares: PortfolioQty,
        price: Option<Price>,
    ) -> Self {
        Order {
            order_type,
            symbol,
            shares,
            price,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BrokerCost {
    PerShare(Price),
    PctOfValue(f64),
    Flat(CashValue),
}

impl BrokerCost {
    pub fn calc(&self, trade: &Trade) -> CashValue {
        match self {
            BrokerCost::PerShare(cost) => trade.quantity * *cost,
            BrokerCost::PctOfValue(pct) => CashValue::from(f64::from(trade.value) * pct),
            BrokerCost::Flat(val) => *val,
        }
    }

    //Returns a valid trade given trading costs given a current budget
    //and price of security
    pub fn trade_impact(
        &self,
        gross_budget: &CashValue,
        gross_price: &Price,
        is_buy: bool,
    ) -> (CashValue, Price) {
        let mut net_budget = *gross_budget;
        let mut net_price = *gross_price;
        match self {
            BrokerCost::PerShare(val) => {
                if is_buy {
                    net_price += *val;
                } else {
                    net_price -= *val;
                }
            }
            BrokerCost::PctOfValue(pct) => {
                net_budget *= CashValue::from(1.0 - *pct);
            }
            BrokerCost::Flat(val) => net_budget -= *val,
        }
        (net_budget, net_price)
    }

    pub fn trade_impact_total(
        trade_costs: &Vec<BrokerCost>,
        gross_budget: &CashValue,
        gross_price: &Price,
        is_buy: bool,
    ) -> (CashValue, Price) {
        let mut res = (*gross_budget, *gross_price);
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
pub trait TransferCash {
    fn withdraw_cash(&mut self, cash: CashValue) -> BrokerEvent;
    fn deposit_cash(&mut self, cash: CashValue) -> BrokerEvent;
    fn debit(&mut self, value: CashValue) -> BrokerEvent;
    fn credit(&mut self, value: CashValue) -> BrokerEvent;
    fn get_cash_balance(&self) -> CashValue;
}

//Mutates because we have to call get_position_value
pub trait PositionInfo {
    //Position qty can always return a value, if we don't have the position then qty is 0
    fn get_position_qty(&self, symbol: &str) -> Option<&PortfolioQty>;
    //This mutates because the broker needs to keep track of prices last seen
    fn get_position_value(&mut self, symbol: &str) -> Option<CashValue>;
    fn get_position_cost(&self, symbol: &str) -> Option<Price>;
    fn get_position_liquidation_value(&mut self, symbol: &str) -> Option<CashValue>;
    fn get_position_profit(&mut self, symbol: &str) -> Option<CashValue>;
    fn get_liquidation_value(&mut self) -> CashValue;
    fn get_total_value(&mut self) -> CashValue;
    fn get_positions(&self) -> Vec<String>;
    fn get_values(&mut self) -> PortfolioValues;
    fn get_holdings(&self) -> PortfolioHoldings;
}

pub trait GetsQuote {
    fn get_quote(&self, symbol: &str) -> Option<Quote>;
    fn get_quotes(&self) -> Option<&Vec<Quote>>;
}

pub trait CanUpdate {
    fn update_holdings(&mut self, symbol: &str, change: &PortfolioQty);
}

pub trait PendingOrder {
    fn insert_order(&mut self, order: &Order);
    fn delete_order(&mut self, id: &u8);
}

pub trait ExecutesOrder {
    fn execute_order(&mut self, order: &Order) -> BrokerEvent;
    fn execute_orders(&mut self, orders: Vec<Order>) -> Vec<BrokerEvent>;
}

pub trait TradeCost {
    fn get_trade_costs(&self, trade: &Trade) -> CashValue;
    fn calc_trade_impact(
        &self,
        budget: &CashValue,
        price: &Price,
        is_buy: bool,
    ) -> (CashValue, Price);
}

pub trait PayDividend {
    fn pay_dividends(&mut self);
}

pub trait EventLog {
    fn trades_between(&self, start: &DateTime, end: &DateTime) -> Vec<Trade>;
    fn dividends_between(&self, start: &DateTime, end: &DateTime) -> Vec<DividendPayment>;
}

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
    pub fn withdraw_cash_with_liquidation<
        T: ExecutesOrder + TradeCost + PositionInfo + GetsQuote,
    >(
        cash: &CashValue,
        brkr: &mut T,
    ) -> BrokerEvent {
        //TODO:should this execute any trades at all? Would it be better to return a sequence of orders
        //required to achieve the cash balance, and then leave it up to the calling function to decide
        //whether to execute?
        info!("STRATEGY: Withdrawing {:?} with liquidation", cash);
        let value = brkr.get_liquidation_value();
        if cash > &value {
            BrokerEvent::WithdrawFailure(*cash)
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
                //Sell 100% of position
                if position_value <= total_sold {
                    //Cannot be called without qty existing
                    let qty = *brkr.get_position_qty(&ticker).unwrap();
                    let order = Order::new(OrderType::MarketSell, ticker, qty, None);
                    info!("STRATEGY: Withdrawing {:?} with liquidation, queueing sale of {:?} shares of {:?}", cash, order.get_shares(), order.get_symbol());
                    sell_orders.push(order);
                    total_sold -= position_value;
                } else {
                    //Position can generate all the cash we need
                    //Create orders to sell 100% of position, don't continue to next
                    //stock
                    //
                    //Cannot be called without quote existing
                    let price = brkr.get_quote(&ticker).unwrap().bid;
                    let shares_req = (total_sold / price).ceil();
                    let order = Order::new(OrderType::MarketSell, ticker, shares_req, None);
                    info!("STRATEGY: Withdrawing {:?} with liquidation, queueing sale of {:?} shares of {:?}", cash, order.get_shares(), order.get_symbol());
                    sell_orders.push(order);
                    total_sold = CashValue::default();
                    break;
                }
            }
            if total_sold == 0.0 {
                //The portfolio can provide enough cash so we can execute the sell orders
                //We leave the portfolio in the wrong state for the client to deal with
                brkr.execute_orders(sell_orders);
                info!("STRATEGY: Succesfully withdrew {:?} with liquidation", cash);
                BrokerEvent::WithdrawSuccess(*cash)
            } else {
                //The portfolio doesn't have the cash, don't execute any orders and return to
                //client to deal with the result
                info!("STRATEGY: Failed to withdrew {:?} with liquidation", cash);
                BrokerEvent::WithdrawFailure(*cash)
            }
        }
    }

    //Calculates the diff between the current state of the portfolio within broker, and the
    //target_weights passed into the function.
    //Returns orders so calling function has control over when orders are executed
    //Requires mutable reference to brkr because it calls get_position_value
    pub fn diff_brkr_against_target_weights<T: PositionInfo + TradeCost + GetsQuote>(
        target_weights: &PortfolioAllocation<PortfolioWeight>,
        brkr: &mut T,
    ) -> Vec<Order> {
        //Need liquidation value so we definitely have enough money to make all transactions after
        //costs
        info!("STRATEGY: Calculating diff of current allocation vs. target");
        let total_value = brkr.get_liquidation_value();
        if total_value == 0.0 {
            panic!("Client is attempting to trade a portfolio with zero value");
        }
        let mut orders: Vec<Order> = Vec::new();

        let mut buy_orders: Vec<Order> = Vec::new();
        let mut sell_orders: Vec<Order> = Vec::new();

        let calc_required_shares_with_costs =
            |diff_val: &CashValue, quote: &Quote, brkr: &T| -> PortfolioQty {
                let abs_val = diff_val.abs();
                //Maximise the number of shares we can acquire/sell net of costs.
                let trade_price: Price = if *diff_val > 0.0 {
                    quote.ask
                } else {
                    quote.bid
                };
                let res = brkr.calc_trade_impact(&abs_val, &trade_price, true);
                (res.0 / res.1).floor()
            };

        for symbol in target_weights.keys() {
            let curr_val = brkr.get_position_value(&symbol).unwrap_or_default();
            //Iterating over target_weights so will always find value
            let target_val = total_value * *target_weights.get(&symbol).unwrap();
            let diff_val = target_val - curr_val;
            if diff_val == 0.0 {
                break;
            }

            //We do not throw an error here, we just proceed assuming that the client has passed in data that will
            //eventually prove correct if we are missing quotes for the current time.
            if let Some(quote) = brkr.get_quote(&symbol) {
                let net_target_shares = calc_required_shares_with_costs(&diff_val, &quote, brkr);
                if diff_val > 0.0 {
                    buy_orders.push(Order::new(
                        OrderType::MarketBuy,
                        symbol.clone(),
                        net_target_shares,
                        None,
                    ));
                } else {
                    sell_orders.push(Order::new(
                        OrderType::MarketSell,
                        symbol.clone(),
                        net_target_shares,
                        None,
                    ));
                }
            }
        }
        //Sell orders have to be executed before buy orders
        orders.extend(sell_orders);
        orders.extend(buy_orders);
        orders
    }
}

#[cfg(test)]
mod tests {

    use crate::clock::ClockBuilder;
    use crate::input::fake_data_generator;
    use crate::sim::broker::SimulatedBrokerBuilder;
    use crate::types::PortfolioAllocation;
    use std::rc::Rc;

    use super::{BrokerCalculations, BrokerCost, TransferCash};

    #[test]
    fn diff_continues_if_security_missing() {
        //In this scenario, the user has inserted incorrect information but this scenario can also occur if there is no quote
        //for a given security on a certain date. We are interested in the latter case, not the former but it is more
        //difficult to test for the latter, and the code should be the same.
        let clock = ClockBuilder::from_length_days(&(0.into()), 10).daily();
        let input = fake_data_generator(Rc::clone(&clock));

        let mut brkr = SimulatedBrokerBuilder::new().with_data(input).build();

        let mut weights = PortfolioAllocation::new();
        weights.insert("ABC", &0.5.into());
        //There is no quote for this security in the underlying data, code should make the assumption (that doesn't apply here)
        //that there is some quote for this security at a later date and continues to generate order for ABC without throwing
        //error
        weights.insert("XYZ", &0.5.into());

        brkr.deposit_cash(100_000.0.into());
        clock.borrow_mut().tick();
        let orders = BrokerCalculations::diff_brkr_against_target_weights(&weights, &mut brkr);
        assert!(orders.len() == 1);
    }

    #[test]
    #[should_panic]
    fn diff_panics_if_brkr_has_no_cash() {
        //If we get to a point where the client is diffing without cash, we can assume that no further operations are possible
        //and we should panic
        let clock = ClockBuilder::from_length_days(&(0.into()), 10).daily();
        let input = fake_data_generator(Rc::clone(&clock));

        let mut brkr = SimulatedBrokerBuilder::new().with_data(input).build();

        let mut weights = PortfolioAllocation::new();
        weights.insert("ABC", &1.0.into());

        clock.borrow_mut().tick();
        BrokerCalculations::diff_brkr_against_target_weights(&weights, &mut brkr);
    }

    #[test]
    fn can_estimate_trade_costs_of_proposed_trade() {
        let pershare = BrokerCost::PerShare(0.1.into());
        let flat = BrokerCost::Flat(10.0.into());
        let pct = BrokerCost::PctOfValue(0.01);

        let res = pershare.trade_impact(&1000.0.into(), &1.0.into(), true);
        assert!(res.1 == 1.1);

        let res = pershare.trade_impact(&1000.0.into(), &1.0.into(), false);
        assert!(res.1 == 0.9);

        let res = flat.trade_impact(&1000.0.into(), &1.0.into(), true);
        assert!(res.0 == 990.00);

        let res = pct.trade_impact(&100.0.into(), &1.0.into(), true);
        assert!(res.0 == 99.0);

        let costs = vec![pershare, flat];
        let initial = BrokerCost::trade_impact_total(&costs, &1000.0.into(), &1.0.into(), true);
        assert!(initial.0 == 990.00);
        assert!(initial.1 == 1.1);
    }
}
