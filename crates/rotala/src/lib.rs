//! # What is Rotala?
//!
//! Rotala provides exchange back=ends against which users can run backtests. The standard
//! mechanism for creating and running a backtest is the JSON server but users can also import a
//! lib. The lib is intended to be used primarily for testing and creating examples within Rust.
//! Implementations of front-end code using the Rotala lib are in the Alator library.
//!
//! Rotala is at an early-stage in development. Most of the components used in the article were
//! already in-use in a predecessor application. The separation of responsibilities within this
//! library are still being worked out so the large number of abstractions currently in use may
//! reduce.
//!
//! # Implementation
//!
//! A single exchange implementation is composed of:
//! - An input, [Penelope](crate::input::penelope::Penelope) is an example. The input produces
//! quotes and will define the format of quotes that exchanges wishing to use the source must use.
//! - An orderbook implementation, [Diana](crate::orderbook::diana::Diana) is an example. The
//! orderbook contains the core execution logic and defines the format of orders and trades. This
//! is distinct from an exchange as the an orderbook could be LOB, could use candles, etc. And this
//! varies in a distinct way from the interface presented to clients.
//! - An exchange implementation, [Uist](crate::exchange::uist::Uist) is an example. In terms of
//! code, this ends up being a fairly thin wrapper depending more on the kind of clients than
//! the actual execution logic used by the orderbook. To explain a bit more from above, the
//! exchange is the external interface that provides a set of possible operations to users and does
//! not concern itself too closely with how things are implemented (but it does have to bind to s
//! single orderbook implementation). Uist, for example, has a lot of additional methods concerning
//! orchestration and how clients can match state with exchange.
//! - The server implementation of the exchange returning JSON responses over the exchange impl.
//! - The client implementation of the exchange which provides a Rust API for the server, as much
//! for documenting how clients can call the server.
//!
//! In addition to all this, we have data sources which call some external source and are bound into
//! the exchange: for example, the Uist exchange can be created using a Binance input.
//!
//! The proliferation of abstractions is to offer users the most flexibility but this is going to be
//! subject to change as I learn more about this application.
//!
//! # Uist
//!
//! Interface to Uist is defined in [UistClient](crate::client::uist::UistClient).
//!
//! Uist contains no native synchronization features and provides no guarantees about ordering (even
//! though the exchange is wrapped in a Mutex on the server). Therefore, strategies can run concurrently
//! but it is not advised as Uist does not synchronize calls to `check` itself and does not return
//! client-specific information (i.e. the order issued by strategy X will get mixed with orders for
//! strategy Y).
//!
//! ``
//! cargo run --bin uist_server [ipv4_address] [port]
//! ``
//!
//! # Development priorities
//!
//! Short-term:
//! - Add example code showing a strategy running in Python.
//! - Add more external data sources
//! - Add more binaries with servers loading external data
//! - Add another exchange so separation of concerns is clearer
//!
//! Long-term:
//! - Add orderbook with L2 data, this is going to require L2 sources
pub mod client;
pub mod clock;
pub mod exchange;
pub mod input;
pub mod orderbook;
pub mod server;
pub mod source;
