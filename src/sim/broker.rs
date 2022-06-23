use core::panic;

use super::orderbook::SimOrderBook;
use crate::broker::record::BrokerLog;
use crate::broker::rules::OrderExecutionRules;
use crate::broker::{
    BrokerCost, BrokerEvent, CashManager, ClientControlled, Dividend, HasLog, HasTime,
    PaysDividends, PendingOrders, PositionInfo, PriceQuote, Quote, Trade, TradeCosts,
};
use crate::broker::{Order, OrderExecutor, OrderType};
use crate::data::{
    CashValue, DataSource, DateTime, PortfolioHoldings, PortfolioQty, Price, SimSource,
};

#[derive(Clone)]
pub struct SimulatedBroker {
    raw_data: DataSource,
    date: DateTime,
    holdings: PortfolioHoldings,
    orderbook: SimOrderBook,
    cash: CashValue,
    log: BrokerLog,
    trade_costs: Vec<BrokerCost>,
}

impl CashManager for SimulatedBroker {
    fn withdraw_cash(&mut self, cash: CashValue) -> BrokerEvent {
        if cash > self.cash {
            return BrokerEvent::WithdrawFailure(cash);
        }
        self.cash -= cash;
        BrokerEvent::WithdrawSuccess(cash)
    }

    fn deposit_cash(&mut self, cash: CashValue) -> BrokerEvent {
        self.cash += cash;
        BrokerEvent::DepositSuccess(cash)
    }

    //Identical to deposit_cash but is seperated to distinguish internal cash
    //transactions from external with no value returned to client
    fn credit(&mut self, value: CashValue) -> BrokerEvent {
        self.cash += value;
        BrokerEvent::TransactionSuccess
    }

    //Looks similar to withdraw_cash but distinguished because it represents
    //failure of an internal transaction with no value returned to clients
    fn debit(&mut self, value: CashValue) -> BrokerEvent {
        if value > self.cash {
            return BrokerEvent::TransactionFailure;
        }
        self.cash -= value;
        BrokerEvent::TransactionSuccess
    }

    fn get_cash_balance(&self) -> CashValue {
        self.cash
    }
}

impl PositionInfo for SimulatedBroker {
    fn get_position_cost(&self, symbol: &str) -> Option<Price> {
        self.log.cost_basis(symbol)
    }

    fn get_position_profit(&self, symbol: &str) -> Option<CashValue> {
        if let Some(cost) = self.log.cost_basis(symbol) {
            if let Some(price) = self.get_quote(symbol) {
                //Once we get to this point we can unwrap safely
                let qty = *self.get_position_qty(symbol).unwrap();
                let profit: Price = if qty > 0.0 {
                    price.bid - cost
                } else {
                    price.ask - cost
                };
                //Profit in CashValue
                return Some(profit * qty);
            }
        }
        None
    }

    fn get_position_qty(&self, symbol: &str) -> Option<&PortfolioQty> {
        self.holdings.get(symbol)
    }

    fn get_position_liquidation_value(&self, symbol: &str) -> Option<CashValue> {
        //TODO: we need to introduce some kind of distinction between short and long
        //      positions.
        if let Some(quote) = self.get_quote(symbol) {
            let price = quote.bid;
            if let Some(qty) = self.get_position_qty(symbol) {
                //TODO: When we fix qty off primitive, this should be a special operation that
                //produces CashValue
                let position_value = price * *qty;
                let (value_after_costs, _price_after_costs) =
                    self.calc_trade_impact(&position_value, &price, false);
                return Some(value_after_costs);
            }
        }
        None
    }

    fn get_position_value(&self, symbol: &str) -> Option<CashValue> {
        //TODO: we need to introduce some kind of distinction between short and long
        //      positions.
        if let Some(quote) = self.get_quote(symbol) {
            let price = quote.bid;
            if let Some(qty) = self.get_position_qty(symbol) {
                return Some(price * *qty);
            }
        }
        None
    }
}

impl PriceQuote for SimulatedBroker {
    fn get_quote(&self, symbol: &str) -> Option<Quote> {
        self.raw_data.get_quote_by_date_symbol(&self.date, symbol)
    }

    fn get_quotes(&self) -> Option<Vec<Quote>> {
        self.raw_data.get_quotes_by_date(&self.date)
    }
}

