use core::panic;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use log::info;

use crate::broker::{BrokerCashEvent, Dividend, Order, OrderType, Quote, Trade, TradeType};
use crate::clock::Clock;
use crate::input::{
    DataSource, Dividendable, DividendsHashMap, HashMapInput, Quotable, QuotesHashMap,
};
use crate::types::{CashValue, PortfolioQty};

pub type PriceSender<Q> = tokio::sync::broadcast::Sender<Vec<Arc<Q>>>;
pub type PriceReceiver<Q> = tokio::sync::broadcast::Receiver<Vec<Arc<Q>>>;
pub type NotifySender = tokio::sync::broadcast::Sender<Trade>;
pub type NotifyReceiver = tokio::sync::broadcast::Receiver<Trade>;
pub type OrderSender = tokio::sync::mpsc::Sender<Order>;
pub type OrderReciever = tokio::sync::mpsc::Receiver<Order>;

type DefaultExchangeOrderId = u32;

pub struct DefaultExchangeBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    data_source: Option<T>,
    clock: Option<Clock>,
    price_sender: Option<PriceSender<Q>>,
    notify_sender: Option<NotifySender>,
    order_reciever: Option<OrderReciever>,
    _quote: PhantomData<Q>,
    _dividend: PhantomData<D>,
}

impl<T, Q, D> DefaultExchangeBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    pub fn build(&mut self) -> DefaultExchange<T, Q, D> {
        if self.data_source.is_none() {
            panic!("Exchange must have data source");
        }

        if self.clock.is_none() {
            panic!("Exchange must have clock");
        }

        if self.order_reciever.is_none() {
            panic!("Exchange must have a channel to receive orders");
        }

        if self.price_sender.is_none() {
            panic!("Exchange must have a channel to send prices");
        }

        if self.notify_sender.is_none() {
            panic!("Exchange must have a channel to notify results");
        }

        let order_reciever = std::mem::replace(&mut self.order_reciever, None);

        DefaultExchange::new(
            self.clock.as_ref().unwrap().clone(),
            self.data_source.as_ref().unwrap().clone(),
            self.price_sender.as_ref().unwrap().clone(),
            self.notify_sender.as_ref().unwrap().clone(),
            order_reciever.unwrap(),
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

    pub fn with_price_sender(&mut self, price_sender: PriceSender<Q>) -> &mut Self {
        self.price_sender = Some(price_sender);
        self
    }

    pub fn with_order_reciever(&mut self, order_reciever: OrderReciever) -> &mut Self {
        self.order_reciever = Some(order_reciever);
        self
    }

    pub fn with_notify_sender(&mut self, notify_sender: NotifySender) -> &mut Self {
        self.notify_sender = Some(notify_sender);
        self
    }

    pub fn new() -> Self {
        Self {
            clock: None,
            data_source: None,
            price_sender: None,
            notify_sender: None,
            order_reciever: None,
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
    clock: Clock,
    orderbook: HashMap<DefaultExchangeOrderId, Arc<Order>>,
    last: DefaultExchangeOrderId,
    data_source: T,
    trade_log: Vec<Trade>,
    price_sender: PriceSender<Q>,
    notify_sender: NotifySender,
    order_reciever: OrderReciever,
    _dividend: PhantomData<D>,
}

unsafe impl<T, Q, D> Send for DefaultExchange<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
}

unsafe impl<T, Q, D> Sync for DefaultExchange<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
}

impl<T, Q, D> DefaultExchange<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    pub fn new(
        clock: Clock,
        data_source: T,
        price_sender: PriceSender<Q>,
        notify_sender: NotifySender,
        order_reciever: OrderReciever,
    ) -> Self {
        Self {
            clock,
            orderbook: HashMap::new(),
            last: 0,
            data_source,
            trade_log: Vec::new(),
            price_sender,
            notify_sender,
            order_reciever,
            _dividend: PhantomData,
        }
    }
}

impl<T, Q, D> DefaultExchange<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    pub async fn check(&mut self) {
        //To eliminate lookahead bias, we only start executing orders on the next
        //tick.
        self.clock.tick();
        let quotes = self.data_source.get_quotes();
        self.price_sender.send(quotes.unwrap());

