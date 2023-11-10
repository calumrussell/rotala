//! alator-exchange is an implementation of an alator exchange over gRPC using Protobuf for data
//! serialization.
//!
//! The implementation of multi-threading has added significant overhead to this library. The
//! purpose of this change was to move towards a more standalone backtesting library. gRPC
//! implementation provides a potential direction of travel whereby the inner exchange
//! becomes distinct from other components allowing multi-language backtesting.
//!
//! This adds substantial overhead due to network latency/serialization and some amplification of
//! message-passing/calls between broker and exchange as full co-ordination requires n extra calls
//! where n is the number of brokers (due to the need to register that brokers are ready to tick
//! forward).
//!
//! The inner loop of the gRPC with two brokers runs in 8ms with the inner loop of the single-
//! threaded implementation running in ~8μs. The plan is to maintain the single-threaded
//! implementation but leave open the option for a multi-language backtest (or a distributed one).
//!
//! This is experimental and the interface/names used will change in the future.
//! use std::sync::{atomic::AtomicU64, Mutex};
pub mod types;
pub use crate::types::proto::{
    exchange_client::ExchangeClient, FetchQuotesRequest, FetchTradesRequest, RegisterSourceRequest,
    SendOrderRequest, TickRequest
};
pub use types::{OrderType, ExchangeTrade, ExchangeOrder};

pub mod orderbook;
pub mod rpc;

pub trait ExchangeSync {
    fn fetch_quotes(&self) -> Vec<std::sync::Arc<types::Quote>>;
    fn fetch_trades(&self, from: usize) -> &[types::ExchangeTrade];
    fn insert_order(&mut self, order: types::ExchangeOrder);
    fn delete_order(&mut self, order_i: types::DefaultExchangeOrderId);
    fn clear_orders_by_symbol(&mut self, symbol: String);
    fn check(&mut self) -> Vec<types::ExchangeTrade>;
}

#[tonic::async_trait]
pub trait ExchangeAsync {
    async fn register_source(&mut self) -> Result<u64, Box<dyn std::error::Error>>;
    async fn send_order(&mut self, subscriber_id: u64, order: ExchangeOrder) -> Result<u64, Box<dyn std::error::Error>>;
    async fn delete_order(&mut self, subscriber_id: u64, order_id: u64) -> Result<u64, Box<dyn std::error::Error>>;
    async fn fetch_trades(&mut self) -> Result<Vec<types::ExchangeTrade>, Box<dyn std::error::Error>>;
    async fn fetch_quotes(&mut self) -> Result<Vec<types::Quote>, Box<dyn std::error::Error>>;
    async fn tick(&mut self, subscriber_id: u64) -> Result<(), Box<dyn std::error::Error>>;
}