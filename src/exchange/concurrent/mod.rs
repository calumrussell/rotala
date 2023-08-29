mod builder;

pub use builder::ConcurrentExchangeBuilder;

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

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
pub struct ConcurrentExchange<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    clock: Clock,
    orderbook: HashMap<super::types::DefaultExchangeOrderId, Arc<super::types::ExchangeOrder>>,
    last: super::types::DefaultExchangeOrderId,
    data_source: T,
    trade_log: Vec<super::types::ExchangeTrade>,
    price_sender: Vec<super::types::PriceSender<Q>>,
    notify_sender: Vec<super::types::NotifySender>,
    order_reciever: Vec<super::types::OrderReciever>,
    last_subscriber_id: super::types::DefaultSubscriberId,
    _dividend: PhantomData<D>,
}

unsafe impl<T, Q, D> Send for ConcurrentExchange<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
}

unsafe impl<T, Q, D> Sync for ConcurrentExchange<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
}

impl<T, Q, D> ConcurrentExchange<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    pub fn new(clock: Clock, data_source: T) -> Self {
        Self {
            clock,
            orderbook: HashMap::new(),
            last: 0,
            last_subscriber_id: 0,
            data_source,
            trade_log: Vec::new(),
            price_sender: Vec::new(),
            notify_sender: Vec::new(),
            order_reciever: Vec::new(),
            _dividend: PhantomData,
        }
    }
}

