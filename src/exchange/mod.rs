use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use crate::broker::{Order, OrderType, Trade, TradeType};
use crate::clock::Clock;
use crate::input::{DataSource, Dividendable, Quotable};
use crate::types::CashValue;

///Exchanges accept orders for securities, store them on an internal order book, and then execute
///them over time.
///
///[Exchange] does not execute orders immediately. [Broker] owns the exchange/s, passes orders to the
///exchange and then checks back to find out the result. Orders are not triggered by the broker but
///prie changes over time within the exchange. And the results are not reported back to broker
///immediately but must be polled.
///
///[Broker] polls a buffer of trades that is flushed empty after every check.
///
///This execution model is more complex than executing trades immediately with no exchange
///abstraction. But this creates clear separation between the responsibility of the broker between
///storing results and executing trades.
///
///This model also creates a dependency on the client to call all operations in order. For example,
///the client should not insert new orders into the book, check them, and then insert more orders.
///Clients must check, then insert new orders, then finish; ordering of operations should be
///maintained through state in the implementation.
pub trait Exchange<Q: Quotable> {
    fn insert_order(&mut self, order: Order) -> DefaultExchangeOrderId;
    fn delete_order(&mut self, order_id: DefaultExchangeOrderId);
    fn get_order(&self, order_id: &DefaultExchangeOrderId) -> Option<Arc<Order>>;
    fn check(&mut self);
    fn finish(&mut self);
    fn get_trade_log(&self) -> Vec<Trade>;
    //Represents size of orders in orderbook
    fn orderbook_size(&self) -> usize;
    fn flush_buffer(&mut self) -> Vec<Trade>;
    fn get_quote(&self, symbol: &str) -> Option<Arc<Q>>;
    fn get_quotes(&self) -> Option<Vec<Arc<Q>>>;
    fn clear(&mut self);
    fn clear_pending_market_orders_by_symbol(&mut self, symbol: &str);
}

///Exchange state maintains the consistency and timing of transactions. Intended for use with
///[DefaultExchange] implementation.
///
///If client attempts to call `insert_order` on [DefaultExchange] without the exchange being
///checked first - and existing trades on the orderbook being executed - then panic will be thrown.
///Panic is necessary because this would mean that calling client has a compile-time error in
///logic. Once check has been called [DefaultExchange] is [Ready], state is reset back to [Waiting]
///by finish.
#[derive(Clone, Debug)]
enum DefaultExchangeState {
    Waiting,
    Ready,
}

type DefaultExchangeOrderId = u32;

pub struct DefaultExchangeBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    data_source: Option<T>,
    clock: Option<Clock>,
    _quote: PhantomData<Q>,
    _dividend: PhantomData<D>,
}

impl<T, Q, D> DefaultExchangeBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    pub fn build(&self) -> DefaultExchange<T, Q, D> {
        if self.data_source.is_none() {
            panic!("Exchange must have data source");
        }

        if self.clock.is_none() {
            panic!("Exchange must have clock");
        }

        DefaultExchange::new(
            self.clock.as_ref().unwrap().clone(),
            self.data_source.as_ref().unwrap().clone(),
        )
    }

    pub fn with_clock(&mut self, clock: Clock) -> &mut Self {
        self.clock = Some(clock);
        self
    }

    pub fn with_data_source(&mut self, data_source: T) -> &mut Self {
        self.data_source = Some(data_source);
        self
    }

    pub fn new() -> Self {
        Self {
            clock: None,
            data_source: None,
            _quote: PhantomData,
            _dividend: PhantomData,
        }
    }
}

impl<T, Q, D> Default for DefaultExchangeBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct ExchangeInner<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    clock: Clock,
    orderbook: HashMap<DefaultExchangeOrderId, Arc<Order>>,
    last: DefaultExchangeOrderId,
    data_source: T,
    trade_log: Vec<Trade>,
    trade_buffer: Vec<Trade>,
    ready_state: DefaultExchangeState,
    last_seen_quote: HashMap<String, Arc<Q>>,
    _dividend: PhantomData<D>,
}

