//! Single-threaded exchange
mod builder;

pub use builder::SingleExchangeBuilder;

use std::marker::PhantomData;
use std::sync::Arc;

use crate::clock::Clock;
use crate::input::{PriceSource, Quotable};

/// Single-threaded exchange. Created with [SingleExchangeBuilder].
#[derive(Debug)]
pub struct SingleExchange<Q, P>
where
    Q: Quotable,
    P: PriceSource<Q>,
{
    clock: Clock,
    orderbook: crate::exchange::OrderBook,
    price_source: P,
    trade_log: Vec<crate::exchange::types::ExchangeTrade>,
    //This is cleared on every tick
    order_buffer: Vec<crate::exchange::types::ExchangeOrder>,
    _quote: PhantomData<Q>,
}

impl<Q, P> SingleExchange<Q, P>
where
    Q: Quotable,
    P: PriceSource<Q>,
{
    pub fn new(clock: Clock, price_source: P) -> Self {
        Self {
            clock,
            orderbook: super::super::orderbook::OrderBook::new(),
            price_source,
            trade_log: Vec::new(),
            order_buffer: Vec::new(),
            _quote: PhantomData,
        }
    }
}

impl<Q, P> SingleExchange<Q, P>
where
    Q: Quotable,
    P: PriceSource<Q>,
{
    pub fn fetch_quotes(&self) -> Vec<Arc<Q>> {
        if let Some(quotes) = self.price_source.get_quotes() {
            return quotes;
        }
        vec![]
    }

    pub fn fetch_trades(&self, from: usize) -> &[crate::exchange::ExchangeTrade] {
        &self.trade_log[from..]
    }

    pub fn insert_order(&mut self, order: super::super::types::ExchangeOrder) {
        self.order_buffer.push(order);
    }

    pub fn delete_order(&mut self, order_id: super::super::types::DefaultExchangeOrderId) {
        self.orderbook.delete_order(order_id);
    }

    pub fn clear_orders_by_symbol(&mut self, symbol: String) {
        self.orderbook.clear_orders_by_symbol(&symbol);
    }

    pub fn check(&mut self) -> Vec<super::super::types::ExchangeTrade> {
        //To eliminate lookahead bias, we only start executing orders on the next
        //tick.
        self.clock.tick();

        for order in &self.order_buffer {
            self.orderbook.insert_order(order.clone());
        }

        let now = self.clock.now();
        let executed_trades = self.orderbook.execute_orders(now, &self.price_source);
        self.trade_log.extend(executed_trades.clone());
        self.order_buffer.clear();
        executed_trades
    }
}

#[cfg(test)]
mod tests {
    use crate::broker::Quote;
    use crate::exchange::ExchangeOrder;
    use crate::input::DefaultPriceSource;

    use super::{SingleExchange, SingleExchangeBuilder};

    fn setup() -> SingleExchange<Quote, DefaultPriceSource> {
        let clock = crate::clock::ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&crate::types::Frequency::Second)
            .build();
        let mut price_source = DefaultPriceSource::new(clock.clone());
        price_source.add_quotes(101.00, 102.00, 100, "ABC");
        price_source.add_quotes(102.00, 103.00, 101, "ABC");
        price_source.add_quotes(105.00, 106.00, 102, "ABC");

        let exchange = SingleExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_price_source(price_source)
            .build();

        exchange
    }

    #[test]
    fn test_that_buy_market_executes_incrementing_trade_log() {
        let mut exchange = setup();

        exchange.insert_order(ExchangeOrder::market_buy(0, "ABC", 100.0));
        exchange.check();

        //TODO: no abstraction!
        assert_eq!(exchange.trade_log.len(), 1);
    }

    #[test]
    fn test_that_multiple_orders_are_executed_on_same_tick() {
        let mut exchange = setup();

        exchange.insert_order(ExchangeOrder::market_buy(0, "ABC", 25.0));
        exchange.insert_order(ExchangeOrder::market_buy(0, "ABC", 25.0));
        exchange.insert_order(ExchangeOrder::market_buy(0, "ABC", 25.0));
        exchange.insert_order(ExchangeOrder::market_buy(0, "ABC", 25.0));

        exchange.check();
        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_multiple_orders_are_executed_on_consecutive_tick() {
        let mut exchange = setup();
        exchange.insert_order(ExchangeOrder::market_buy(0, "ABC", 25.0));
        exchange.insert_order(ExchangeOrder::market_buy(0, "ABC", 25.0));
        exchange.check();

        exchange.insert_order(ExchangeOrder::market_buy(0, "ABC", 25.0));
        exchange.insert_order(ExchangeOrder::market_buy(0, "ABC", 25.0));
        exchange.check();

        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_buy_market_executes_on_next_tick() {
        //Verifies that trades do not execute instaneously removing lookahead bias
        let mut exchange = setup();

        exchange.insert_order(ExchangeOrder::market_buy(0, "ABC", 100.0));
        exchange.check();

        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 103
        assert_eq!(trade.value / trade.quantity, 103.00);
        assert_eq!(*trade.date, 101);
    }

    #[test]
    fn test_that_sell_market_executes_on_next_tick() {
        //Verifies that trades do not execute instaneously removing lookahead bias
        let mut exchange = setup();

        exchange.insert_order(ExchangeOrder::market_sell(0, "ABC", 100.0));
        exchange.check();

        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 103
        assert_eq!(trade.value / trade.quantity, 102.00);
        assert_eq!(*trade.date, 101);
    }

    #[test]
    fn test_that_order_for_nonexistent_stock_fails_silently() {
        let mut exchange = setup();

        exchange.insert_order(ExchangeOrder::market_buy(0, "XYZ", 100.0));
        exchange.check();

        assert_eq!(exchange.trade_log.len(), 0);
    }

    #[test]
    fn test_that_order_buffer_clears() {
        //Sounds redundant but accidentally removing the clear could cause unusual errors elsewhere
        let mut exchange = setup();

        exchange.insert_order(ExchangeOrder::market_buy(0, "ABC", 100.0));
        exchange.check();

        assert!(exchange.order_buffer.is_empty());
    }

    #[test]
    fn test_that_order_with_missing_price_executes_later() {
        let clock = crate::clock::ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&crate::types::Frequency::Second)
            .build();
        let mut price_source = DefaultPriceSource::new(clock.clone());
        price_source.add_quotes(101.00, 102.00, 100, "ABC");
        price_source.add_quotes(105.00, 106.00, 102, "ABC");

        let mut exchange = SingleExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_price_source(price_source)
            .build();

        exchange.insert_order(ExchangeOrder::market_buy(0, "ABC", 100.0));
        exchange.check();
        //Orderbook should have one order and trade log has no executed trades
        assert_eq!(exchange.trade_log.len(), 0);

        exchange.check();
        //Order should execute now
        assert_eq!(exchange.trade_log.len(), 1);
    }
}
