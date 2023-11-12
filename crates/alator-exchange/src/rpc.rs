use std::sync::{atomic::AtomicU64, Mutex};
use tonic::{Request, Response, Status, transport::Channel};

use crate::ExchangeAsync;
use crate::input::DefaultPriceSource;
use crate::orderbook::OrderBook;
use crate::types::proto::exchange_client::ExchangeClient;

pub struct DefaultExchange {
    orderbook: Mutex<OrderBook>,
    subscriber_id: AtomicU64,
    source: DefaultPriceSource,
    clock: Mutex<alator::clock::Clock>,
    trades: Mutex<Vec<crate::ExchangeTrade>>,
    subscriber_count: AtomicU64,
    ticked_count: AtomicU64,
}

impl DefaultExchange {
    pub fn new(clock: alator::clock::Clock, source: DefaultPriceSource) -> Self {
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
impl crate::types::proto::exchange_server::Exchange for DefaultExchange {
    async fn register_source(
        &self,
        _request: Request<crate::types::proto::RegisterSourceRequest>,
    ) -> Result<Response<crate::types::proto::RegisterSourceReply>, Status> {
        let curr = self
            .subscriber_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.subscriber_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        Ok(Response::new(crate::types::proto::RegisterSourceReply {
            subscriber_id: curr,
        }))
    }

    async fn send_order(
        &self,
        request: Request<crate::types::proto::SendOrderRequest>,
    ) -> Result<Response<crate::types::proto::SendOrderReply>, Status> {
        let inner = request.into_inner();
        let subscriber_id = inner.subscriber_id;
        if let Some(order) = inner.order {
            let mut orderbook = self.orderbook.lock().unwrap();
            let order_id = orderbook.insert_order(crate::ExchangeOrder {
                symbol: order.symbol,
                shares: order.quantity,
                price: order.price,
                order_type: order.order_type.into(),
                subscriber_id,
            });
            return Ok(Response::new(crate::types::proto::SendOrderReply { order_id }));
        }
        Ok(Response::new(crate::types::proto::SendOrderReply { order_id: 0 }))
    }

    async fn delete_order(
        &self,
        request: Request<crate::types::proto::DeleteOrderRequest>,
    ) -> Result<Response<crate::types::proto::DeleteOrderReply>, Status> {
        let inner = request.into_inner();
        let order_id = inner.order_id;

        let mut orderbook = self.orderbook.lock().unwrap();
        orderbook.delete_order(order_id);
        Ok(Response::new(crate::types::proto::DeleteOrderReply { order_id }))
    }

    async fn fetch_trades(
        &self,
        _request: Request<crate::types::proto::FetchTradesRequest>,
    ) -> Result<Response<crate::types::proto::FetchTradesReply>, Status> {
        let trades = self.trades.lock().unwrap();
        let formatted_trades: Vec<crate::types::proto::Trade> = trades.iter().map(|v| v.clone().into()).collect();
        Ok(Response::new(crate::types::proto::FetchTradesReply {
            trades: formatted_trades,
        }))
    }

    async fn fetch_quotes(
        &self,
        _request: Request<crate::types::proto::FetchQuotesRequest>,
    ) -> Result<Response<crate::types::proto::FetchQuotesReply>, Status> {
        let now = self.clock.lock().unwrap().now();
        if let Some(quotes) = self.source.get_quotes(&now) {
            let formatted_quotes: Vec<crate::types::proto::Quote> =
                quotes.iter().map(|v| v.clone().into()).collect();
            return Ok(Response::new(crate::types::proto::FetchQuotesReply {
                quotes: formatted_quotes,
            }));
        }
        Ok(Response::new(crate::types::proto::FetchQuotesReply {
            quotes: Vec::new(),
        }))
    }

    async fn tick(
        &self,
        _request: Request<crate::types::proto::TickRequest>,
    ) -> Result<Response<crate::types::proto::TickReply>, Status> {
        self.ticked_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Ok(_val) = self.ticked_count.compare_exchange(
            self.subscriber_count
                .load(std::sync::atomic::Ordering::Acquire),
            0,
            std::sync::atomic::Ordering::Acquire,
            std::sync::atomic::Ordering::Relaxed,
        ) {
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
        Ok(Response::new(crate::types::proto::TickReply {}))
    }
}

pub struct RPCExchange  {
    client: ExchangeClient<Channel>,
}

impl RPCExchange {
    pub fn build_exchange_server(clock: alator::clock::Clock, source: DefaultPriceSource) -> crate::types::proto::exchange_server::ExchangeServer<DefaultExchange> {
        crate::types::proto::exchange_server::ExchangeServer::new(DefaultExchange::new(clock, source))
    }

    pub fn new(client: ExchangeClient<Channel>) -> Self {
        Self {
            client,
        }
    }
}

#[tonic::async_trait]
impl ExchangeAsync for RPCExchange {
    async fn register_source(&mut self) -> Result<u64, Box<dyn std::error::Error>> {
        let subscriber_id_resp = self.client
            .register_source(Request::new(crate::types::proto::RegisterSourceRequest {}))
            .await?;
        let subscriber_id = subscriber_id_resp.into_inner().subscriber_id;
        Ok(subscriber_id)
    }

    async fn send_order(&mut self, subscriber_id: u64, order: crate::ExchangeOrder) -> Result<u64, Box<dyn std::error::Error>> {

        let proto_order = crate::types::proto::Order {
            symbol: order.symbol,
            order_type: order.order_type.into(),
            quantity: order.shares,
            price: order.price,
        };

        let send_order_resp = self.client
            .send_order(Request::new(crate::types::proto::SendOrderRequest {
                subscriber_id,
                order: Some(proto_order),
            }))
            .await?;
        let order_id = send_order_resp.into_inner().order_id;
        Ok(order_id)
    }

    async fn delete_order(&mut self, subscriber_id: u64, order_id: u64) -> Result<u64, Box<dyn std::error::Error>> {
        let delete_order_resp = self.client
            .delete_order(Request::new(crate::types::proto::DeleteOrderRequest {
                subscriber_id,
                order_id,
            }))
            .await?;
        let order_id = delete_order_resp.into_inner().order_id;
        Ok(order_id)
    }

    async fn tick(&mut self, subscriber_id: u64) -> Result<(), Box<dyn std::error::Error>> {
        self.client
            .tick(Request::new(crate::types::proto::TickRequest {
                subscriber_id,
            }))
            .await?;
        Ok(())
    }

    async fn fetch_quotes(&mut self) -> Result<Vec<crate::types::Quote>, Box<dyn std::error::Error>> {
        let quotes_resp = self.client
            .fetch_quotes(Request::new(crate::types::proto::FetchQuotesRequest {}))
            .await?;
        let quotes: Vec<crate::types::Quote> = quotes_resp.into_inner().quotes.into_iter().map(|v| v.into()).collect();
        Ok(quotes)
    }

    async fn fetch_trades(&mut self) -> Result<Vec<crate::types::ExchangeTrade>, Box<dyn std::error::Error>> {
        let trades_resp = self.client
            .fetch_trades(Request::new(crate::types::proto::FetchTradesRequest {}))
            .await?;
        let trades: Vec<crate::types::ExchangeTrade> = trades_resp.into_inner().trades.into_iter().map(|v| v.into()).collect();
        Ok(trades)
    }
}