use crate::input::{DataSource, Dividendable, Quotable};
use crate::types::CashValue;

#[derive(Debug)]
pub struct OrderBook {
    inner: std::collections::HashMap<
        super::DefaultExchangeOrderId,
        std::sync::Arc<super::ExchangeOrder>,
    >,
    last: super::DefaultExchangeOrderId,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            inner: std::collections::HashMap::new(),
            last: 0,
        }
    }

    pub fn delete_order(&mut self, order_id: super::DefaultExchangeOrderId) {
        self.inner.remove(&order_id);
    }

    pub fn insert_order(&mut self, order: super::ExchangeOrder) -> super::DefaultExchangeOrderId {
        let last = self.last;
        self.last = last + 1;
        self.inner.insert(last, std::sync::Arc::new(order));
        last
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn clear_orders_by_symbol(&mut self, symbol: &str) -> Vec<super::DefaultExchangeOrderId> {
        let mut to_remove = Vec::new();
        for (key, order) in self.inner.iter() {
            if order.get_symbol() == symbol {
                to_remove.push(*key);
            }
        }
        for key in &to_remove {
            self.delete_order(*key);
        }
        to_remove
    }

    pub fn execute_orders<Q, D, S>(
        &mut self,
        date: crate::types::DateTime,
        source: &S,
    ) -> Vec<super::ExchangeTrade>
    where
        Q: Quotable,
        D: Dividendable,
        S: DataSource<Q, D>,
    {
        let execute_buy = |quote: &Q, order: &super::ExchangeOrder| -> super::ExchangeTrade {
            let trade_price = quote.get_ask();
            let value = CashValue::from(**trade_price * *order.get_shares());
            super::ExchangeTrade::new(
                *order.get_subscriber_id(),
                order.get_symbol().to_string(),
                *value,
                *order.get_shares(),
                date,
                super::TradeType::Buy,
            )
        };

        let execute_sell = |quote: &Q, order: &super::ExchangeOrder| -> super::ExchangeTrade {
            let trade_price = quote.get_bid();
            let value = CashValue::from(**trade_price * *order.get_shares());
            super::ExchangeTrade::new(
                *order.get_subscriber_id(),
                order.get_symbol().to_string(),
                *value,
                *order.get_shares(),
                date,
                super::TradeType::Sell,
            )
        };

        let mut completed_orderids = Vec::new();
        let mut trade_results = Vec::new();
        if self.is_empty() {
            return trade_results;
        }

        //Execute orders in the orderbook
        for (key, order) in self.inner.iter() {
            let security_id = order.get_symbol();
            if let Some(quote) = source.get_quote(security_id) {
                let result = match order.get_order_type() {
                    super::OrderType::MarketBuy => Some(execute_buy(&quote, order)),
                    super::OrderType::MarketSell => Some(execute_sell(&quote, order)),
                    super::OrderType::LimitBuy => {
                        //Unwrap is safe because LimitBuy will always have a price
                        let order_price = order.get_price().as_ref().unwrap();
                        if order_price >= quote.get_ask() {
                            Some(execute_buy(&quote, order))
                        } else {
                            None
                        }
                    }
                    super::OrderType::LimitSell => {
                        //Unwrap is safe because LimitSell will always have a price
                        let order_price = order.get_price().as_ref().unwrap();
                        if order_price <= quote.get_bid() {
                            Some(execute_sell(&quote, order))
                        } else {
                            None
                        }
                    }
                    super::OrderType::StopBuy => {
                        //Unwrap is safe because StopBuy will always have a price
                        let order_price = order.get_price().as_ref().unwrap();
                        if order_price <= quote.get_ask() {
                            Some(execute_buy(&quote, order))
                        } else {
                            None
                        }
                    }
                    super::OrderType::StopSell => {
                        //Unwrap is safe because StopSell will always have a price
                        let order_price = order.get_price().as_ref().unwrap();
                        if order_price >= quote.get_bid() {
                            Some(execute_sell(&quote, order))
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
    use std::collections::HashMap;
    use std::sync::Arc;

    use crate::broker::Quote;
    use crate::exchange::orderbook::OrderBook;
    use crate::exchange::ExchangeOrder;
    use crate::input::{HashMapInput, QuotesHashMap};
    use crate::types::DateTime;

    fn setup() -> HashMapInput {
        let mut quotes: QuotesHashMap = HashMap::new();
        quotes.insert(
            DateTime::from(100),
            vec![Arc::new(Quote::new(101.00, 102.00, 100, "ABC"))],
        );
        quotes.insert(
            DateTime::from(101),
            vec![Arc::new(Quote::new(102.00, 103.00, 101, "ABC"))],
        );
        quotes.insert(
            DateTime::from(102),
            vec![Arc::new(Quote::new(105.00, 106.00, 102, "ABC"))],
        );

        let clock = crate::clock::ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&crate::types::Frequency::Second)
            .build();

        let source = crate::input::HashMapInputBuilder::new()
            .with_clock(clock.clone())
            .with_quotes(quotes)
            .build();

        source
    }

    #[test]
    fn test_that_multiple_orders_will_execute() {
        let source = setup();
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
        let source = setup();
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
        let source = setup();
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
        let source = setup();
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
        let source = setup();
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

        let source = setup();
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

        let source = setup();
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
        let source = setup();
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
        let mut quotes: QuotesHashMap = HashMap::new();
        quotes.insert(
            DateTime::from(100),
            vec![Arc::new(Quote::new(101.00, 102.00, 100, "ABC"))],
        );
        quotes.insert(DateTime::from(101), vec![]);
        quotes.insert(
            DateTime::from(102),
            vec![Arc::new(Quote::new(105.00, 106.00, 102, "ABC"))],
        );

        let mut clock = crate::clock::ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&crate::types::Frequency::Second)
            .build();

        let source = crate::input::HashMapInputBuilder::new()
            .with_clock(clock.clone())
            .with_quotes(quotes)
            .build();

        clock.tick();

        let mut orderbook = OrderBook::new();
        orderbook.insert_order(ExchangeOrder::market_buy(0, "ABC", 100.0));
        let orders = orderbook.execute_orders(101.into(), &source);
        //Trades cannot execute without prices
        assert_eq!(orders.len(), 0);
        assert!(!orderbook.is_empty());

        clock.tick();
        //Order executes now with prices
        let mut orders = orderbook.execute_orders(102.into(), &source);
        assert_eq!(orders.len(), 1);

        let trade = orders.pop().unwrap();
        assert_eq!(trade.value / trade.quantity, 106.00);
        assert_eq!(*trade.date, 102);
    }
}