impl OrderExecutor for SimulatedBroker {
    fn execute_order(&mut self, order: &Order) -> BrokerEvent {
        if let OrderType::LimitBuy
        | OrderType::LimitSell
        | OrderType::StopBuy
        | OrderType::StopSell = order.get_order_type()
        {
            panic!("Can only call execute order with market orders")
        };

        let quote = self.get_quote(&order.get_symbol());
        if quote.is_none() {
            return BrokerEvent::TradeFailure(order.clone());
        }

        let price = match order.get_order_type() {
            OrderType::MarketBuy => quote.unwrap().ask,
            OrderType::MarketSell => quote.unwrap().bid,
            _ => unreachable!("Can only get here with market orders"),
        };

        match OrderExecutionRules::run_all(order, &price, self) {
            Ok(trade) => {
                self.log.record(&trade);
                BrokerEvent::TradeSuccess(trade.clone())
            }
            Err(e) => e,
        }
    }

    fn execute_orders(&mut self, orders: Vec<Order>) -> Vec<BrokerEvent> {
        let mut res = Vec::new();
        for o in orders {
            let trade = self.execute_order(&o);
            res.push(trade);
        }
        res
    }
}

impl PendingOrders for SimulatedBroker {
    fn insert_order(&mut self, order: &Order) {
        self.orderbook.insert_order(order);
    }

    fn delete_order(&mut self, order_id: &u8) {
        self.orderbook.delete_order(order_id)
    }
}

impl ClientControlled for SimulatedBroker {
    fn get_positions(&self) -> Vec<String> {
        self.holdings.keys()
    }

    fn get_holdings(&self) -> PortfolioHoldings {
        self.holdings.clone()
    }

    fn get(&self, symbol: &str) -> Option<&PortfolioQty> {
        self.holdings.get(symbol)
    }

    fn update_holdings(&mut self, symbol: &str, change: &PortfolioQty) {
        self.holdings.insert(symbol, &*change);
    }
}

impl TradeCosts for SimulatedBroker {
    fn get_trade_costs(&self, trade: &Trade) -> CashValue {
        let mut cost = CashValue::default();
        for trade_cost in &self.trade_costs {
            cost += trade_cost.calc(trade);
        }
        cost
    }

    fn calc_trade_impact(
        &self,
        budget: &CashValue,
        price: &Price,
        is_buy: bool,
    ) -> (CashValue, Price) {
        BrokerCost::trade_impact_total(&self.trade_costs, budget, price, is_buy)
    }
}

impl PaysDividends for SimulatedBroker {
    fn pay_dividends(&mut self) {
        if let Some(dividends) = self.raw_data.get_dividends_by_date(&self.date) {
            for dividend in &dividends {
                //Our dataset can include dividends for stocks we don't own so we need to check
                //that we own the stock, not performant but can be changed later
                if let Some(qty) = self.get_position_qty(&dividend.symbol) {
                    let cash_value = *qty * dividend.value;
                    self.credit(cash_value);
                    self.log.record(dividend);
                }
            }
        }
    }
}

impl HasTime for SimulatedBroker {
    fn now(&self) -> DateTime {
        self.date
    }
}

impl HasLog for SimulatedBroker {
    fn trades_between(&self, start: &DateTime, end: &DateTime) -> Vec<Trade> {
        self.log.trades_between(start, end)
    }

    fn dividends_between(&self, start: &DateTime, end: &DateTime) -> Vec<Dividend> {
        self.log.dividends_between(start, end)
    }
}

impl SimulatedBroker {
    pub fn cost_basis(&self, symbol: &str) -> Option<Price> {
        self.log.cost_basis(symbol)
    }

    fn check_orderbook(&mut self) {
        //Should always return because we are running after we set a new date
        if let Some(quotes) = self.get_quotes() {
            for quote in quotes {
                if let Some(active_orders) = self.orderbook.check_orders_by_symbol(&quote) {
                    for (order_id, order) in active_orders {
                        let order = match order.get_order_type() {
                            OrderType::LimitBuy | OrderType::StopBuy => Order::new(
                                OrderType::MarketBuy,
                                quote.symbol.clone(),
                                order.get_shares(),
                                None,
                            ),
                            OrderType::LimitSell | OrderType::StopSell => Order::new(
                                OrderType::MarketSell,
                                quote.symbol.clone(),
                                order.get_shares(),
                                None,
                            ),
                            _ => panic!("Orderbook should have only non-market orders"),
                        };
                        let order_result = self.execute_order(&order);
                        //TODO: orders fail silently if the market order can't be executed
                        if let BrokerEvent::TradeSuccess(_t) = order_result {
                            self.orderbook.delete_order(&order_id);
                        }
                    }
                }
            }
        }
    }

