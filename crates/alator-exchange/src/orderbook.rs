use std::collections::HashMap;

use crate::input::DefaultPriceSource;
use crate::{ ExchangeOrder, ExchangeTrade, OrderType, TradeType, Quote};

#[doc(hidden)]
#[derive(Debug)]
pub (crate) struct OrderBook {
    inner: HashMap<u64, ExchangeOrder>,
    last: u64,
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            inner: std::collections::HashMap::new(),
            last: 0,
        }
    }

    pub fn delete_order(&mut self, order_id: u64) {
        self.inner.remove(&order_id);
    }

    pub fn insert_order(&mut self, order: ExchangeOrder) -> u64 {
        let last = self.last;
        self.last = last + 1;
        self.inner.insert(last, order);
        last
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn clear_orders_by_symbol(&mut self, symbol: &str) -> Vec<u64> {
        let mut to_remove = Vec::new();
        for (key, order) in self.inner.iter() {
            if order.symbol == symbol {
                to_remove.push(*key);
            }
        }
        for key in &to_remove {
            self.delete_order(*key);
        }
        to_remove
    }

    pub fn execute_orders(&mut self, date: i64, source: &DefaultPriceSource) -> Vec<ExchangeTrade> {
        let execute_buy = |quote: &Quote, order: &ExchangeOrder| -> ExchangeTrade {
            let trade_price = quote.ask;
            let value = trade_price * order.shares;
            ExchangeTrade {
                subscriber_id: order.subscriber_id,
                symbol: order.symbol.to_string(),
                value,
                quantity: order.shares,
                date: date.into(),
                typ: TradeType::Buy,
            }
        };

        let execute_sell = |quote: &Quote, order: &ExchangeOrder| -> ExchangeTrade {
            let trade_price = quote.bid;
            let value = trade_price * order.shares;
            ExchangeTrade {
                subscriber_id: order.subscriber_id,
                symbol: order.symbol.to_string(),
                value,
                quantity: order.shares,
                date: date.into(),
                typ: TradeType::Sell,
            }
        };

        let mut completed_orderids = Vec::new();
        let mut trade_results = Vec::new();
        if self.is_empty() {
            return trade_results;
        }

        //Execute orders in the orderbook
        for (key, order) in self.inner.iter() {
            let security_id = &order.symbol;
            if let Some(quote) = source.get_quote(&date, security_id) {
                let result = match order.order_type {
                    OrderType::MarketBuy => Some(execute_buy(quote, order)),
                    OrderType::MarketSell => Some(execute_sell(quote, order)),
                    OrderType::LimitBuy => {
                        //Unwrap is safe because LimitBuy will always have a price
                        let order_price = order.price;
                        if order_price >= Some(quote.ask) {
                            Some(execute_buy(quote, order))
                        } else {
                            None
                        }
                    }
                    OrderType::LimitSell => {
                        //Unwrap is safe because LimitSell will always have a price
                        let order_price = order.price;
                        if order_price <= Some(quote.bid) {
                            Some(execute_sell(quote, order))
                        } else {
                            None
                        }
                    }
                    OrderType::StopBuy => {
                        //Unwrap is safe because StopBuy will always have a price
                        let order_price = order.price;
                        if order_price <= Some(quote.ask) {
                            Some(execute_buy(quote, order))
                        } else {
                            None
                        }
                    }
                    OrderType::StopSell => {
                        //Unwrap is safe because StopSell will always have a price
                        let order_price = order.price;
                        if order_price >= Some(quote.bid) {
                            Some(execute_sell(quote, order))
                        } else {
                            None
                        }
                    }
                };
                if let Some(trade) = &result {
                    completed_orderids.push(*key);
                    trade_results.push(trade.clone());
                }
            }
        }
        for order_id in completed_orderids {
            self.delete_order(order_id);
        }
        trade_results
    }
}

#[cfg(test)]
mod tests {
    use crate::input::DefaultPriceSource;
    use crate::types::ExchangeOrder;
    use super::OrderBook;
    use alator_clock::{Clock, ClockBuilder, Frequency};

    fn setup() -> (Clock, DefaultPriceSource) {
        let clock = ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&Frequency::Second)
            .build();

