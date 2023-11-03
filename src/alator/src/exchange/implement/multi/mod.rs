//! Multi-threaded exchange
mod builder;

pub use builder::ConcurrentExchangeBuilder;

use std::sync::Arc;

use crate::clock::Clock;
#[allow(unused)]
use crate::exchange::implement::single::SingleExchange;
use crate::input::{PriceSource, Quotable};

/// Multi-threaded exchange. Created with [ConcurrentExchangeBuilder].
#[derive(Debug)]
pub struct ConcurrentExchange<Q, P>
where
    Q: Quotable,
    P: PriceSource<Q>,
{
    clock: Clock,
    orderbook: crate::exchange::OrderBook,
    price_source: P,
    trade_log: Vec<crate::exchange::types::ExchangeTrade>,
    price_sender: Vec<crate::exchange::types::PriceSender<Q>>,
    notify_sender: Vec<crate::exchange::types::NotifySender>,
    order_reciever: Vec<crate::exchange::types::OrderReciever>,
    last_subscriber_id: crate::exchange::types::DefaultSubscriberId,
}

unsafe impl<Q, P> Send for ConcurrentExchange<Q, P>
where
    Q: Quotable,
    P: PriceSource<Q>,
{
}

unsafe impl<Q, P> Sync for ConcurrentExchange<Q, P>
where
    Q: Quotable,
    P: PriceSource<Q>,
{
}

impl<Q, P> ConcurrentExchange<Q, P>
where
    Q: Quotable,
    P: PriceSource<Q>,
{
    pub fn new(clock: Clock, price_source: P) -> Self {
        Self {
            clock,
            orderbook: super::super::orderbook::OrderBook::new(),
            last_subscriber_id: 0,
            price_source,
            trade_log: Vec::new(),
            price_sender: Vec::new(),
            notify_sender: Vec::new(),
            order_reciever: Vec::new(),
        }
    }
}

impl<Q, P> ConcurrentExchange<Q, P>
where
    Q: Quotable,
    P: PriceSource<Q>,
{
    pub async fn subscribe(
        &mut self,
    ) -> (
        crate::exchange::types::DefaultSubscriberId,
        crate::exchange::types::PriceReceiver<Q>,
        crate::exchange::types::NotifyReceiver,
        crate::exchange::types::OrderSender,
    ) {
        let price_channel = tokio::sync::mpsc::channel::<Vec<Arc<Q>>>(100000);
        let notify_channel = tokio::sync::mpsc::channel::<
            crate::exchange::types::ExchangeNotificationMessage,
        >(100000);
        let order_channel =
            tokio::sync::mpsc::channel::<crate::exchange::types::ExchangeOrderMessage>(100000);

        //Initialize the price channel
        match self.price_source.get_quotes() {
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
        match self.price_source.get_quotes() {
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
        let mut recieved_orders = Vec::new();
        //Pull orders received over last tick and add them to book
        for order_reciever in self.order_reciever.iter_mut() {
            while let Ok(order_message) = order_reciever.try_recv() {
                recieved_orders.push(order_message);
            }
        }

        for message in recieved_orders {
            match message {
                crate::exchange::types::ExchangeOrderMessage::CreateOrder(order) => {
                    let order_id = self.orderbook.insert_order(order.clone());
                    let notifier = self
                        .notify_sender
                        .get(*order.get_subscriber_id() as usize)
                        .unwrap();
                    let _ = notifier
                        .send(
                            crate::exchange::types::ExchangeNotificationMessage::OrderBooked(
                                order_id, order,
                            ),
                        )
                        .await;
                }
                crate::exchange::types::ExchangeOrderMessage::DeleteOrder(
                    subscriber_id,
                    order_id,
                ) => {
                    //TODO: we don't check the subscriber_id so subscribers can delete orders
                    //for different subscribers
                    self.orderbook.delete_order(order_id);
                    let notifier = self.notify_sender.get(subscriber_id as usize).unwrap();
                    let _ = notifier
                        .send(
                            crate::exchange::types::ExchangeNotificationMessage::OrderDeleted(
                                order_id,
                            ),
                        )
                        .await;
                }
                crate::exchange::types::ExchangeOrderMessage::ClearOrdersBySymbol(
                    subscriber_id,
                    symbol,
                ) => {
                    //TODO: there is a bug here with operation ordering whereby an order can get executed before
                    //it gets cleared by a later operation
                    let removed = self.orderbook.clear_orders_by_symbol(symbol.as_str());
                    for order_id in removed {
                        let notifier = self.notify_sender.get(subscriber_id as usize).unwrap();
                        let _ = notifier
                            .send(
                                crate::exchange::types::ExchangeNotificationMessage::OrderDeleted(
                                    order_id,
                                ),
                            )
                            .await;
                    }
                }
            }
        }

        let now = self.clock.now();
        let executed_trades = self.orderbook.execute_orders(now, &self.price_source);
        for trade in executed_trades {
            //Should always be valid, so we can unwrap
            let notifier = self
                .notify_sender
                .get(trade.subscriber_id as usize)
                .unwrap();
            let _ = notifier
                .send(
                    crate::exchange::types::ExchangeNotificationMessage::TradeCompleted(
                        trade.clone(),
                    ),
                )
                .await;
            self.trade_log.push(trade);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConcurrentExchange, ConcurrentExchangeBuilder};
    use crate::broker::Quote;
    use crate::exchange::types::{
        DefaultSubscriberId, ExchangeNotificationMessage, ExchangeOrderMessage, NotifyReceiver,
        OrderSender, PriceReceiver,
    };
    use crate::input::DefaultPriceSource;

    async fn setup() -> (
        ConcurrentExchange<Quote, DefaultPriceSource>,
        DefaultSubscriberId,
        PriceReceiver<Quote>,
        OrderSender,
        NotifyReceiver,
    ) {
        let clock = crate::clock::ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&crate::types::Frequency::Second)
            .build();
        let mut price_source = DefaultPriceSource::new(clock.clone());
        price_source.add_quotes(101.00, 102.00, 100, "ABC");
        price_source.add_quotes(102.00, 103.00, 101, "ABC");
        price_source.add_quotes(105.00, 106.00, 102, "ABC");

        let mut exchange = ConcurrentExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_price_source(price_source)
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
            .send(ExchangeOrderMessage::market_sell(id, "ABC", 100.0))
            .await
            .unwrap();

        exchange.check().await;
        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 102
        assert_eq!(trade.value / trade.quantity, 102.00);
        assert_eq!(*trade.date, 101);
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
    async fn test_that_order_with_missing_price_executes_later() {
        let clock = crate::clock::ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&crate::types::Frequency::Second)
            .build();
        let mut price_source = DefaultPriceSource::new(clock.clone());
        price_source.add_quotes(101.00, 102.00, 100, "ABC");
        price_source.add_quotes(105.00, 106.00, 102, "ABC");

        let mut exchange = ConcurrentExchangeBuilder::new()
            .with_clock(clock.clone())
            .with_price_source(price_source)
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