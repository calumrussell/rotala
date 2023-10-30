use proto::{
    exchange_client::ExchangeClient, FetchQuotesRequest, FetchTradesRequest, RegisterSourceRequest,
    SendOrderRequest, TickRequest,
};
use std::sync::{atomic::AtomicU64, Mutex};
use tonic::{transport::Channel, Request, Response, Status};

use orderbook::{DefaultPriceSource, ExchangeTrade, OrderBook};

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
            symbol: value.symbol,
        }
    }
}

impl From<orderbook::ExchangeOrder> for proto::Order {
    fn from(value: orderbook::ExchangeOrder) -> Self {
        Self {
            symbol: value.symbol,
            quantity: value.shares,
            price: value.price,
            order_type: value.order_type.into(),
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
impl proto::exchange_server::Exchange for DefaultExchange {
    async fn register_source(
        &self,
        _request: Request<proto::RegisterSourceRequest>,
    ) -> Result<Response<proto::RegisterSourceReply>, Status> {
        let curr = self
            .subscriber_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.subscriber_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

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
                order_type: order.order_type.into(),
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
        _request: Request<proto::FetchTradesRequest>,
    ) -> Result<Response<proto::FetchTradesReply>, Status> {
        let trades = self.trades.lock().unwrap();
        let formatted_trades: Vec<proto::Trade> = trades.iter().map(|v| v.clone().into()).collect();
        Ok(Response::new(proto::FetchTradesReply {
            trades: formatted_trades,
        }))
    }

    async fn fetch_quotes(
        &self,
        _request: Request<proto::FetchQuotesRequest>,
    ) -> Result<Response<proto::FetchQuotesReply>, Status> {
        let now = self.clock.lock().unwrap().now();
        if let Some(quotes) = self.source.get_quotes(&now) {
            let formatted_quotes: Vec<proto::Quote> =
                quotes.iter().map(|v| v.clone().into()).collect();
            return Ok(Response::new(proto::FetchQuotesReply {
                quotes: formatted_quotes,
            }));
        }
        Ok(Response::new(proto::FetchQuotesReply {
            quotes: Vec::new(),
        }))
    }

    async fn tick(
        &self,
        _request: Request<proto::TickRequest>,
    ) -> Result<Response<proto::TickReply>, Status> {
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
        Ok(Response::new(proto::TickReply {}))
    }
}

pub struct DefaultClient {
    subscriber_id: u64,
}

impl DefaultClient {
    pub async fn init(
        client: &mut ExchangeClient<Channel>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let subscriber_id_resp = client
            .register_source(Request::new(RegisterSourceRequest {}))
            .await?;
        let subscriber_id = subscriber_id_resp.into_inner().subscriber_id;
        Ok(Self { subscriber_id })
    }

    pub async fn send_order(
        &self,
        client: &mut ExchangeClient<Channel>,
        order_type: orderbook::OrderType,
        price: Option<f64>,
        quantity: f64,
        symbol: &str,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let order = Some(proto::Order {
            order_type: order_type.into(),
            symbol: symbol.to_string(),
            price,
            quantity,
        });
        let send_order_resp = client
            .send_order(Request::new(SendOrderRequest {
                subscriber_id: self.subscriber_id,
                order,
            }))
            .await?;
        let order_id = send_order_resp.into_inner().order_id;
        Ok(order_id)
    }

    pub async fn tick(
        &self,
        client: &mut ExchangeClient<Channel>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        client
            .tick(Request::new(TickRequest {
                subscriber_id: self.subscriber_id,
            }))
            .await?;
        Ok(())
    }

    pub async fn fetch_quotes(
        &self,
        client: &mut ExchangeClient<Channel>,
    ) -> Result<Vec<proto::Quote>, Box<dyn std::error::Error>> {
        let quotes_resp = client
            .fetch_quotes(Request::new(FetchQuotesRequest {}))
            .await?;
        let quotes = quotes_resp.into_inner().quotes;
        Ok(quotes)
    }

    pub async fn fetch_trades(
        &self,
        client: &mut ExchangeClient<Channel>,
    ) -> Result<Vec<proto::Trade>, Box<dyn std::error::Error>> {
        let trades_resp = client
            .fetch_trades(Request::new(FetchTradesRequest {}))
            .await?;
        let trades = trades_resp.into_inner().trades;
        Ok(trades)
    }
}
