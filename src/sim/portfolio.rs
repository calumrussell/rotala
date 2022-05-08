use std::collections::HashMap;

use math::round;

use super::broker::SimulatedBroker;
use crate::broker::{BrokerEvent, CashManager, ClientControlled, PositionInfo, PriceQuote};
use crate::broker::{Order, OrderExecutor, OrderType};
use crate::portfolio::{Holdings, Portfolio, PortfolioState, PortfolioStats};

#[derive(Clone)]
pub struct SimPortfolio {
    brkr: SimulatedBroker,
    net_cash_flow: f64,
}

impl PortfolioStats for SimPortfolio {
    fn get_total_value(&self) -> f64 {
        let assets = self.brkr.get_positions();
        let mut value = self.brkr.get_cash_balance() as f64;
        for a in assets {
            let symbol_value = self.brkr.get_position_value(&a);
            if symbol_value.is_some() {
                value += symbol_value.unwrap()
            }
        }
        value
    }

    fn get_holdings(&self) -> Holdings {
        let mut holdings = Holdings::new();

        let assets = self.brkr.get_positions();
        for a in assets {
            let value = self.brkr.get_position_value(&a);
            if value.is_some() {
                holdings.put(&a, &value.unwrap())
            }
        }
        holdings
    }

    fn get_position_value(&self, symbol: &String) -> Option<f64> {
        self.brkr.get_position_value(symbol)
    }

    fn get_current_state(&self) -> PortfolioState {
        let holdings = self.get_holdings();
        PortfolioState {
            value: self.get_total_value(),
            positions: holdings,
            net_cash_flow: self.net_cash_flow,
        }
    }
}

impl SimPortfolio {
    pub fn new(brkr: SimulatedBroker) -> SimPortfolio {
        SimPortfolio {
            brkr,
            net_cash_flow: 0_f64,
        }
    }

    pub fn set_date(&mut self, new_date: &i64) -> PortfolioState {
        self.brkr.set_date(new_date);
        self.get_current_state()
    }

    pub fn execute_order(&mut self, order: &Order) -> BrokerEvent {
        self.brkr.execute_order(order)
    }

    pub fn execute_orders(&mut self, orders: Vec<Order>) -> Vec<BrokerEvent> {
        self.brkr.execute_orders(orders)
    }

    fn get_position_diff(
        &self,
        symbol: &String,
        target_weights: &HashMap<String, f64>,
        total_value: f64,
    ) -> f64 {
        let target_value = target_weights.get(symbol).unwrap() * total_value;
        let curr_value = self.get_position_value(symbol).unwrap_or(0.0);
        target_value - curr_value
    }
}

impl Portfolio for SimPortfolio {
    fn deposit_cash(&mut self, cash: &u64) {
        self.brkr.deposit_cash(*cash);
        self.net_cash_flow += *cash as f64;
    }

    fn withdraw_cash(&mut self, cash: &u64) {
        self.brkr.withdraw_cash(*cash);
        self.net_cash_flow -= *cash as f64;
    }