    //Contains tasks that should be run on every iteration of the simulation irregardless of the
    //state on the client.
    //Right now, this largely consists of actions that the broker needs to perform i.e. checking if
    //an order has been triggered.
    pub fn set_date(&mut self, new_date: &DateTime) {
        self.date = *new_date;
        self.check_orderbook();
        self.pay_dividends();
    }

    pub fn new(raw_data: DataSource, trade_costs: Vec<BrokerCost>) -> SimulatedBroker {
        let holdings = PortfolioHoldings::new();
        let orderbook = SimOrderBook::new();
        let log = BrokerLog::new();

        SimulatedBroker {
            raw_data,
            //Intialised as invalid so errors throw if client tries to run before init
            date: DateTime::from(-1),
            holdings,
            orderbook,
            cash: CashValue::from(0.0),
            log,
            trade_costs,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::{PendingOrders, SimulatedBroker};
    use crate::broker::{BrokerCost, BrokerEvent, CashManager, Dividend, PositionInfo, Quote};
    use crate::broker::{Order, OrderExecutor, OrderType};
    use crate::data::{DataSource, DateTime};

    use std::collections::HashMap;

    fn setup() -> (SimulatedBroker, i64) {
        let mut prices: HashMap<DateTime, Vec<Quote>> = HashMap::new();
        let mut dividends: HashMap<DateTime, Vec<Dividend>> = HashMap::new();

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
            date: 102.into(),
            symbol: String::from("ABC"),
        };
        let quote5 = Quote {
            bid: 10.00.into(),
            ask: 11.00.into(),
            date: 102.into(),
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

        let mut dividend_row: Vec<Dividend> = Vec::new();
        let divi1 = Dividend {
            value: 5.0.into(),
            symbol: String::from("ABC"),
            date: 101.into(),
        };
        dividend_row.push(divi1);

        dividends.insert(101.into(), dividend_row);

        let source = DataSource::from_hashmap(prices, dividends);
        let brkr = SimulatedBroker::new(source, vec![BrokerCost::Flat(1.0.into())]);
        (brkr, 10)
    }

    #[test]
    fn test_cash_deposit_withdraw() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100.0.into());
        brkr.set_date(&100.into());

        //Test cash
        assert!(matches!(
            brkr.withdraw_cash(50.0.into()),
            BrokerEvent::WithdrawSuccess(..)
        ));
        assert!(matches!(
            brkr.withdraw_cash(51.0.into()),
            BrokerEvent::WithdrawFailure(..)
        ));
        assert!(matches!(
            brkr.deposit_cash(50.0.into()),
            BrokerEvent::DepositSuccess(..)
        ));

        //Test transactions
        assert!(matches!(
            brkr.debit(50.0.into()),
            BrokerEvent::TransactionSuccess
        ));
        assert!(matches!(
            brkr.debit(51.0.into()),
            BrokerEvent::TransactionFailure
        ));
        assert!(matches!(
            brkr.credit(50.0.into()),
            BrokerEvent::TransactionSuccess
        ));
    }

