pub mod orderbook;

use std::sync::Mutex;
use std::sync::atomic::AtomicU64;

use tonic::{transport::Server, Request, Response, Status};

use exchange::exchange_server::{Exchange, ExchangeServer};
use exchange::{ RegisterSourceRequest, RegisterSourceReply, SendOrderReply, SendOrderRequest, DeleteOrderRequest, DeleteOrderReply };
use orderbook::{OrderBook, ExchangeOrder, DefaultPriceSource, ExchangeTrade};

pub mod exchange {
    tonic::include_proto!("exchange");
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
    fn new (clock: alator::clock::Clock, source: DefaultPriceSource) -> Self {
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
impl Exchange for DefaultExchange {
    async fn register_source(
        &self,
        _request: Request<RegisterSourceRequest>,
    ) -> Result<Response<RegisterSourceReply>, Status> {
        let curr = self.subscriber_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.subscriber_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        Ok(Response::new(RegisterSourceReply {
            subscriber_id: curr,
        }))
    }

    async fn send_order(
        &self,
        request: Request<SendOrderRequest>,
    ) -> Result<Response<SendOrderReply>, Status> {
        let inner = request.into_inner();
        let subscriber_id = inner.subscriber_id;
        if let Some(order) = inner.order {
            let mut orderbook = self.orderbook.lock().unwrap();
            let order_id = orderbook.insert_order(ExchangeOrder { 
                symbol: order.symbol,
                shares: order.quantity,
                price: order.price,
                order_type: order.r#type.into(),
                subscriber_id,

            });
            return Ok(Response::new(SendOrderReply { order_id }));
        }
        Ok(Response::new(SendOrderReply { order_id: 0 }))
    }

    async fn delete_order(
        &self,
        request: Request<DeleteOrderRequest>,
    ) -> Result<Response<DeleteOrderReply>, Status> {
        let inner = request.into_inner();
        let order_id = inner.order_id;

        let mut orderbook = self.orderbook.lock().unwrap();
        orderbook.delete_order(order_id);
        Ok(Response::new(DeleteOrderReply { order_id }))
    }

    async fn fetch_trades(
        &self,
        request: Request<crate::exchange::FetchTradesRequest>,
    ) -> Result<Response<crate::exchange::FetchTradesReply>, Status> {
        let trades = self.trades.lock().unwrap();
        let formatted_trades: Vec<crate::exchange::Trade> = trades.iter().map(|v| v.clone().into()).collect();
        Ok(Response::new( crate::exchange::FetchTradesReply { trades: formatted_trades }))
    }

    async fn fetch_quotes(
        &self,
        request: Request<crate::exchange::FetchQuotesRequest>,
    ) -> Result<Response<crate::exchange::FetchQuotesReply>, Status> {
        let now = self.clock.lock().unwrap().now();
        if let Some(quotes) = self.source.get_quotes(&now) {
            let formatted_quotes: Vec<crate::exchange::Quote> = quotes.iter().map(|v| v.clone().into()).collect();
            return Ok(Response::new(crate::exchange::FetchQuotesReply { quotes: formatted_quotes }));
        }
        Ok(Response::new(crate::exchange::FetchQuotesReply { quotes: Vec::new() }))
    }

    async fn tick(
        &self,
        request: Request<crate::exchange::TickRequest>,
    ) -> Result<Response<crate::exchange::TickReply>, Status> {
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
        Ok(Response::new(crate::exchange::TickReply {}))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();
    let clock = alator::clock::ClockBuilder::with_length_in_seconds(100, 1000)
        .with_frequency(&alator::types::Frequency::Second)
        .build();

    let mut source = DefaultPriceSource::new();
    for date in clock.peek() {
       source.add_quotes(100.0, 101.0, *date, "ABC".to_string());
    }

    let exchange = DefaultExchange::new(clock, source);

    println!("DefaultExchange listening on {}", addr);

    Server::builder()
        .add_service(ExchangeServer::new(exchange))
        .serve(addr)
        .await?;

    Ok(())
}