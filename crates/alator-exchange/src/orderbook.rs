use std::collections::HashMap;

#[derive(Clone)]
pub struct Quote {
    pub bid: f64,
    pub ask: f64,
    pub date: i64,
    pub symbol: String,
}

pub struct DefaultPriceSource {
    inner: HashMap<i64, HashMap<String, Quote>>,
}

impl DefaultPriceSource {
    pub fn get_quote(&self, date: &i64, symbol: &str) -> Option<&Quote> {
        if let Some(date_row) = self.inner.get(date) {
            if let Some(quote) = date_row.get(symbol) {
                return Some(quote);
            }
        }
        None
    }

    pub fn get_quotes(&self, date: &i64) -> Option<Vec<Quote>> {
        if let Some(date_row) = self.inner.get(date) {
            return Some(date_row.values().cloned().collect());
        }
        None
    }

    pub fn add_quotes(&mut self, bid: f64, ask: f64, date: i64, symbol: String) {
        let quote = Quote {
            bid,
            ask,
            date,
            symbol: symbol.clone(),
        };

        if let Some(date_row) = self.inner.get_mut(&date) {
            date_row.insert(symbol.clone(), quote);
        } else {
            let mut date_row = HashMap::new();
            date_row.insert(symbol, quote);
            self.inner.insert(date, date_row);
        }
    }

    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }
}

impl Default for DefaultPriceSource {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderType {
    MarketSell,
    MarketBuy,
    LimitSell,
    LimitBuy,
    StopSell,
    StopBuy,
}

impl From<i32> for OrderType {
    fn from(value: i32) -> Self {
        match value {
            0 => OrderType::MarketSell,
            1 => OrderType::MarketBuy,
            2 => OrderType::LimitSell,
            3 => OrderType::LimitBuy,
            4 => OrderType::StopSell,
            5 => OrderType::StopBuy,
            _ => unimplemented!("0/1/2/3 are only types supported"),
        }
    }
}

impl From<OrderType> for i32 {
    fn from(value: OrderType) -> Self {
        match value {
            OrderType::MarketSell => 0,
            OrderType::MarketBuy => 1,
            OrderType::LimitSell => 2,
            OrderType::LimitBuy => 3,
            OrderType::StopSell => 4,
            OrderType::StopBuy => 5,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExchangeOrder {
    pub subscriber_id: u64,
    pub order_type: OrderType,
    pub symbol: String,
    pub shares: f64,
    pub price: Option<f64>,
}

impl ExchangeOrder {
    fn market(
        subscriber_id: u64,
        order_type: OrderType,
        symbol: impl Into<String>,
        shares: f64,
    ) -> Self {
        Self {
            subscriber_id,
            order_type,
            symbol: symbol.into(),
            shares,
            price: None,
        }
    }

    fn delayed(
        subscriber_id: u64,
        order_type: OrderType,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
    ) -> Self {
        Self {
            subscriber_id,
            order_type,
            symbol: symbol.into(),
            shares,
            price: Some(price),
        }
    }

    pub fn market_buy(subscriber_id: u64, symbol: impl Into<String>, shares: f64) -> Self {
        ExchangeOrder::market(subscriber_id, OrderType::MarketBuy, symbol, shares)
    }

    pub fn market_sell(subscriber_id: u64, symbol: impl Into<String>, shares: f64) -> Self {
        ExchangeOrder::market(subscriber_id, OrderType::MarketSell, symbol, shares)
    }

    pub fn stop_buy(
        subscriber_id: u64,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
    ) -> Self {
        ExchangeOrder::delayed(subscriber_id, OrderType::StopBuy, symbol, shares, price)
    }

    pub fn stop_sell(
        subscriber_id: u64,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
    ) -> Self {
        ExchangeOrder::delayed(subscriber_id, OrderType::StopSell, symbol, shares, price)
    }

    pub fn limit_buy(
        subscriber_id: u64,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
    ) -> Self {
        ExchangeOrder::delayed(subscriber_id, OrderType::LimitBuy, symbol, shares, price)
    }

    pub fn limit_sell(
        subscriber_id: u64,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
    ) -> Self {
        ExchangeOrder::delayed(subscriber_id, OrderType::LimitSell, symbol, shares, price)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TradeType {
    Buy,
    Sell,
}

#[derive(Clone, Debug)]
pub struct ExchangeTrade {
    pub subscriber_id: u64,
    pub symbol: String,
    pub value: f64,
    pub quantity: f64,
    pub date: i64,
    pub typ: TradeType,
}

#[doc(hidden)]
#[derive(Debug)]
pub struct OrderBook {
    inner: std::collections::HashMap<u64, ExchangeOrder>,
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
                date,
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
                date,
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
    use super::DefaultPriceSource;
    use super::ExchangeOrder;
    use super::OrderBook;
    use alator::clock::{Clock, ClockBuilder};

    fn setup() -> (Clock, DefaultPriceSource) {
        let clock = ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&alator::types::Frequency::Second)
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
        assert_eq!(trade.date, 100);
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
        assert_eq!(trade.date, 100);
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
        assert_eq!(trade.date, 100);
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
        assert_eq!(trade.date, 100);
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
        assert_eq!(trade.date, 100);
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
        assert_eq!(trade.date, 100);
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
        let mut clock = alator::clock::ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&alator::types::Frequency::Second)
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
        assert_eq!(trade.date, 102);
    }
}