        //orderbook only contains orders that are pending, once an order has been executed it is
        //removed from the orderbook so we can just check all orders here
        let mut executed_trades: Vec<Trade> = Vec::new();
        let mut removed_keys: Vec<DefaultExchangeOrderId> = Vec::new();

        let mut pending_orders = Vec::new();

        //Pull the orders from the queue
        while let Some(order) = self.order_reciever.recv().await {
            pending_orders.push(order);
        }

        for order in &pending_orders {
            self.insert_order(order.clone());
        }

        let execute_buy = |quote: &Q, order: &Order| -> Trade {
            let trade_price = quote.get_ask();
            let value = CashValue::from(**trade_price * **order.get_shares());
            let date = self.clock.now();
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
            let date = self.clock.now();
            Trade::new(
                order.get_symbol(),
                value,
                *order.get_shares().clone(),
                date,
                TradeType::Sell,
            )
        };

        //Execute orders
        for (key, order) in self.orderbook.iter() {
            let security_id = order.get_symbol();
            if let Some(quote) = self.data_source.get_quote(security_id) {
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

        //Notify executions
        for trade in &executed_trades {
            self.notify_sender.send(trade.clone());
        }

        //Remove executed orders
        for key in removed_keys {
            self.delete_order(key)
        }
        self.trade_log.extend(executed_trades.clone());
    }

    fn delete_order(&mut self, order_id: DefaultExchangeOrderId) {
        self.orderbook.remove(&order_id);
    }

    fn insert_order(&mut self, order: Order) -> DefaultExchangeOrderId {
        let last = self.last;
        self.last = last + 1;
        self.orderbook.insert(last, Arc::new(order));
        last
    }

    fn clear(&mut self) {
        self.orderbook = HashMap::new();
    }

    fn clear_pending_market_orders_by_symbol(&mut self, symbol: &str) {
        let mut to_remove = Vec::new();
        for (key, order) in self.orderbook.iter() {
            match order.get_order_type() {
                OrderType::MarketBuy | OrderType::MarketSell => {
                    if order.get_symbol() == symbol {
                        to_remove.push(*key);
                    }
                }
                _ => {}
            }
        }
        for key in to_remove {
            self.delete_order(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use tokio::join;

    use super::{DefaultExchange, DefaultExchangeBuilder};
    use crate::broker::{Dividend, Order, OrderType, Quote, Trade};
    use crate::clock::{Clock, ClockBuilder};
    use crate::input::{HashMapInput, HashMapInputBuilder, QuotesHashMap};
    use crate::types::DateTime;

    fn setup() -> (
        DefaultExchange<HashMapInput, Quote, Dividend>,
        Clock,
        tokio::sync::broadcast::Receiver<Vec<Arc<Quote>>>,
        tokio::sync::mpsc::Sender<Order>,
        tokio::sync::broadcast::Receiver<Trade>,
    ) {
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

        let clock = crate::clock::ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&crate::types::Frequency::Second)
            .build();

        let source = crate::input::HashMapInputBuilder::new()
            .with_clock(clock.clone())
            .with_quotes(quotes)
            .build();

        let (price_tx, price_rx) = tokio::sync::broadcast::channel::<Vec<Arc<Quote>>>(100);
        let (notify_tx, notify_rx) = tokio::sync::broadcast::channel::<Trade>(100);
        let (order_tx, order_rx) = tokio::sync::mpsc::channel::<Order>(100);

        let exchange = DefaultExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_data_source(source)
            .with_notify_sender(notify_tx)
            .with_order_reciever(order_rx)
            .with_price_sender(price_tx)
            .build();
        (exchange, clock, price_rx, order_tx, notify_rx)
    }

    #[tokio::test]
    async fn test_that_trade_executes() {
        let (mut exchange, clock, price_rx, order_tx, notify_rx) = setup();

        tokio::spawn(async move {
            order_tx
                .send(Order::market(OrderType::MarketBuy, "ABC", 100.0))
                .await
                .unwrap();
        });

        join!(exchange.check());
        dbg!(exchange.trade_log);
        assert!(true == false);
    }
}
