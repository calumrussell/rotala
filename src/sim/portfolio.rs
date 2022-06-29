use super::broker::SimulatedBroker;
use crate::broker::{
    BrokerEvent, CashManager, ClientControlled, DividendPayment, HasLog, Order, OrderExecutor,
    OrderType, PositionInfo, PriceQuote, Quote, Trade, TradeCosts,
};
use crate::data::{CashValue, DateTime, PortfolioAllocation, PortfolioQty, PortfolioWeight, Price};
use crate::portfolio::{Portfolio, PortfolioState, PortfolioStats, PortfolioValues};

#[derive(Clone, Debug)]
pub struct SimPortfolio {
    brkr: SimulatedBroker,
    //Needed for calculation of portfolio performance inc. deposit/withdrawal
    net_cash_flow: CashValue,
}

impl PortfolioStats for SimPortfolio {
    fn get_total_value(&self) -> CashValue {
        //TODO: this should only use methods on the portfolio
        let assets = self.brkr.get_positions();
        let mut value = self.brkr.get_cash_balance();
        for a in assets {
            if let Some(position_value) = self.brkr.get_position_value(&a) {
                value += position_value
            }
        }
        value
    }

    fn get_liquidation_value(&self) -> CashValue {
        //TODO: this should only use methods on the portfolio
        let mut value = self.brkr.get_cash_balance();
        for asset in self.brkr.get_positions() {
            if let Some(asset_value) = self.brkr.get_position_liquidation_value(&asset) {
                value += asset_value
            }
        }
        value
    }

    fn get_cash_value(&self) -> CashValue {
        self.brkr.get_cash_balance()
    }

    fn get_holdings(&self) -> PortfolioValues {
        let mut holdings = PortfolioValues::new();

        let assets = self.brkr.get_positions();
        for a in assets {
            let value = self.brkr.get_position_value(&a);
            if let Some(v) = value {
                holdings.insert(&a, &v);
            }
        }
        holdings
    }

    fn get_position_qty(&self, symbol: &str) -> Option<&PortfolioQty> {
        self.brkr.get_position_qty(symbol)
    }

    fn get_position_value(&self, symbol: &str) -> Option<CashValue> {
        self.brkr.get_position_value(symbol)
    }

    fn get_position_liquidation_value(&self, symbol: &str) -> Option<CashValue> {
        self.brkr.get_position_liquidation_value(symbol)
    }

    fn get_current_state(&self, new_date: &DateTime) -> PortfolioState {
        let holdings = self.get_holdings();
        PortfolioState {
            date: new_date.clone(),
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
            net_cash_flow: CashValue::from(0.0),
        }
    }

    pub fn trades_between(&self, start: &DateTime, end: &DateTime) -> Vec<Trade> {
        self.brkr.trades_between(start, end)
    }

    pub fn dividends_between(&self, start: &DateTime, end: &DateTime) -> Vec<DividendPayment> {
        self.brkr.dividends_between(start, end)
    }

    pub fn set_date(&mut self, new_date: &DateTime) -> PortfolioState {
        self.brkr.set_date(new_date);
        self.get_current_state(new_date)
    }

    pub fn execute_order(&mut self, order: &Order) -> BrokerEvent {
        self.brkr.execute_order(order)
    }

    pub fn execute_orders(&mut self, orders: Vec<Order>) -> Vec<BrokerEvent> {
        self.brkr.execute_orders(orders)
    }
}

impl Portfolio for SimPortfolio {
    fn deposit_cash(&mut self, cash: &CashValue) -> bool {
        self.brkr.deposit_cash(*cash);
        self.net_cash_flow += *cash;
        true
    }

    fn withdraw_cash(&mut self, cash: &CashValue) -> bool {
        let event = self.brkr.withdraw_cash(*cash);
        match event {
            BrokerEvent::WithdrawSuccess(_val) => {
                self.net_cash_flow -= *cash;
                true
            }
            BrokerEvent::WithdrawFailure(_val) => false,
            _ => false,
        }
    }

