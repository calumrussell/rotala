pub mod orderbook;

use std::sync::Mutex;
use std::sync::atomic::AtomicU64;

use tonic::{transport::Server, Request, Response, Status};

use exchange::exchange_server::{Exchange, ExchangeServer};
use exchange::{ RegisterSourceRequest, RegisterSourceReply, SendOrderReply, SendOrderRequest, DeleteOrderRequest, DeleteOrderReply };
use orderbook::{OrderBook, ExchangeOrder};

pub mod exchange {
    tonic::include_proto!("exchange");
}

pub struct DefaultExchange {
    orderbook: Mutex<OrderBook>,
    consumer_id: AtomicU64,
}

impl Default for DefaultExchange {
    fn default() -> Self {
        DefaultExchange {
            orderbook: Mutex::new(OrderBook::new()),
            consumer_id: AtomicU64::new(0)
        }
    }
}

#[tonic::async_trait]
impl Exchange for DefaultExchange {
    async fn register_source(
        &self,
        _request: Request<RegisterSourceRequest>,
    ) -> Result<Response<RegisterSourceReply>, Status> {
        let curr = self.consumer_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Response::new(RegisterSourceReply {
            source_id: curr,
        }))
    }

    async fn send_order(
        &self,
        request: Request<SendOrderRequest>,
    ) -> Result<Response<SendOrderReply>, Status> {
        let inner = request.into_inner();
        let source_id = inner.source_id;
        if let Some(order) = inner.order {
            let mut orderbook = self.orderbook.lock().unwrap();
            let order_id = orderbook.insert_order(ExchangeOrder { 
                symbol: order.symbol,
                shares: order.quantity,
                price: order.price,
                order_type: order.r#type.into(),
                subscriber_id: source_id,

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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();
    let exchange = DefaultExchange::default();

    println!("DefaultExchange listening on {}", addr);

    Server::builder()
        .add_service(ExchangeServer::new(exchange))
        .serve(addr)
        .await?;

    Ok(())
}