        let mut price_source = DefaultPriceSource::new();
        price_source.add_quotes(101.0, 102.00, 100, "ABC".to_string());
        price_source.add_quotes(102.0, 103.00, 101, "ABC".to_string());
        price_source.add_quotes(105.0, 106.00, 102, "ABC".to_string());
        (clock, price_source)
    }

    #[test]
    fn test_that_multiple_orders_will_execute() {
        let (_clock, source) = setup();
        let mut orderbook = OrderBook::new();

        orderbook.insert_order(ExchangeOrder::market_buy(0, "ABC", 25.0));
        orderbook.insert_order(ExchangeOrder::market_buy(0, "ABC", 25.0));
        orderbook.insert_order(ExchangeOrder::market_buy(0, "ABC", 25.0));
        orderbook.insert_order(ExchangeOrder::market_buy(0, "ABC", 25.0));

        let executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.len(), 4);
    }

    #[test]
    fn test_that_buy_market_executes() {
        let (_clock, source) = setup();
        let mut orderbook = OrderBook::new();

        orderbook.insert_order(ExchangeOrder::market_buy(0, "ABC", 100.0));
        let mut executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.len(), 1);

        let trade = executed.pop().unwrap();
        //Trade executes at 100 so trade price should be 102
        assert_eq!(trade.value / trade.quantity, 102.00);
        assert_eq!(*trade.date, 100);
    }

    #[test]
    fn test_that_sell_market_executes() {
        let (_clock, source) = setup();
        let mut orderbook = OrderBook::new();

        orderbook.insert_order(ExchangeOrder::market_sell(0, "ABC", 100.0));
        let mut executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.len(), 1);

        let trade = executed.pop().unwrap();
        //Trade executes at 100 so trade price should be 101
        assert_eq!(trade.value / trade.quantity, 101.00);
        assert_eq!(*trade.date, 100);
    }

    #[test]
    fn test_that_buy_limit_triggers_correctly() {
        let (_clock, source) = setup();
        let mut orderbook = OrderBook::new();

        orderbook.insert_order(ExchangeOrder::limit_buy(0, "ABC", 100.0, 95.0));
        orderbook.insert_order(ExchangeOrder::limit_buy(0, "ABC", 100.0, 105.0));
        let mut executed = orderbook.execute_orders(100.into(), &source);
        //Only one order should execute on this tick
        assert_eq!(executed.len(), 1);

        let trade = executed.pop().unwrap();
        //Limit order has price of 105 but should execute at the ask, which is 102
        assert_eq!(trade.value / trade.quantity, 102.00);
        assert_eq!(*trade.date, 100);
    }

    #[test]
    fn test_that_sell_limit_triggers_correctly() {
        let (_clock, source) = setup();
        let mut orderbook = OrderBook::new();

        orderbook.insert_order(ExchangeOrder::limit_sell(0, "ABC", 100.0, 95.0));
        orderbook.insert_order(ExchangeOrder::limit_sell(0, "ABC", 100.0, 105.0));
        let mut executed = orderbook.execute_orders(100.into(), &source);
        //Only one order should execute on this tick
        assert_eq!(executed.len(), 1);

        let trade = executed.pop().unwrap();
        //Limit order has price of 95 but should execute at the ask, which is 101
        assert_eq!(trade.value / trade.quantity, 101.00);
        assert_eq!(*trade.date, 100);
    }

    #[test]
    fn test_that_buy_stop_triggers_correctly() {
        //We are short from 90, and we put a StopBuy of 95 & 105 to take
        //off the position. If we are quoted 101/102 then 95 order
        //should be executed.

        let (_clock, source) = setup();
        let mut orderbook = OrderBook::new();

        orderbook.insert_order(ExchangeOrder::stop_buy(0, "ABC", 100.0, 95.0));
        orderbook.insert_order(ExchangeOrder::stop_buy(0, "ABC", 100.0, 105.0));
        let mut executed = orderbook.execute_orders(100.into(), &source);
        //Only one order should execute on this tick
        assert_eq!(executed.len(), 1);

        let trade = executed.pop().unwrap();
        //Stop order has price of 103 but should execute at the ask, which is 102
        assert_eq!(trade.value / trade.quantity, 102.00);
        assert_eq!(*trade.date, 100);
    }

    #[test]
    fn test_that_sell_stop_triggers_correctly() {
        //Long from 110, we place orders to exit at 100 and 105.
        //If we are quoted 101/102 then our 105 StopSell is executed.

        let (_clock, source) = setup();
        let mut orderbook = OrderBook::new();

        orderbook.insert_order(ExchangeOrder::stop_buy(0, "ABC", 100.0, 99.0));
        orderbook.insert_order(ExchangeOrder::stop_buy(0, "ABC", 100.0, 105.0));
        let mut executed = orderbook.execute_orders(100.into(), &source);
        //Only one order should execute on this tick
        assert_eq!(executed.len(), 1);

        let trade = executed.pop().unwrap();
        //Stop order has price of 105 but should execute at the ask, which is 102
        assert_eq!(trade.value / trade.quantity, 102.00);
        assert_eq!(*trade.date, 100);
    }

    #[test]
    fn test_that_order_for_nonexistent_stock_fails_silently() {
        let (_clock, source) = setup();
        let mut orderbook = OrderBook::new();

        orderbook.insert_order(ExchangeOrder::market_buy(0, "XYZ", 100.0));
        let executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.len(), 0);
    }

    #[test]
    fn test_that_orderbook_clears_by_symbol() {
        let _source = setup();
        let mut orderbook = OrderBook::new();

        orderbook.insert_order(ExchangeOrder::limit_buy(0, "XYZ", 100.0, 200.0));

        assert!(!orderbook.is_empty());

        orderbook.clear_orders_by_symbol("XYZ");
        assert!(orderbook.is_empty());
    }

    #[test]
    fn test_that_order_with_missing_price_executes_later() {
        let mut clock = ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&Frequency::Second)
            .build();

        let mut price_source = DefaultPriceSource::new();
        price_source.add_quotes(101.00, 102.00, 100, "ABC".to_string());
        price_source.add_quotes(105.00, 106.00, 102, "ABC".to_string());

        clock.tick();

        let mut orderbook = OrderBook::new();
        orderbook.insert_order(ExchangeOrder::market_buy(0, "ABC", 100.0));
        let orders = orderbook.execute_orders(101.into(), &price_source);
        //Trades cannot execute without prices
        assert_eq!(orders.len(), 0);
        assert!(!orderbook.is_empty());

        clock.tick();
        //Order executes now with prices
        let mut orders = orderbook.execute_orders(102.into(), &price_source);
        assert_eq!(orders.len(), 1);

        let trade = orders.pop().unwrap();
        assert_eq!(trade.value / trade.quantity, 106.00);
        assert_eq!(*trade.date, 102);
    }
}