    #[test]
    fn test_that_successful_market_buy_order_reduces_cash() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.0.into());
        brkr.set_date(&100.into());

        let order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            495.00.into(),
            None,
        );
        let _res = brkr.execute_order(&order);

        let cash = brkr.get_cash_balance();
        assert!(cash < 100_000.0);
    }

    #[test]
    fn test_that_order_fails_without_cash_bubbling_correct_error() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100.0.into());
        brkr.set_date(&100.into());

        let order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            495.00.into(),
            None,
        );
        let res = brkr.execute_order(&order);

        let cash = brkr.get_cash_balance();

        assert!(cash == 100.0);
        assert!(matches!(res, BrokerEvent::TradeFailure(..)));
    }

    #[test]
    fn test_that_market_buy_increases_holdings() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.0.into());
        brkr.set_date(&100.into());

        let order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            495.00.into(),
            None,
        );
        let _res = brkr.execute_order(&order);

        let qty = *brkr.get_position_qty(&String::from("ABC")).unwrap();
        assert!(qty == 495.00);
    }

    #[test]
    fn test_that_market_sell_decreases_holdings() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.0.into());
        brkr.set_date(&100.into());

        let order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            495.00.into(),
            None,
        );
        let _res = brkr.execute_order(&order);

        let order1 = Order::new(
            OrderType::MarketSell,
            String::from("ABC"),
            295.00.into(),
            None,
        );
        let _res1 = brkr.execute_order(&order1);

        let qty = *brkr.get_position_qty(&String::from("ABC")).unwrap();
        assert!(qty == 200.00);
    }

    #[test]
    fn test_that_limit_order_increases_holdings_when_price_hits() {
        //This shouldn't just trigger but we must check that the
        //order executes at the market price, not the price of the limit
        //order

        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.0.into());
        brkr.set_date(&100.into());

        let order = Order::new(
            OrderType::LimitBuy,
            String::from("ABC"),
            495.00.into(),
            Some(102.00.into()),
        );
        let _res = brkr.insert_order(&order);

        brkr.set_date(&101.into());

        let qty = *brkr.get_position_qty(&String::from("ABC")).unwrap();
        let cost = brkr.cost_basis(&String::from("ABC")).unwrap();
        assert!(qty == 495.00);
        assert!(cost == 105.00);
    }

    #[test]
    fn test_that_stop_order_decreases_holdings_when_price_hits() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.0.into());
        brkr.set_date(&100.into());

        let entry_order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            500.0.into(),
            None,
        );
        let _res = brkr.execute_order(&entry_order);

        let stop_order = Order::new(
            OrderType::StopSell,
            String::from("ABC"),
            500.0.into(),
            Some(98.0.into()),
        );
        let _res1 = brkr.insert_order(&stop_order);
        brkr.set_date(&101.into());
        brkr.set_date(&102.into());
        brkr.set_date(&103.into());

        let qty = *brkr.get_position_qty(&String::from("ABC")).unwrap();
        assert!(qty == 0.0);
    }

    #[test]
    fn test_that_valuation_updates_in_next_period() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.0.into());
        brkr.set_date(&100.into());

        let order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            495.0.into(),
            None,
        );
        let _res = brkr.execute_order(&order);

        let val = brkr.get_position_value(&String::from("ABC")).unwrap();
        brkr.set_date(&101.into());
        let val1 = brkr.get_position_value(&String::from("ABC")).unwrap();
        assert_ne!(val, val1);
    }

    #[test]
    fn test_that_profit_calculation_is_accurate() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.0.into());
        brkr.set_date(&100.into());

        let order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            495.0.into(),
            None,
        );
        let _res = brkr.execute_order(&order);

        brkr.set_date(&101.into());
        let profit = brkr.get_position_profit(&String::from("ABC")).unwrap();
        assert!(profit == 1485.00);
    }

    #[test]
    fn test_that_order_for_non_existent_stock_returns_error() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.0.into());
        brkr.set_date(&100.into());

        //Ticker is not in the data
        let order = Order::new(
            OrderType::MarketBuy,
            String::from("XYZ"),
            495.0.into(),
            None,
        );
        let res = brkr.execute_order(&order);
        brkr.set_date(&101.into());

        let cash = brkr.get_cash_balance();
        assert!(cash == 100_000.0);
        assert!(matches!(res, BrokerEvent::TradeFailure(..)));
    }

    #[test]
    fn test_that_dividends_are_paid() {
        let (mut brkr, _) = setup();
        brkr.deposit_cash(100_000.0.into());
        brkr.set_date(&100.into());

        let order = Order::new(
            OrderType::MarketBuy,
            String::from("ABC"),
            100.0.into(),
            None,
        );
        brkr.execute_order(&order);

        let cash_before_dividend = brkr.get_cash_balance();
        brkr.set_date(&101.into());
        let cash_after_dividend = brkr.get_cash_balance();
        assert!(cash_before_dividend != cash_after_dividend);
    }
}