    //This function is named erroneously, we aren't mutating the state of the portfolio
    //but calculating a diff and set of orders needed to close a diff
    //Returns orders so calling client has control when orders are executed
    fn update_weights(&self, target_weights: &HashMap<String, f64>) -> Vec<Order> {
        let total_value = self.get_total_value();
        let mut orders: Vec<Order> = Vec::new();

        let mut buy_orders: Vec<Order> = Vec::new();
        let mut sell_orders: Vec<Order> = Vec::new();

        for symbol in target_weights.keys() {
            let diff_val = self.get_position_diff(symbol, target_weights, total_value);
            let quote = self.brkr.get_quote(symbol);
            match quote {
                Some(q) => {
                    if diff_val > 0.0 {
                        let target_shares = round::floor(diff_val / q.ask, 0);
                        let order =
                            Order::new(OrderType::MarketBuy, symbol.clone(), target_shares, None);
                        buy_orders.push(order);
                    } else {
                        let target_shares = round::floor(diff_val / q.bid, 0);
                        if target_shares == 0.00 {
                            //Covers case when diff_val is zero
                            break;
                        }

                        let order = Order::new(
                            OrderType::MarketSell,
                            symbol.clone(),
                            target_shares * -1.0,
                            None,
                        );
                        sell_orders.push(order);
                    }
                }
                //This is implementation detail, for a simulation we prefer immediate panic
                None => panic!("Can't find price for symbol"),
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

    use super::SimPortfolio;
    use crate::broker::{BrokerEvent, Quote};
    use crate::data::DataSource;
    use crate::portfolio::Portfolio;
    use crate::sim::broker::SimulatedBroker;

    use std::collections::HashMap;

    fn setup() -> SimulatedBroker {
        let mut prices: HashMap<i64, Vec<Quote>> = HashMap::new();

        let mut price_row: Vec<Quote> = Vec::new();
        let mut price_row1: Vec<Quote> = Vec::new();
        let mut price_row2: Vec<Quote> = Vec::new();
        let quote = Quote {
            bid: 100.00,
            ask: 101.00,
            date: 100,
            symbol: String::from("ABC"),
        };
        let quote1 = Quote {
            bid: 10.00,
            ask: 11.00,
            date: 100,
            symbol: String::from("BCD"),
        };
        let quote2 = Quote {
            bid: 104.00,
            ask: 105.00,
            date: 101,
            symbol: String::from("ABC"),
        };
        let quote3 = Quote {
            bid: 14.00,
            ask: 15.00,
            date: 101,
            symbol: String::from("BCD"),
        };
        let quote4 = Quote {
            bid: 95.00,
            ask: 96.00,
            date: 101,
            symbol: String::from("ABC"),
        };
        let quote5 = Quote {
            bid: 10.00,
            ask: 11.00,
            date: 101,
            symbol: String::from("BCD"),
        };

        price_row.push(quote);
        price_row.push(quote1);
        price_row1.push(quote2);
        price_row1.push(quote3);
        price_row2.push(quote4);
        price_row2.push(quote5);

        prices.insert(100, price_row);
        prices.insert(101, price_row1);
        prices.insert(102, price_row2);

        let source = DataSource::from_hashmap(prices);
        let brkr = SimulatedBroker::new(source);
        brkr
    }

    #[test]
    #[should_panic]
    fn test_that_portfolio_with_bad_target_weights_throws_panic() {
        let simbrkr = setup();
        let mut port = SimPortfolio::new(simbrkr);
        port.deposit_cash(&100_000_u64);
        port.set_date(&101);

        //Update weights with non-existent target weight
        let mut target: HashMap<String, f64> = HashMap::new();
        target.insert(String::from("XYZ"), 0.9);

        port.update_weights(&target);
    }

    #[test]
    fn test_that_diff_is_calculated_correctly() {
        let simbrkr = setup();
        let mut port = SimPortfolio::new(simbrkr);
        port.deposit_cash(&100_000_u64);
        port.set_date(&101);

        let mut target: HashMap<String, f64> = HashMap::new();
        target.insert(String::from("ABC"), 1.0);

        let orders = port.update_weights(&target);
        assert!(orders.get(0).unwrap().get_shares() == 952.0);
    }

    #[test]
    fn test_that_there_update_weights_is_idempotent() {
        //We need to add this because we had odd cycling behaviour when
        //we introduced another dependency into how updates were
        //calculated

        let simbrkr = setup();

        let mut port = SimPortfolio::new(simbrkr);
        port.deposit_cash(&100_000_u64);
        port.set_date(&101);

        let mut target: HashMap<String, f64> = HashMap::new();
        target.insert(String::from("ABC"), 1.0);

        let orders = port.update_weights(&target);
        let orders1 = port.update_weights(&target);

        assert!(orders.len() == orders1.len());
        assert!(orders.get(0).unwrap().get_shares() == orders1.get(0).unwrap().get_shares());

        port.execute_orders(orders);
        port.set_date(&102);

        let mut target1: HashMap<String, f64> = HashMap::new();
        target1.insert(String::from("ABC"), 0.5);
        target1.insert(String::from("BCD"), 0.5);

        let orders2 = port.update_weights(&target1);
        //Not perfect, but we had a bug where this would fail after orders were executed due
        //to bug in outside dependency
        assert!(orders2.len() > 0);
    }

    #[test]
    fn test_that_orders_created_with_valid_input() {
        let simbrkr = setup();
        let mut port = SimPortfolio::new(simbrkr);
        port.deposit_cash(&100_000_u64);
        port.set_date(&101);

        let mut target: HashMap<String, f64> = HashMap::new();
        target.insert(String::from("ABC"), 1.0);

        let orders = port.update_weights(&target);
        assert!(orders.len() > 0);
    }

    #[test]
    fn test_that_portfolio_creates_no_orders_with_cashless_portfolio() {
        //Odd case but could occur if client fails to deposit cash or
        //if the portfolio enters a state with no free cash

        //Initial bug was that the portfolio would enter this state but
        //issue orders for zero shares
        let simbrkr = setup();
        let mut port = SimPortfolio::new(simbrkr);
        port.deposit_cash(&0_u64);
        port.set_date(&101);

        let mut target: HashMap<String, f64> = HashMap::new();
        target.insert(String::from("ABC"), 1.0);

        let orders = port.update_weights(&target);
        assert!(orders.len() == 0);
    }

    #[test]
    fn test_that_orders_will_be_ordered_correctly() {
        //Sell orders should be executed before buy orders so that we have cash
        //from sell orders to create new buy orders. Need to that orders complete
        //Order should always complete if we have sell order for N then a buy order
        //for N + Y, as long as liquidation value is > N+Y.

        //The sequence of trades is impossible to execute without ordering sells
        //before buys
        let simbrkr = setup();
        let mut port = SimPortfolio::new(simbrkr);
        port.deposit_cash(&100_000_u64);
        port.set_date(&101);

        let mut target: HashMap<String, f64> = HashMap::new();
        target.insert(String::from("ABC"), 1.0);

        let orders = port.update_weights(&target);
        port.execute_orders(orders);

        let mut target1: HashMap<String, f64> = HashMap::new();
        target1.insert(String::from("ABC"), 0.1);
        target1.insert(String::from("BCD"), 0.9);

        let orders1 = port.update_weights(&target1);
        let res = port.execute_orders(orders1);
        assert!(res.len() == 2);
        assert!(matches!(res.get(0).unwrap(), BrokerEvent::TradeSuccess(..)));
        assert!(matches!(res.get(1).unwrap(), BrokerEvent::TradeSuccess(..)));
    }
}
