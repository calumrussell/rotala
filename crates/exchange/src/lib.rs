use std::sync::{atomic::AtomicU64, Mutex};
use tonic::{Request, Response, Status};

use orderbook::{OrderBook, DefaultPriceSource, ExchangeTrade};

pub mod orderbook;

pub mod proto {
    tonic::include_proto!("exchange");
}

impl From<orderbook::Quote> for proto::Quote {
    fn from(value: orderbook::Quote) -> Self {
        Self { 
            bid: value.bid, 
            ask: value.ask, 
            date: value.date, 
            symbol: value.symbol 
        }
    }
}

impl From<orderbook::ExchangeTrade> for proto::Trade {
    fn from(value: orderbook::ExchangeTrade) -> Self {
        Self {
            price: value.value / value.quantity,
            quantity: value.quantity,
            value: value.value,
            date: value.date,
            symbol: value.symbol,
        }
    }
}
pub struct DefaultExchange {
    orderbook: Mutex<OrderBook>,
    subscriber_id: AtomicU64,
    source: DefaultPriceSource,
    clock: Mutex<alator::clock::Clock>,
    trades: Mutex<Vec<ExchangeTrade>>,
    subscriber_count: AtomicU64,
    ticked_count: AtomicU64,
}

impl DefaultExchange {
    pub fn new (clock: alator::clock::Clock, source: DefaultPriceSource) -> Self {
        Self {
            orderbook: Mutex::new(OrderBook::new()),
            subscriber_id: AtomicU64::new(0),
            source,
            clock: Mutex::new(clock),
            trades: Mutex::new(Vec::new()),
            subscriber_count: AtomicU64::new(0),
            ticked_count: AtomicU64::new(0),
        }
    }
}

#[tonic::async_trait]
impl proto::exchange_server::Exchange for DefaultExchange {
    async fn register_source(
        &self,
        _request: Request<proto::RegisterSourceRequest>,
    ) -> Result<Response<proto::RegisterSourceReply>, Status> {
        let curr = self.subscriber_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.subscriber_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        Ok(Response::new(proto::RegisterSourceReply {
            subscriber_id: curr,
        }))
    }

    async fn send_order(
        &self,
        request: Request<proto::SendOrderRequest>,
    ) -> Result<Response<proto::SendOrderReply>, Status> {
        let inner = request.into_inner();
        let subscriber_id = inner.subscriber_id;
        if let Some(order) = inner.order {
            let mut orderbook = self.orderbook.lock().unwrap();
            let order_id = orderbook.insert_order(orderbook::ExchangeOrder { 
                symbol: order.symbol,
                shares: order.quantity,
                price: order.price,
                order_type: order.r#type.into(),
                subscriber_id,

            });
            return Ok(Response::new(proto::SendOrderReply { order_id }));
        }
        Ok(Response::new(proto::SendOrderReply { order_id: 0 }))
    }

    async fn delete_order(
        &self,
        request: Request<proto::DeleteOrderRequest>,
    ) -> Result<Response<proto::DeleteOrderReply>, Status> {
        let inner = request.into_inner();
        let order_id = inner.order_id;

        let mut orderbook = self.orderbook.lock().unwrap();
        orderbook.delete_order(order_id);
        Ok(Response::new(proto::DeleteOrderReply { order_id }))
    }

    async fn fetch_trades(
        &self,
        request: Request<proto::FetchTradesRequest>,
    ) -> Result<Response<proto::FetchTradesReply>, Status> {
        let trades = self.trades.lock().unwrap();
        let formatted_trades: Vec<proto::Trade> = trades.iter().map(|v| v.clone().into()).collect();
        Ok(Response::new( proto::FetchTradesReply { trades: formatted_trades }))
    }

    async fn fetch_quotes(
        &self,
        request: Request<proto::FetchQuotesRequest>,
    ) -> Result<Response<proto::FetchQuotesReply>, Status> {
        let now = self.clock.lock().unwrap().now();
        if let Some(quotes) = self.source.get_quotes(&now) {
            let formatted_quotes: Vec<proto::Quote> = quotes.iter().map(|v| v.clone().into()).collect();
            return Ok(Response::new(proto::FetchQuotesReply { quotes: formatted_quotes }));
        }
        Ok(Response::new(proto::FetchQuotesReply { quotes: Vec::new() }))
    }

    async fn tick(
        &self,
        request: Request<proto::TickRequest>,
    ) -> Result<Response<proto::TickReply>, Status> {
        self.ticked_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Ok(val) = self.ticked_count.compare_exchange(self.subscriber_count.load(std::sync::atomic::Ordering::Acquire), 0, std::sync::atomic::Ordering::Acquire, std::sync::atomic::Ordering::Relaxed) {
            //If all subscribers have ticked then we move forward
            let mut orderbook = self.orderbook.lock().unwrap();
            let mut clock = self.clock.lock().unwrap();
            let now = clock.now();
            let mut executed_trades = orderbook.execute_orders(*now, &self.source);
            
            let mut trades = self.trades.lock().unwrap();
            trades.clear();
            trades.append(&mut executed_trades);

            clock.tick();
        }
        Ok(Response::new(proto::TickReply {  }))
    }
}