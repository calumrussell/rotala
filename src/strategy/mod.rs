/*
 * A Strategy wraps around the broker and portfolio, the idea
 * is to move most of the functionality into a trading strategy
 * and organize calls to the rest of the system through that.
 *
 * One key point is that the Strategy should only be aware of an
 * overall portfolio, and not aware of how the portfolio executes
 * changes with the broker.
*/

use crate::broker::{
    BrokerEvent, DividendPayment, EventLog, ExecutesOrder, GetsQuote, Order, OrderType,
    PositionInfo, Quote, Trade, TradeCost, TransferCash,
};
use crate::clock::Clock;
use crate::input::DataSource;
use crate::perf::{PerfStruct, PortfolioPerformance, StrategySnapshot};
use crate::schedule::{DefaultTradingSchedule, TradingSchedule};
use crate::sim::broker::SimulatedBroker;
use crate::types::{
    CashValue, DateTime, PortfolioAllocation, PortfolioQty, PortfolioWeight, Price,
};
use std::rc::Rc;

pub trait Strategy: TransferTo + Clone {
    fn update(&mut self) -> CashValue;
    fn init(&mut self, initial_cash: &CashValue);
    fn get_perf(&self) -> PerfStruct;
}

pub trait TransferTo {
    fn deposit_cash(&mut self, cash: &CashValue);
}

pub trait Audit {
    fn trades_between(&self, start: &DateTime, end: &DateTime) -> Vec<Trade>;
    fn dividends_between(&self, start: &DateTime, end: &DateTime) -> Vec<DividendPayment>;
}

pub trait TransferFrom {
    fn withdraw_cash(&mut self, cash: &CashValue);
    fn withdraw_cash_with_liquidation(&mut self, cash: &CashValue);
}

//TODO:should this execute any trades at all
fn withdraw_cash_with_liquidation_algo<T: ExecutesOrder + TradeCost + PositionInfo + GetsQuote>(
    cash: &CashValue,
    brkr: &mut T,
) -> BrokerEvent {
    let value = brkr.get_liquidation_value();
    if cash > &value {
        BrokerEvent::WithdrawFailure(*cash)
    } else {
        //This holds how much we have left to generate from the portfolio
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
                sell_orders.push(order);
                total_sold = CashValue::default();
                break;
            }
        }
        if total_sold == 0.0 {
            //The portfolio can provide enough cash so we can execute the sell orders
            //We leave the portfolio in the wrong state for the client to deal with
            brkr.execute_orders(sell_orders);
            BrokerEvent::WithdrawSuccess(*cash)
        } else {
            //The portfolio doesn't have the cash, don't execute any orders and return to
            //client to deal with the result
            BrokerEvent::WithdrawFailure(*cash)
        }
    }
}

