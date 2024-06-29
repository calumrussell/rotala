//! # What is Alator?
//!
//! Alator is a financial simulation library that can be used with [Rotala](rotala) to perform
//! backtests of trading strategies.
//!
//! Alator used to be a standalone package but is being transitioned towards an implementation of
//! the frontend of a backtest that uses a [Rotala](rotala) exchange backend. Versions after v0.4
//! implement [Rotala](rotala).
//!
//! A backtest is composed of:
//!     * Strategy - generates new orders
//!     * Broker - within Alator this overlaps with a portfolio containing position calculations
//! (i.e. position/profit), routes new orders to exchange, embeds costs.
//!     * Exchange - executes new orders, may embed further cost and latency.
//!     * Data source - data source used by strategy/exchange to determine new orders/fills.
//!     * Clock - synchronizes times between components.
//!
//! Alator contains code for Strategy and Broker. You do not need to use this code when
//! backtesting a strategy and, as some exchange implementations work across a network, you do not
//! even need to use Rust. The code within this package represents common functionality and, as
//! explained above, was already built so is included here.
//!
//! Current exchange implementations do not contain any logic to determine the correctness of
//! trades. For example, if a client places an order for 100 units @ $1 and gets filled with a cash
//! balance of $50 then this should be blocked by the Broker.
//!
//! # Synchronization
//!
//! State is synchronized between components through the exchange. The exchange holds the clock and
//! should be the only source of price data.
//!
//! In terms of performance, this may seem like a non-optimal choice. In initial versions of the
//! library, when the library was a backend to another simulation application, synchronization
//! was split with each component sharing the clock and price source. By splitting synchronization,
//! the number of messages to the exchange is amplified as, for example, the strategy needs to get
//! an update on recent prices.
//!
//! But by keeping state in one place we can offer good guarantees about the correctness of the
//! backtest - by making it harder to introduce lookahead bias - and by enforcing separation of
//! concerns the front-end becomes simpler/replicates production environment.
//!
//! To repeat, some backtesting libraries have instant trade execution: you create an order at
//! epoch 100 and it gets executed immediately on the same epoch. This isn't possible in Alator
//! and backtests should have no lookahead bias.
//!
//! As a result, it is not advisable to use low-frequency data as orders won't execute until the
//! next available tick (or you can fill next tick).
//!
//! Equally, strategies that constantly trigger liquidations, whilst usually representing a bug in
//! the front-end, will also replicate performance poorly because the Broker implementations in
//! Alator will try to rebalance to regain solvency. As that rebalancing won't occur until the next
//! tick you can end up with persistent shortfalls under some conditions (i.e. trending prices).

#[allow(unused)]
pub mod broker;
pub mod perf;
pub mod schedule;
pub mod strategy;