///Implementation of [Exchange]. Supports all [OrderType]. Generic implementation of the execution
///and updating logic of an exchange.
///
///If the client sends an order for a non-existent security or a spurious value, we will fail
///silently and do not execute the trade. [Broker] implementations should also attempt to catch
///these errors but errors are not bubbled up in order to keep the simulation running.
///
///If a price is missing then the client does not execute the trade but the order will stay in the
///book until it can be executed.
///
///In both cases, we are potentially creating silent errors but this more closely represents the
///execution model that would exist in reality.
#[derive(Debug)]
pub struct DefaultExchange<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    exchange: Arc<Mutex<ExchangeInner<T, Q, D>>>,
    _dividend: PhantomData<D>,
}

impl<T, Q, D> Clone for DefaultExchange<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn clone(&self) -> Self {
        Self {
            exchange: Arc::clone(&self.exchange),
            _dividend: PhantomData,
        }
    }
}

unsafe impl<T, Q, D> Send for DefaultExchange<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{}

unsafe impl<T, Q, D> Sync for DefaultExchange<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{}

impl<T, Q, D> DefaultExchange<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    pub fn new(clock: Clock, data_source: T) -> Self {
        Self {
            exchange: Arc::new(Mutex::new(ExchangeInner {
                clock,
                orderbook: HashMap::new(),
                last: 0,
                data_source,
                trade_log: Vec::new(),
                //This must be flushed by the broker every time the broker checks this creates a
                //dependency on the time but ready_state ensures that client can only call after we
                //have checked
                trade_buffer: Vec::new(),
                //Exchange is empty, so it must be ready to accept new orders.
                ready_state: DefaultExchangeState::Ready,
                last_seen_quote: HashMap::new(),
                _dividend: PhantomData,
            })),
            _dividend: PhantomData,
        }
    }
}