//Returns orders so calling function has control over when orders are executed
fn diff<T: PositionInfo + TradeCost + GetsQuote>(
    target_weights: &PortfolioAllocation<PortfolioWeight>,
    brkr: &T,
) -> Vec<Order> {
    //Need liquidation value so we definitely have enough money to make all transactions after
    //costs
    let total_value = brkr.get_liquidation_value();
    let mut orders: Vec<Order> = Vec::new();

    let mut buy_orders: Vec<Order> = Vec::new();
    let mut sell_orders: Vec<Order> = Vec::new();

    let calc_required_shares_with_costs = |diff_val: &CashValue, quote: &Quote| -> PortfolioQty {
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

        //This is implementation detail, for a simulation we prefer immediate panic
        let quote = brkr
            .get_quote(&symbol)
            .expect("Can't find quote for symbol");
        let net_target_shares = calc_required_shares_with_costs(&diff_val, &quote);
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
    //Sell orders have to be executed before buy orders
    orders.extend(sell_orders);
    orders.extend(buy_orders);
    orders
}

pub struct StaticWeightStrategyBuilder<T: DataSource> {
    //If missing either field, we cannot run this strategy
    brkr: Option<SimulatedBroker<T>>,
    weights: Option<PortfolioAllocation<PortfolioWeight>>,
    clock: Option<Clock>,
}

impl<T: DataSource> StaticWeightStrategyBuilder<T> {
    pub fn daily(&self) -> StaticWeightStrategy<T> {
        if self.brkr.is_none() || self.weights.is_none() || self.clock.is_none() {
            panic!("Strategy must have broker, weights, and clock");
        }

        StaticWeightStrategy {
            brkr: self.brkr.clone().unwrap(),
            target_weights: self.weights.clone().unwrap(),
            perf: PortfolioPerformance::daily(),
            net_cash_flow: 0.0.into(),
            clock: Rc::clone(self.clock.as_ref().unwrap()),
        }
    }

    pub fn yearly(&self) -> StaticWeightStrategy<T> {
        if self.brkr.is_none() || self.weights.is_none() || self.clock.is_none() {
            panic!("Strategy must have broker, weights, and clock");
        }

        StaticWeightStrategy {
            brkr: self.brkr.clone().unwrap(),
            target_weights: self.weights.clone().unwrap(),
            perf: PortfolioPerformance::yearly(),
            net_cash_flow: 0.0.into(),
            clock: Rc::clone(self.clock.as_ref().unwrap()),
        }
    }

    pub fn with_clock(&mut self, clock: Clock) -> &mut Self {
        self.clock = Some(clock);
        self
    }

    pub fn with_brkr(&mut self, brkr: SimulatedBroker<T>) -> &mut Self {
        self.brkr = Some(brkr);
        self
    }

    pub fn with_weights(&mut self, weights: PortfolioAllocation<PortfolioWeight>) -> &mut Self {
        self.weights = Some(weights);
        self
    }

    pub fn new() -> Self {
        Self {
            brkr: None,
            weights: None,
            clock: None,
        }
    }
}

impl<T: DataSource> Default for StaticWeightStrategyBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct StaticWeightStrategy<T: DataSource> {
    brkr: SimulatedBroker<T>,
    target_weights: PortfolioAllocation<PortfolioWeight>,
    perf: PortfolioPerformance,
    net_cash_flow: CashValue,
    clock: Clock,
}

impl<T: DataSource> StaticWeightStrategy<T> {
    pub fn get_snapshot(&self) -> StrategySnapshot {
        StrategySnapshot {
            date: self.clock.borrow().now(),
            value: self.brkr.get_total_value(),
            net_cash_flow: self.net_cash_flow,
        }
    }
}

impl<T: DataSource> Strategy for StaticWeightStrategy<T> {
    fn init(&mut self, initital_cash: &CashValue) {
        self.deposit_cash(initital_cash);
        let state = self.get_snapshot();
        self.perf.update(&state);
    }

    fn update(&mut self) -> CashValue {
        if DefaultTradingSchedule::should_trade(&self.clock.borrow().now()) {
            let orders = diff(&self.target_weights, &self.brkr);
            if !orders.is_empty() {
                self.brkr.execute_orders(orders);
            }
        }
        self.brkr.check();
        let state = self.get_snapshot();
        self.perf.update(&state);
        self.brkr.get_total_value()
    }

    fn get_perf(&self) -> PerfStruct {
        self.perf.get_output()
    }
}

impl<T: DataSource> TransferTo for StaticWeightStrategy<T> {
    fn deposit_cash(&mut self, cash: &CashValue) {
        self.brkr.deposit_cash(*cash);
        self.net_cash_flow += *cash;
    }
}

impl<T: DataSource> TransferFrom for StaticWeightStrategy<T> {
    fn withdraw_cash(&mut self, cash: &CashValue) {
        if let BrokerEvent::WithdrawSuccess(withdrawn) = self.brkr.withdraw_cash(*cash) {
            self.net_cash_flow -= withdrawn;
        }
    }
    fn withdraw_cash_with_liquidation(&mut self, cash: &CashValue) {
        if let BrokerEvent::WithdrawSuccess(withdrawn) =
            withdraw_cash_with_liquidation_algo(cash, &mut self.brkr)
        {
            self.net_cash_flow -= withdrawn;
        }
    }
}

impl<T: DataSource> Audit for StaticWeightStrategy<T> {
    fn trades_between(&self, start: &DateTime, end: &DateTime) -> Vec<Trade> {
        self.brkr.trades_between(start, end)
    }

    fn dividends_between(&self, start: &DateTime, end: &DateTime) -> Vec<DividendPayment> {
        self.brkr.dividends_between(start, end)
    }
}

#[cfg(test)]
mod tests {

    use std::collections::HashMap;
    use std::rc::Rc;

    use super::StaticWeightStrategyBuilder;
    use crate::broker::{BrokerCost, Quote};
    use crate::clock::{Clock, ClockBuilder};
    use crate::input::{HashMapInput, HashMapInputBuilder};
    use crate::sim::broker::{SimulatedBroker, SimulatedBrokerBuilder};
    use crate::types::{DateTime, PortfolioAllocation};

    fn setup() -> (SimulatedBroker<HashMapInput>, Clock) {
        let mut prices: HashMap<DateTime, Vec<Quote>> = HashMap::new();

        let quote = Quote {
            bid: 100.00.into(),
            ask: 101.00.into(),
            date: 100.into(),
            symbol: String::from("ABC"),
        };
        let quote2 = Quote {
            bid: 104.00.into(),
            ask: 105.00.into(),
            date: 101.into(),
            symbol: String::from("ABC"),
        };
        let quote4 = Quote {
            bid: 95.00.into(),
            ask: 96.00.into(),
            date: 102.into(),
            symbol: String::from("ABC"),
        };
        prices.insert(100.into(), vec![quote]);
        prices.insert(101.into(), vec![quote2]);
        prices.insert(102.into(), vec![quote4]);

        let clock = ClockBuilder::from_fixed(100.into(), 102.into()).every();

        let source = HashMapInputBuilder::new()
            .with_quotes(prices)
            .with_clock(Rc::clone(&clock))
            .build();

        let brkr = SimulatedBrokerBuilder::<HashMapInput>::new()
            .with_data(source)
            .with_trade_costs(vec![BrokerCost::Flat(0.1.into())])
            .build();
        (brkr, clock)
    }

    #[test]
    #[should_panic]
    fn test_that_static_builder_fails_without_weights() {
        let comp = setup();
        let _strat = StaticWeightStrategyBuilder::<HashMapInput>::new()
            .with_brkr(comp.0)
            .daily();
    }

    #[test]
    #[should_panic]
    fn test_that_static_builder_fails_without_brkr() {
        let weights = PortfolioAllocation::new();
        let _strat = StaticWeightStrategyBuilder::<HashMapInput>::new()
            .with_weights(weights)
            .daily();
    }
}