impl<T, Q, D> ConcurrentExchange<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    pub async fn subscribe(
        &mut self,
    ) -> (
        super::types::DefaultSubscriberId,
        super::types::PriceReceiver<Q>,
        super::types::NotifyReceiver,
        super::types::OrderSender,
    ) {
        let price_channel = tokio::sync::mpsc::channel::<Vec<Arc<Q>>>(100000);
        let notify_channel =
            tokio::sync::mpsc::channel::<super::types::ExchangeNotificationMessage>(100000);
        let order_channel =
            tokio::sync::mpsc::channel::<super::types::ExchangeOrderMessage>(100000);

        //Initialize the price channel
        match self.data_source.get_quotes() {
            Some(quotes) => price_channel.0.send(quotes).await.unwrap(),
            None => panic!("Missing data source, cannot initialize exchange"),
        };

        let subscriber_id = self.last_subscriber_id;
        self.last_subscriber_id += 1;

        self.price_sender.push(price_channel.0);
        self.notify_sender.push(notify_channel.0);
        self.order_reciever.push(order_channel.1);

        (
            subscriber_id,
            price_channel.1,
            notify_channel.1,
            order_channel.0,
        )
    }

    pub async fn check(&mut self) {
        //To eliminate lookahead bias, we only start executing orders on the next
        //tick.
        self.clock.tick();
        match self.data_source.get_quotes() {
            Some(quotes) => {
                for price_sender in &self.price_sender {
                    //TODO: clone here, don't have to optimize away but this is an owned value
                    //so shouldn't be necessary at all
                    let _ = price_sender.send(quotes.clone()).await;
                }
            }
            None => {
                //This usually represents an error with the calling code but can happen when
                //there are quotes missing for a date, so we don't throw panic!
                return;
            }
        }
        //orderbook only contains orders that are pending, once an order has been executed it is
        //removed from the orderbook so we can just check all orders here
        let mut executed_trades: Vec<super::types::ExchangeTrade> = Vec::new();
        let mut removed_keys: Vec<super::types::DefaultExchangeOrderId> = Vec::new();

        let mut tmp = Vec::new();
        //Pull orders received over last tick and add them to book
        for order_reciever in self.order_reciever.iter_mut() {
            while let Ok(order_message) = order_reciever.try_recv() {
                tmp.push(order_message);
            }
        }

        for message in tmp {
            match message {
                super::types::ExchangeOrderMessage::CreateOrder(order) => {
                    let order_id = self.insert_order(order.clone());
                    let notifier = self
                        .notify_sender
                        .get(*order.get_subscriber_id() as usize)
                        .unwrap();
                    let _ = notifier
                        .send(super::types::ExchangeNotificationMessage::OrderBooked(
                            order_id, order,
                        ))
                        .await;
                }
                super::types::ExchangeOrderMessage::DeleteOrder(subscriber_id, order_id) => {
                    //TODO: we don't check the subscriber_id so subscribers can delete orders
                    //for different subscribers
                    self.delete_order(order_id);
                    let notifier = self.notify_sender.get(subscriber_id as usize).unwrap();
                    let _ = notifier
                        .send(super::types::ExchangeNotificationMessage::OrderDeleted(
                            order_id,
                        ))
                        .await;
                }
                super::types::ExchangeOrderMessage::ClearOrdersBySymbol(subscriber_id, symbol) => {
                    //TODO: there is a bug here with operation ordering whereby an order can get executed before
                    //it gets cleared by a later operation
                    let removed = self.clear_orders_by_symbol(symbol.as_str());
                    for order_id in removed {
                        let notifier = self.notify_sender.get(subscriber_id as usize).unwrap();
                        let _ = notifier
                            .send(super::types::ExchangeNotificationMessage::OrderDeleted(
                                order_id,
                            ))
                            .await;
                    }
                }
            }
        }

        let execute_buy =
            |quote: &Q, order: &super::types::ExchangeOrder| -> super::types::ExchangeTrade {
                let trade_price = quote.get_ask();
                let value = CashValue::from(**trade_price * *order.get_shares());
                let date = self.clock.now();
                super::types::ExchangeTrade::new(
                    *order.get_subscriber_id(),
                    order.get_symbol().to_string(),
                    *value,
                    *order.get_shares(),
                    date,
                    super::types::TradeType::Buy,
                )
            };

        let execute_sell =
            |quote: &Q, order: &super::types::ExchangeOrder| -> super::types::ExchangeTrade {
                let trade_price = quote.get_bid();
                let value = CashValue::from(**trade_price * *order.get_shares());
                let date = self.clock.now();
                super::types::ExchangeTrade::new(
                    *order.get_subscriber_id(),
                    order.get_symbol().to_string(),
                    *value,
                    *order.get_shares(),
                    date,
                    super::types::TradeType::Sell,
                )
            };

        //Execute orders in the orderbook
        for (key, order) in self.orderbook.iter() {
            let security_id = order.get_symbol();
            if let Some(quote) = self.data_source.get_quote(security_id) {
                let result = match order.get_order_type() {
                    super::types::OrderType::MarketBuy => Some(execute_buy(&quote, order)),
                    super::types::OrderType::MarketSell => Some(execute_sell(&quote, order)),
                    super::types::OrderType::LimitBuy => {
                        //Unwrap is safe because LimitBuy will always have a price
                        let order_price = order.get_price().as_ref().unwrap();
                        if order_price < quote.get_ask() {
                            Some(execute_buy(&quote, order))
                        } else {
                            None
                        }
                    }
                    super::types::OrderType::LimitSell => {
                        //Unwrap is safe because LimitSell will always have a price
                        let order_price = order.get_price().as_ref().unwrap();
                        if order_price > quote.get_bid() {
                            Some(execute_sell(&quote, order))
                        } else {
                            None
                        }
                    }
                    super::types::OrderType::StopBuy => {
                        //Unwrap is safe because StopBuy will always have a price
                        let order_price = order.get_price().as_ref().unwrap();
                        if **quote.get_ask() > *order_price {
                            Some(execute_buy(&quote, order))
                        } else {
                            None
                        }
                    }
                    super::types::OrderType::StopSell => {
                        //Unwrap is safe because StopSell will always have a price
                        let order_price = order.get_price().as_ref().unwrap();
                        if **quote.get_bid() < *order_price {
                            Some(execute_sell(&quote, order))
                        } else {
                            None
                        }
                    }
                };

                if let Some(trade) = result {
                    //Should always be valid, so we can unwrap
                    let notifier = self
                        .notify_sender
                        .get(*order.get_subscriber_id() as usize)
                        .unwrap();
                    let _ = notifier
                        .send(super::types::ExchangeNotificationMessage::TradeCompleted(
                            trade.clone(),
                        ))
                        .await;
                    executed_trades.push(trade);
                    removed_keys.push(*key);
                }
            }
        }

        //Remove executed orders
        for key in removed_keys {
            self.delete_order(key)
        }
        self.trade_log.extend(executed_trades.clone());
    }

    fn delete_order(&mut self, order_id: super::types::DefaultExchangeOrderId) {
        self.orderbook.remove(&order_id);
    }

    fn insert_order(
        &mut self,
        order: super::types::ExchangeOrder,
    ) -> super::types::DefaultExchangeOrderId {
        let last = self.last;
        self.last = last + 1;
        self.orderbook.insert(last, Arc::new(order));
        last
    }

    pub fn is_empty(&self) -> bool {
        self.orderbook.is_empty()
    }

    fn clear_orders_by_symbol(
        &mut self,
        symbol: &str,
    ) -> Vec<super::types::DefaultExchangeOrderId> {
        let mut to_remove = Vec::new();
        for (key, order) in self.orderbook.iter() {
            if order.get_symbol() == symbol {
                to_remove.push(*key);
            }
        }
        for key in &to_remove {
            self.delete_order(*key);
        }
        to_remove
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::{ConcurrentExchange, ConcurrentExchangeBuilder};
    use crate::broker::{Dividend, Quote};
    use crate::exchange::types::{
        DefaultSubscriberId, ExchangeNotificationMessage, ExchangeOrderMessage, NotifyReceiver,
        OrderSender, PriceReceiver,
    };
    use crate::input::{HashMapInput, QuotesHashMap};
    use crate::types::DateTime;

    async fn setup() -> (
        ConcurrentExchange<HashMapInput, Quote, Dividend>,
        DefaultSubscriberId,
        PriceReceiver<Quote>,
        OrderSender,
        NotifyReceiver,
    ) {
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

        let mut exchange = ConcurrentExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_data_source(source)
            .build();

        let (id, price_rx, notify_rx, order_tx) = exchange.subscribe().await;
        (exchange, id, price_rx, order_tx, notify_rx)
    }

    #[tokio::test]
    async fn can_tick_without_blocking() {
        let (mut exchange, _id, _price_rx, _order_tx, _notify_rx) = setup().await;
        exchange.check().await;
        exchange.check().await;
        exchange.check().await;
    }

    #[tokio::test]
    async fn test_that_buy_market_executes_incrementing_trade_log() {
        let (mut exchange, id, _price_rx, order_tx, _notify_rx) = setup().await;
        order_tx
            .send(ExchangeOrderMessage::market_buy(id, "ABC", 100.0))
            .await
            .unwrap();
        exchange.check().await;

        assert_eq!(exchange.trade_log.len(), 1);
    }

    #[tokio::test]
    async fn test_that_exchange_emits_prices() {
        let (mut exchange, _id, mut price_rx, _order_tx, _notify_rx) = setup().await;

        exchange.check().await;
        while let Ok(prices) = price_rx.try_recv() {
            assert_eq!(prices.len(), 1);
        }
        //Check that len decrements when price is received
        exchange.check().await;
        while let Ok(prices) = price_rx.try_recv() {
            assert_eq!(prices.len(), 1);
            return;
        }
        assert!(true == false);
    }

    #[tokio::test]
    async fn test_that_exchange_notifies_completed_trades() {
        //This should be temporary as exchange needs to offer a variety of notifications
        let (mut exchange, id, _price_rx, order_tx, mut notify_rx) = setup().await;

        order_tx
            .send(ExchangeOrderMessage::market_buy(id, "ABC", 100.0))
            .await
            .unwrap();

        exchange.check().await;
        while let Ok(trade) = notify_rx.try_recv() {
            match trade {
                ExchangeNotificationMessage::TradeCompleted(trade) => assert_eq!(*trade.date, 101),
                _ => (),
            }
            return;
        }
        //Shouldn't hit this if there is a trade
        assert!(true == false);
    }

    #[tokio::test]
    async fn test_that_multiple_orders_are_executed_on_same_tick() {
        let (mut exchange, id, _price_rx, order_tx, _notify_rx) = setup().await;

        order_tx
            .send(ExchangeOrderMessage::market_buy(id, "ABC", 25.0))
            .await
            .unwrap();

        order_tx
            .send(ExchangeOrderMessage::market_buy(id, "ABC", 25.0))
            .await
            .unwrap();

        order_tx
            .send(ExchangeOrderMessage::market_buy(id, "ABC", 25.0))
            .await
            .unwrap();

        order_tx
            .send(ExchangeOrderMessage::market_buy(id, "ABC", 25.0))
            .await
            .unwrap();

        exchange.check().await;
        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[tokio::test]
    async fn test_that_multiple_orders_are_executed_on_consecutive_tick() {
        let (mut exchange, id, _price_rx, order_tx, _notify_rx) = setup().await;

        order_tx
            .send(ExchangeOrderMessage::market_buy(id, "ABC", 25.0))
            .await
            .unwrap();

        order_tx
            .send(ExchangeOrderMessage::market_buy(id, "ABC", 25.0))
            .await
            .unwrap();
        exchange.check().await;

        order_tx
            .send(ExchangeOrderMessage::market_buy(id, "ABC", 25.0))
            .await
            .unwrap();

        order_tx
            .send(ExchangeOrderMessage::market_buy(id, "ABC", 25.0))
            .await
            .unwrap();
        exchange.check().await;
        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[tokio::test]
    async fn test_that_buy_market_executes_on_next_tick() {
        //Verifies that trades do not execute instaneously removing lookahead bias
        let (mut exchange, id, _price_rx, order_tx, _notify_rx) = setup().await;

        order_tx
            .send(ExchangeOrderMessage::market_buy(id, "ABC", 100.0))
            .await
            .unwrap();

        exchange.check().await;
        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 103
        assert_eq!(trade.value / trade.quantity, 103.00);
        assert_eq!(*trade.date, 101);
    }

    #[tokio::test]
    async fn test_that_sell_market_executes_on_next_tick() {
        let (mut exchange, id, _price_rx, order_tx, _notify_rx) = setup().await;

        order_tx
            .send(ExchangeOrderMessage::market_buy(id, "ABC", 100.0))
            .await
            .unwrap();

        exchange.check().await;
        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 103
        assert_eq!(trade.value / trade.quantity, 103.00);
        assert_eq!(*trade.date, 101);
    }

    #[tokio::test]
    async fn test_that_buy_limit_triggers_correctly() {
        let (mut exchange, id, _price_rx, order_tx, _notify_rx) = setup().await;

        order_tx
            .send(ExchangeOrderMessage::limit_buy(id, "ABC", 100.0, 100.0))
            .await
            .unwrap();

        //This order has a price above the current price, so shouldn't execute
        order_tx
            .send(ExchangeOrderMessage::limit_buy(id, "ABC", 100.0, 105.0))
            .await
            .unwrap();

        exchange.check().await;
        assert_eq!(exchange.trade_log.len(), 1);
    }

    #[tokio::test]
    async fn test_that_sell_limit_triggers_correctly() {
        //This will execute even when the client doesn't hold any shares, this provides
        //functionality for shorting but the guards against generating revenue by fake
        //sales should be within broker
        let (mut exchange, id, _price_rx, order_tx, _notify_rx) = setup().await;

        order_tx
            .send(ExchangeOrderMessage::limit_sell(id, "ABC", 100.0, 100.0))
            .await
            .unwrap();

        //This order has a price above the current price, so shouldn't execute
        order_tx
            .send(ExchangeOrderMessage::limit_sell(id, "ABC", 100.0, 105.0))
            .await
            .unwrap();

        exchange.check().await;
        assert_eq!(exchange.trade_log.len(), 1);
    }

    #[tokio::test]
    async fn test_that_buy_stop_triggers_correctly() {
        //We are short from 90, and we put a StopBuy of 100 & 105 to take
        //off the position. If we are quoted 102/103 then our 100 order
        //should be executed.
        let (mut exchange, id, _price_rx, order_tx, _notify_rx) = setup().await;

        order_tx
            .send(ExchangeOrderMessage::stop_buy(id, "ABC", 100.0, 100.0))
            .await
            .unwrap();

        //This order has a price above the current price, so shouldn't execute
        order_tx
            .send(ExchangeOrderMessage::stop_buy(id, "ABC", 100.0, 105.0))
            .await
            .unwrap();

        exchange.check().await;
        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Should execute at market price, not order price
        assert_eq!(trade.value / trade.quantity, 103.00);
    }

    #[tokio::test]
    async fn test_that_sell_stop_triggers_correctly() {
        //Long from 110, we place orders to exit at 100 and 105.
        //If we are quoted 102/103 then our 105 StopSell is executed.
        let (mut exchange, id, _price_rx, order_tx, _notify_rx) = setup().await;

        order_tx
            .send(ExchangeOrderMessage::stop_sell(id, "ABC", 100.0, 100.0))
            .await
            .unwrap();

        //This order has a price above the current price, so shouldn't execute
        order_tx
            .send(ExchangeOrderMessage::stop_sell(id, "ABC", 100.0, 105.0))
            .await
            .unwrap();

        exchange.check().await;
        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Should execute at market price, not order price
        assert_eq!(trade.value / trade.quantity, 102.00);
    }

    #[tokio::test]
    async fn test_that_order_for_nonexistent_stock_fails_silently() {
        let (mut exchange, id, _price_rx, order_tx, _notify_rx) = setup().await;
        order_tx
            .send(ExchangeOrderMessage::market_buy(id, "XYZ", 100.0))
            .await
            .unwrap();
        exchange.check().await;

        assert_eq!(exchange.trade_log.len(), 0);
    }

    #[tokio::test]
    async fn test_that_orderbook_clears_by_symbol() {
        let (mut exchange, id, _price_rx, order_tx, _notify_rx) = setup().await;
        //This order should never execute, we are putting it in the book and then taking it away
        order_tx
            .send(ExchangeOrderMessage::limit_buy(id, "XYZ", 100.00, 200.0))
            .await
            .unwrap();
        exchange.check().await;

        assert!(!exchange.is_empty());

        order_tx
            .send(ExchangeOrderMessage::ClearOrdersBySymbol(id, "XYZ".into()))
            .await
            .unwrap();
        exchange.check().await;

        assert!(exchange.is_empty());
    }

    #[tokio::test]
    async fn test_that_order_with_missing_price_executes_later() {
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

        let clock = crate::clock::ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&crate::types::Frequency::Second)
            .build();

        let source = crate::input::HashMapInputBuilder::new()
            .with_clock(clock.clone())
            .with_quotes(quotes)
            .build();

        let mut exchange = ConcurrentExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_data_source(source)
            .build();

        let (id, _price_rx, _notify_rx, order_tx) = exchange.subscribe().await;

        order_tx
            .send(ExchangeOrderMessage::market_buy(id, "ABC", 100.00))
            .await
            .unwrap();

        exchange.check().await;

        //Orderbook should have one order and trade log has no executed trades
        assert_eq!(exchange.trade_log.len(), 0);

        exchange.check().await;

        //Order should execute now
        assert_eq!(exchange.trade_log.len(), 1);
    }
}