impl<T, Q, D> Exchange<Q> for DefaultExchange<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn get_quote(&self, symbol: &str) -> Option<Arc<Q>> {
        let exchange = self.exchange.lock().unwrap();
        if let Some(quote) = exchange.data_source.get_quote(symbol) {
            Some(quote)
        } else {
            if let Some(quote) = exchange.last_seen_quote.get(symbol) {
                return Some(quote.clone());
            }
            None
        }
    }

    fn get_quotes(&self) -> Option<Vec<Arc<Q>>> {
        let exchange = self.exchange.lock().unwrap();
        exchange.data_source.get_quotes()
    }

    fn flush_buffer(&mut self) -> Vec<Trade> {
        let mut exchange = self.exchange.lock().unwrap();
        match exchange.ready_state {
            DefaultExchangeState::Ready => {
                let copy = exchange.trade_buffer.clone();
                exchange.trade_buffer = Vec::new();
                copy
            }
            //We panic here because if this happens then it is impossible for the simulation to
            //continue and there is an error in the broker code
            DefaultExchangeState::Waiting => {
                panic!("called flush_buffer without first calling check");
            }
        }
    }

    fn finish(&mut self) {
        let mut exchange = self.exchange.lock().unwrap();
        exchange.ready_state = DefaultExchangeState::Waiting;
    }

    fn orderbook_size(&self) -> usize {
        let exchange = self.exchange.lock().unwrap();
        exchange.orderbook.keys().len()
    }

    fn get_order(&self, order_id: &DefaultExchangeOrderId) -> Option<Arc<Order>> {
        let exchange = self.exchange.lock().unwrap();
        exchange.orderbook.get(order_id).cloned()
    }

    fn get_trade_log(&self) -> Vec<Trade> {
        let exchange = self.exchange.lock().unwrap();
        exchange.trade_log.clone()
    }

    fn check(&mut self) {
        let exchange = self.exchange.lock().unwrap();
        //orderbook only contains orders that are pending, once an order has been executed it is
        //removed from the orderbook so we can just check all orders here
        let mut executed_trades: Vec<Trade> = Vec::new();
        let mut removed_keys: Vec<DefaultExchangeOrderId> = Vec::new();

        let execute_buy = |quote: &Q, order: &Order| -> Trade {
            let trade_price = quote.get_ask();
            let value = CashValue::from(**trade_price * **order.get_shares());
            let date = exchange.clock.now();
            Trade::new(
                order.get_symbol(),
                value,
                *order.get_shares().clone(),
                date,
                TradeType::Buy,
            )
        };

        let execute_sell = |quote: &Q, order: &Order| -> Trade {
            let trade_price = quote.get_bid();
            let value = CashValue::from(**trade_price * **order.get_shares());
            let date = exchange.clock.now();
            Trade::new(
                order.get_symbol(),
                value,
                *order.get_shares().clone(),
                date,
                TradeType::Sell,
            )
        };

        for (key, order) in exchange.orderbook.iter() {
            let security_id = order.get_symbol();
            if let Some(quote) = exchange.data_source.get_quote(security_id) {
                let result = match order.get_order_type() {
                    OrderType::MarketBuy => Some(execute_buy(&quote, order)),
                    OrderType::MarketSell => Some(execute_sell(&quote, order)),
                    OrderType::LimitBuy => {
                        //Unwrap is safe because LimitBuy will always have a price
                        let order_price = order.get_price().as_ref().unwrap();
                        if *order_price < *quote.get_ask() {
                            Some(execute_buy(&quote, order))
                        } else {
                            None
                        }
                    }
                    OrderType::LimitSell => {
                        //Unwrap is safe because LimitSell will always have a price
                        let order_price = order.get_price().as_ref().unwrap();
                        if *order_price > *quote.get_bid() {
                            Some(execute_sell(&quote, order))
                        } else {
                            None
                        }
                    }
                    OrderType::StopBuy => {
                        //Unwrap is safe because StopBuy will always have a price
                        let order_price = order.get_price().as_ref().unwrap();
                        if quote.get_ask() > order_price {
                            Some(execute_buy(&quote, order))
                        } else {
                            None
                        }
                    }
                    OrderType::StopSell => {
                        //Unwrap is safe because StopSell will always have a price
                        let order_price = order.get_price().as_ref().unwrap();
                        if quote.get_bid() < order_price {
                            Some(execute_sell(&quote, order))
                        } else {
                            None
                        }
                    }
                };

                if let Some(trade) = result {
                    executed_trades.push(trade);
                    removed_keys.push(*key);
                }
            }
        }

        drop(exchange);
        for key in removed_keys {
            self.delete_order(key)
        }

        let mut exchange_one = self.exchange.lock().unwrap();
        exchange_one.trade_log.extend(executed_trades.clone());
        //Update the buffer, which is flushed to broker, and the log which is held per-simulation
        exchange_one.trade_buffer.extend(executed_trades);
        exchange_one.ready_state = DefaultExchangeState::Ready;

        //Updating last_seen_quote with all the quotes seen on this date, potentially quite a
        //costly operation on every tick but this guarantees that missing prices don't cause a
        //panic that stops the simulation.
        if let Some(quotes) = exchange_one.data_source.get_quotes() {
            for quote in quotes {
                exchange_one.last_seen_quote
                    .insert(quote.get_symbol().clone(), quote.clone());
            }
        }
    }

    fn delete_order(&mut self, order_id: DefaultExchangeOrderId) {
        let mut exchange = self.exchange.lock().unwrap();
        exchange.orderbook.remove(&order_id);
    }

    fn insert_order(&mut self, order: Order) -> DefaultExchangeOrderId {
        let mut exchange = self.exchange.lock().unwrap();
        match exchange.ready_state {
            DefaultExchangeState::Ready => {
                let last = exchange.last;
                exchange.last = last + 1;
                exchange.orderbook.insert(last, Arc::new(order));
                last
            }
            DefaultExchangeState::Waiting => {
                //We panic here because if this happens then it is impossible for the simulation to
                //continue and there is an error in the broker code
                panic!("called insert_order without first calling check");
            }
        }
    }

    fn clear(&mut self) {
        let mut exchange = self.exchange.lock().unwrap();
        exchange.orderbook = HashMap::new();
    }

    fn clear_pending_market_orders_by_symbol(&mut self, symbol: &str) {
        let exchange = self.exchange.lock().unwrap();
        let mut to_remove = Vec::new();
        for (key, order) in exchange.orderbook.iter() {
            match order.get_order_type() {
                OrderType::MarketBuy | OrderType::MarketSell => {
                    if order.get_symbol() == symbol {
                        to_remove.push(*key);
                    }
                }
                _ => {}
            }
        }
        drop(exchange);
        for key in to_remove {
            self.delete_order(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::{DefaultExchange, DefaultExchangeBuilder};
    use crate::broker::{Dividend, Order, OrderType, Quote};
    use crate::clock::{Clock, ClockBuilder};
    use crate::exchange::Exchange;
    use crate::input::{HashMapInput, HashMapInputBuilder, QuotesHashMap};
    use crate::types::DateTime;

    fn setup() -> (DefaultExchange<HashMapInput, Quote, Dividend>, Clock) {
        let mut quotes: QuotesHashMap = HashMap::new();
        quotes.insert(
            DateTime::from(100),
            vec![Arc::new(Quote::new(101.00, 102.00, 100, "ABC"))],
        );
        quotes.insert(
            DateTime::from(101),
            vec![Arc::new(Quote::new(101.00, 102.00, 101, "ABC"))],
        );
        quotes.insert(
            DateTime::from(102),
            vec![Arc::new(Quote::new(105.00, 106.00, 102, "ABC"))],
        );

        let clock = ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&crate::types::Frequency::Second)
            .build();

        let source = HashMapInputBuilder::new()
            .with_clock(clock.clone())
            .with_quotes(quotes)
            .build();

        let exchange = DefaultExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_data_source(source)
            .build();

        (exchange, clock)
    }

    #[test]
    fn test_that_exchange_executing_order_decrements_len_increments_trade_log() {
        let order = Order::market(OrderType::MarketBuy, "ABC", 100.00);
        let (mut exchange, mut clock) = setup();

        exchange.insert_order(order);
        exchange.finish();

        assert_eq!(exchange.get_trade_log().len(), 0);
        assert_eq!(exchange.orderbook_size(), 1);

        clock.tick();
        exchange.check();

        println!("{:?}", exchange.get_trade_log());
        assert_eq!(exchange.get_trade_log().len(), 1);
        assert_eq!(exchange.orderbook_size(), 0);
    }

    #[should_panic]
    #[test]
    fn test_that_calling_insert_on_unchecked_exchange_causes_panic() {
        let order = Order::market(OrderType::MarketBuy, "ABC", 100.00);
        let (mut exchange, mut clock) = setup();

        exchange.finish();

        clock.tick();
        exchange.insert_order(order);
        exchange.check();
    }

    #[test]
    fn test_that_exchange_with_buy_market_triggers_correctly() {
        let order = Order::market(OrderType::MarketBuy, "ABC", 100.00);
        let (mut exchange, mut clock) = setup();

        exchange.insert_order(order);
        exchange.finish();

        clock.tick();
        exchange.check();

        println!("{:?}", exchange.get_trade_log());
        assert_eq!(exchange.get_trade_log().len(), 1);
    }

    #[test]
    fn test_that_exchange_with_buy_market_does_not_trigger_immediately() {
        //This is more of a timing issue with the clock but this validates that we don't see future
        //prices and that trading is sequential, not instantaneous
        let (mut exchange, mut clock) = setup();

        //We need to tick forward to enter the order in the period before the price changes
        clock.tick();
        exchange.check();
        let order = Order::market(OrderType::MarketBuy, "ABC", 100.00);
        exchange.insert_order(order);
        exchange.finish();

        //We only insert after check has been called
        clock.tick();
        exchange.check();

        println!("{:?}", exchange.get_trade_log());
        assert_eq!(exchange.get_trade_log().len(), 1);
        let trade = exchange.get_trade_log().first().unwrap().clone();
        println!("{:?}", trade);
        assert_eq!(*trade.value / *trade.quantity, 106.00);
    }

    #[test]
    fn test_that_exchange_with_sell_market_triggers_correctly() {
        let order = Order::market(OrderType::MarketSell, "ABC", 100.00);
        let (mut exchange, mut clock) = setup();

        exchange.insert_order(order);
        exchange.finish();

        clock.tick();
        exchange.check();

        println!("{:?}", exchange.get_trade_log());
        assert_eq!(exchange.get_trade_log().len(), 1);
    }

    #[test]
    fn test_that_exchange_with_buy_limit_triggers_correctly() {
        let order = Order::delayed(OrderType::LimitBuy, "ABC", 100.0, 100.0);
        let order0 = Order::delayed(OrderType::LimitBuy, "ABC", 100.0, 105.0);
        let (mut exchange, mut clock) = setup();

        exchange.insert_order(order);
        exchange.insert_order(order0);
        exchange.finish();

        clock.tick();
        exchange.check();

        println!("{:?}", exchange.get_trade_log());
        assert_eq!(exchange.get_trade_log().len(), 1);
    }

    #[test]
    fn test_that_exchange_with_sell_limit_triggers_correctly() {
        let order = Order::delayed(OrderType::LimitSell, "ABC", 100.0, 100.0);
        let order0 = Order::delayed(OrderType::LimitSell, "ABC", 100.0, 105.0);
        let (mut exchange, mut clock) = setup();

        exchange.insert_order(order);
        exchange.insert_order(order0);
        exchange.finish();

        clock.tick();
        exchange.check();

        println!("{:?}", exchange.get_trade_log());
        assert_eq!(exchange.get_trade_log().len(), 1);
    }

    #[test]
    fn test_that_exchange_with_buy_stop_triggers_correctly() {
        //We are short from 90, and we put a StopBuy of 100 & 105 to take
        //off the position. If we are quoted 101/102 then our 100 order
        //should be executed.
        let order = Order::delayed(OrderType::StopBuy, "ABC", 100.0, 100.0);
        let order0 = Order::delayed(OrderType::StopBuy, "ABC", 100.0, 105.0);
        let (mut exchange, mut clock) = setup();

        exchange.insert_order(order);
        exchange.insert_order(order0);
        exchange.finish();

        clock.tick();
        exchange.check();

        println!("{:?}", exchange.get_trade_log());
        assert_eq!(exchange.get_trade_log().len(), 1);
    }

    #[test]
    fn test_that_exchange_with_sell_stop_triggers_correctly() {
        //Long from 110, we place orders to exit at 100 and 105.
        //If we are quoted 101/102 then our 105 StopSell is executed.
        let order = Order::delayed(OrderType::StopSell, "ABC", 100.0, 100.0);
        let order0 = Order::delayed(OrderType::StopSell, "ABC", 100.0, 105.0);
        let (mut exchange, mut clock) = setup();

        exchange.insert_order(order);
        exchange.insert_order(order0);
        exchange.finish();

        clock.tick();
        exchange.check();

        println!("{:?}", exchange.get_trade_log());
        assert_eq!(exchange.get_trade_log().len(), 1);
    }

    #[test]
    fn test_that_delete_and_insert_dont_conflict() {
        let order = Order::delayed(OrderType::LimitBuy, "ABC", 100.0, 100.0);
        let order0 = Order::delayed(OrderType::LimitBuy, "ABC", 100.0, 105.0);
        let (mut exchange, mut clock) = setup();

        let order_id = exchange.insert_order(order);
        exchange.delete_order(order_id);
        exchange.insert_order(order0);
        exchange.finish();

        clock.tick();
        exchange.check();

        println!("{:?}", exchange.get_trade_log());
        assert_eq!(exchange.orderbook_size(), 1);
    }

    #[test]
    fn test_that_order_for_nonexistent_stock_fails_silently() {
        let order = Order::delayed(OrderType::LimitBuy, "XYZ", 100.0, 100.0);
        let (mut exchange, mut clock) = setup();

        exchange.insert_order(order);
        exchange.finish();

        clock.tick();
        exchange.check();

        println!("{:?}", exchange.get_trade_log());
        assert_eq!(exchange.get_trade_log().len(), 0);
        assert_eq!(exchange.orderbook_size(), 1);
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

        let mut clock = ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&crate::types::Frequency::Second)
            .build();

        let source = HashMapInputBuilder::new()
            .with_clock(clock.clone())
            .with_quotes(quotes)
            .build();

        let mut exchange = DefaultExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_data_source(source)
            .build();

        let order = Order::market(OrderType::MarketBuy, "ABC", 100.00);
        exchange.insert_order(order);
        exchange.finish();

        clock.tick();
        exchange.check();

        //Orderbook should have one order and trade log has no executed trades
        assert_eq!(exchange.get_trade_log().len(), 0);
        assert_eq!(exchange.orderbook_size(), 1);

        clock.tick();
        exchange.check();

        //Order should execute now
        assert_eq!(exchange.get_trade_log().len(), 1);
        assert_eq!(exchange.orderbook_size(), 0);
    }
}