    fn withdraw_cash_with_liquidation(&mut self, cash: &CashValue) -> bool {
        let value = self.get_liquidation_value();
        if cash > &value {
            false
        } else {
            //This holds how much we have left to generate from the portfolio
            let mut total_sold = *cash;

            let positions = self.brkr.get_positions();
            let mut sell_orders: Vec<Order> = Vec::new();
            for ticker in positions {
                let position_value = self.brkr.get_position_value(&ticker).unwrap_or_default();
                //Position won't generate enough cash to fulfill total order
                //Create orders for selling 100% of position, continue
                //to next position to see if we can generate enough cash
                //Sell 100% of position
                if position_value <= total_sold {
                    //Cannot be called without qty existing
                    let qty = *self.brkr.get_position_qty(&ticker).unwrap();
                    let order = Order::new(OrderType::MarketSell, ticker, qty, None);
                    sell_orders.push(order);
                    total_sold -= position_value;
                } else {
                    //Position can generate all the cash we need
                    //Create orders to sell 100% of position, don't continue to next
                    //stock
                    //
                    //Cannot be called without quote existing
                    let price = self.brkr.get_quote(&ticker).unwrap().bid;
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
                self.execute_orders(sell_orders);
                self.net_cash_flow -= *cash;
                true
            } else {
                //The portfolio doesn't have the cash, don't execute any orders and return to
                //client to deal with the result
                false
            }
        }
    }

    //This function is named erroneously, we aren't mutating the state of the portfolio
    //but calculating a diff and set of orders needed to close a diff
    //Returns orders so calling client has control when orders are executed
    fn update_weights(&self, target_weights: &PortfolioAllocation<PortfolioWeight>) -> Vec<Order> {
        //Need liquidation value so we definitely have enough money to make all transactions after
        //costs
        let total_value = self.get_liquidation_value();
        let mut orders: Vec<Order> = Vec::new();

        let mut buy_orders: Vec<Order> = Vec::new();
        let mut sell_orders: Vec<Order> = Vec::new();

        let calc_required_shares_with_costs = |diff_val: &CashValue,
                                               quote: &Quote|
         -> PortfolioQty {
            let abs_val = diff_val.abs();
            let trade_price: Price;
            let (net_budget, net_price): (CashValue, Price);
            //Maximise the number of shares we can acquire/sell net of costs.
            if *diff_val > 0.0 {
                trade_price = quote.ask;
                (net_budget, net_price) = self.brkr.calc_trade_impact(&abs_val, &trade_price, true);
            } else {
                trade_price = quote.bid;
                (net_budget, net_price) =
                    self.brkr.calc_trade_impact(&abs_val, &trade_price, false);
            }
            (net_budget / net_price).floor()
        };

        for symbol in target_weights.keys() {
            let curr_val = self.get_position_value(&symbol).unwrap_or_default();
            //Iterating over target_weights so will always find value
            let target_val = total_value * *target_weights.get(&symbol).unwrap();
            let diff_val = target_val - curr_val;
            if diff_val == 0.0 {
                break;
            }

            //This is implementation detail, for a simulation we prefer immediate panic
            let quote = self
                .brkr
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
}

#[cfg(test)]
mod tests {

    use super::SimPortfolio;
    use crate::broker::{BrokerCost, BrokerEvent, Dividend, Quote};
    use crate::data::{CashValue, DataSource, DateTime, PortfolioAllocation};
    use crate::portfolio::{Portfolio, PortfolioStats};
    use crate::sim::broker::SimulatedBroker;

    use std::collections::HashMap;

    fn setup() -> SimulatedBroker {
        let mut prices: HashMap<DateTime, Vec<Quote>> = HashMap::new();
        let dividends: HashMap<DateTime, Vec<Dividend>> = HashMap::new();

        let mut price_row: Vec<Quote> = Vec::new();
        let mut price_row1: Vec<Quote> = Vec::new();
        let mut price_row2: Vec<Quote> = Vec::new();
        let quote = Quote {
            bid: 100.00.into(),
            ask: 101.00.into(),
            date: 100.into(),
            symbol: String::from("ABC"),
        };
        let quote1 = Quote {
            bid: 10.00.into(),
            ask: 11.00.into(),
            date: 100.into(),
            symbol: String::from("BCD"),
        };
        let quote2 = Quote {
            bid: 104.00.into(),
            ask: 105.00.into(),
            date: 101.into(),
            symbol: String::from("ABC"),
        };
        let quote3 = Quote {
            bid: 14.00.into(),
            ask: 15.00.into(),
            date: 101.into(),
            symbol: String::from("BCD"),
        };
        let quote4 = Quote {
            bid: 95.00.into(),
            ask: 96.00.into(),
            date: 101.into(),
            symbol: String::from("ABC"),
        };
        let quote5 = Quote {
            bid: 10.00.into(),
            ask: 11.00.into(),
            date: 101.into(),
            symbol: String::from("BCD"),
        };

        price_row.push(quote);
        price_row.push(quote1);
        price_row1.push(quote2);
        price_row1.push(quote3);
        price_row2.push(quote4);
        price_row2.push(quote5);

        prices.insert(100.into(), price_row);
        prices.insert(101.into(), price_row1);
        prices.insert(102.into(), price_row2);

        let source = DataSource::from_hashmap(prices, dividends);
        let brkr = SimulatedBroker::new(source, vec![BrokerCost::PctOfValue(0.001)]);
        brkr
    }

    #[test]
    #[should_panic]
    fn test_that_portfolio_with_bad_target_weights_throws_panic() {
        let simbrkr = setup();
        let mut port = SimPortfolio::new(simbrkr);
        port.deposit_cash(&100_000.0.into());
        port.set_date(&101.into());

        //Update weights with non-existent target weight
        let mut target = PortfolioAllocation::new();
        target.insert(&String::from("XYZ"), &0.9.into());

        port.update_weights(&target);
    }

    #[test]
    fn test_that_diff_creates_new_order() {
        let simbrkr = setup();
        let mut port = SimPortfolio::new(simbrkr);
        port.deposit_cash(&100_000.0.into());
        port.set_date(&101.into());

        let mut target = PortfolioAllocation::new();
        target.insert(&String::from("ABC"), &1.0.into());

        let orders = port.update_weights(&target);
        assert!(orders.len() > 0);
    }

    #[test]
    fn test_that_there_update_weights_is_idempotent() {
        //We need to add this because we had odd cycling behaviour when
        //we introduced another dependency into how updates were
        //calculated

        let simbrkr = setup();

        let mut port = SimPortfolio::new(simbrkr);
        port.deposit_cash(&100_000.0.into());
        port.set_date(&101.into());

        let mut target = PortfolioAllocation::new();
        target.insert(&String::from("ABC"), &1.0.into());

        let orders = port.update_weights(&target);
        let orders1 = port.update_weights(&target);

        assert!(orders.len() == orders1.len());
        assert!(orders.get(0).unwrap().get_shares() == orders1.get(0).unwrap().get_shares());

        port.execute_orders(orders);
        port.set_date(&102.into());

        let mut target1 = PortfolioAllocation::new();
        target1.insert(&String::from("ABC"), &0.5.into());
        target1.insert(&String::from("BCD"), &0.5.into());

        let orders2 = port.update_weights(&target1);
        //Not perfect, but we had a bug where this would fail after orders were executed due
        //to bug in outside dependency
        assert!(orders2.len() > 0);
    }

    #[test]
    fn test_that_orders_created_with_valid_input() {
        let simbrkr = setup();
        let mut port = SimPortfolio::new(simbrkr);
        port.deposit_cash(&100_000.0.into());
        port.set_date(&101.into());

        let mut target = PortfolioAllocation::new();
        target.insert(&String::from("ABC"), &1.0.into());

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
        port.deposit_cash(&0.0.into());
        port.set_date(&101.into());

        let mut target = PortfolioAllocation::new();
        target.insert(&String::from("ABC"), &1.0.into());

        let orders = port.update_weights(&target);
        assert!(orders.len() == 0);
    }

    #[test]
    fn test_that_orders_will_be_ordered_correctly() {
        //Sell orders should be executed before buy orders so that we have cash
        //from sell orders to create new buy orders.
        //Order should always complete if we have sell order for N then a buy order
        //for N + Y, as long as liquidation value is > N+Y.

        //Sequence of trades is impossible to execute without ordering sells
        //before buys
        let simbrkr = setup();
        let mut port = SimPortfolio::new(simbrkr);
        port.deposit_cash(&100_000.0.into());
        port.set_date(&101.into());

        let mut target = PortfolioAllocation::new();
        target.insert(&String::from("ABC"), &1.0.into());

        let orders = port.update_weights(&target);
        port.execute_orders(orders);

        let mut target1 = PortfolioAllocation::new();
        target1.insert(&String::from("ABC"), &0.1.into());
        target1.insert(&String::from("BCD"), &0.9.into());
        let orders1 = port.update_weights(&target1);

        let res = port.execute_orders(orders1);
        //Failing here because the sell is being calculated off the totalvalue
        //So the buy order thinks the portfolio is worth X but it is actually worth
        //X - costs, when the sell goes through there isn't enough cash to fulfill
        //the buy order that was calculated using the values
        //Need to work out total portfolio value net of costs
        assert!(res.len() == 2);
        assert!(matches!(res.get(0).unwrap(), BrokerEvent::TradeSuccess(..)));
        assert!(matches!(res.get(1).unwrap(), BrokerEvent::TradeSuccess(..)));
    }

    #[test]
    fn test_that_withdraw_returns_correct_result_with_transaction_ordering() {
        let simbrkr = setup();
        let mut port = SimPortfolio::new(simbrkr);

        port.deposit_cash(&100.0.into());
        assert!(port.withdraw_cash(&CashValue::from(50.0)) == true);
        assert!(port.withdraw_cash(&CashValue::from(200.0)) == false);
    }

    #[test]
    fn test_that_withdraw_liquidation_will_sell_positions_to_generate_cash() {
        let simbrkr = setup();
        let mut port = SimPortfolio::new(simbrkr);

        port.deposit_cash(&100_000.0.into());
        port.set_date(&101.into());

        let mut target = PortfolioAllocation::new();
        target.insert(&String::from("ABC"), &1.0.into());

        let orders = port.update_weights(&target);
        port.execute_orders(orders);

        port.set_date(&102.into());
        port.withdraw_cash_with_liquidation(&CashValue::from(50_000.0));
    }

    #[test]
    fn test_that_withdraw_liquidation_can_liquidate_total_portfolio() {
        //Full liquidations can fail once costs are added
        //We need to attempt to liquidate the full value without costs
        //and check that will fail
        //Then check that we can liquidate the liquidation value which
        //includes costs
        let simbrkr = setup();
        let mut port = SimPortfolio::new(simbrkr);
        port.deposit_cash(&100_000.0.into());
        port.set_date(&101.into());

        let mut target = PortfolioAllocation::new();
        target.insert(&String::from("ABC"), &1.0.into());

        let orders = port.update_weights(&target);
        port.execute_orders(orders);

        port.set_date(&102.into());
        let total_value = port.get_total_value();
        let liquidation_value = port.get_liquidation_value();
        assert!(port.withdraw_cash_with_liquidation(&total_value) == false);
        assert!(port.withdraw_cash_with_liquidation(&liquidation_value));
    }
}
