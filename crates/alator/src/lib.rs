//! # How does Alator work?
//!
//! The development goal is to a provide a simple and flexible backtesting library whilst achieving
//! reasonable performance.
//!
//! A backtest is composed of three components: `Strategy`, `Broker`, and `Exchange`. A strategy
//! tells the broker what trades to execute, the broker is responsible for creating orders, and the
//! exchange orders into completed trades. Alator provides implementations for each of these
//! components, these can be used interchangeably with user-provided components by implementing traits.
//!
//! Before v0.3, alator just provided single-threaded implementations. After v0.3, we began
//! providing components that can run in multiple threads (potentially, determining the appropriate
//! runtime is left to the user, we just introduce the option of using more than one thread).
//! This is not foolproof as a backtest requires correct ordering but does provide easier
//! translation to a production environment where strategy code would expect to be non-blocking.
//!
//! Library implementations of components use tokio as the runtime. This leaves it up to the
//! user to decide what resource bound their strategies may have. For example, strategies will
//! often be I/O bound as they may query other data sources but they may not. It is up to the user
//! to decide how many threads will work best. The implementation of the multi-threaded code still
//! needs to be refined and may change over time.
//!
//! Adding multi-threaded code involved substantial changes to the code as, for reasons explored in
//! the next section, there wasn't full separation of `Broker` and `Exchange`. For the most part,
//! the implementation of multi-threading has not impacted the definition of components but there
//! are exceptions: [ReceivesOrders]/[ReceivesOrdersAsync]. Hopefully, it will be possible to
//! remove these at some point but it is something to be aware of when implementing your own
//! components.
//!
//! ## Execution
//!
//! One of the problems with backtesting libraries is that orders will execute instanteously: an
//! order is submitted to an exchange, the exchange queries a price source and receives the precise
//! time set by the system, and that price is filled in as an executed trade. Trades do not execute
//! instanteously in production environments: your strategy looks at some price source but due to
//! latency that price is very likely untradeable. This lookahead bias is why many strategies that
//! backtest well end up performing poorly out-of-sample.
//!
//! Many backtesting libraries execute trades instantneously, as it can be more complicated to
//! implement in an environment with more than one running thread, but do not explain this to users.
//! After v0.1.7, alator removed instanteous execution. The soonest that orders could execute was
//! the next tick. Multi-threaded library implementations of components offer the same guarantee.
//!
//! Because orders are not executed instaneously it is not advisable to use low-frequency data as
//! orders won't execute until the next tick, which would be a month later with monthly frequencies.
//! Cash reconciliation is not triggered externally but will typically occur at the tick frequency
//! so low-frequency execution will also poorly replicate the underlying as cash reconciliations
//! will skew expected performance (it is possible to run cash reconciliations at a different
//! frequency, but it is easier to perform common modifications to the data i.e. add extra "fake"
//! prices around period end where trades can execute).
//!
//! ## Example
//!
//! An example backtest (with data creation excluded):
//!
//! ```
//!     use alator::broker::uist::UistBrokerBuilder;
//!     use alator::broker::BrokerCost;
//!     use alator::strategy::staticweight::StaticWeightStrategyBuilder;
//!     use alator::simcontext::SimContextBuilder;
//!     use alator::types::{ CashValue, PortfolioAllocation };
//!     use rotala::exchange::uist::random_uist_generator;
//!
//!     let initial_cash: CashValue = 100_000.0.into();
//!     let (uist, clock) = random_uist_generator(1000);
//!     let mut brkr = UistBrokerBuilder::new()
//!         .with_trade_costs(vec![BrokerCost::flat(1.0)])
//!         .with_exchange(uist)
//!         .build();
//!
//!     let mut weights: PortfolioAllocation = PortfolioAllocation::new();
//!     weights.insert("ABC", 0.5);
//!     weights.insert("BCD", 0.5);
//!
//!     let strat = StaticWeightStrategyBuilder::new()
//!         .with_brkr(brkr)
//!         .with_weights(weights)
//!         .with_clock(clock.clone())
//!         .default();
//!
//!     let mut sim = SimContextBuilder::new()
//!         .with_clock(clock.clone())
//!         .with_strategy(strat)
//!         .init(&initial_cash);
//!
//!     sim.run();
//!
//! ```
//! Alator comes with a `StaticWeightStrategy` and clients will typically need to implement the
//! `Strategy` trait to build a new strategy. The `Broker` component should be reusable for most
//! cases but may lack the features for some use-cases, the three biggest being: leverage,
//! multiple-currency support, and shorting. Because of the current uses of the library, the main
//! priority in the near future is to add support for multiple-currencies.
//!
//! ## Data
//!
//! Alator is extremely flexible about the underlying representation of data requiring, in the case
//! of prices, only something that implements [Quotable] which can be used with [PriceSource]. Users
//! can create their own data types but, in most cases, the library implementation [PriceSource[ can
//! be used. Corporate events are structured in a similar way but, currently, dividends are the only
//! corporate event supported.
//!
//! In the tests folder, we have provided an implementation of a simple moving average crossover
//! strategy that pulls data directly from Binance. To see the system running: fail the test with
//! an assert and pass `RUST_LOG=info` to the command.
//!
//! ## Cross-language support
//!
//! Alator backtests can be run from Python and price data transferred into Rust without copying
//! (this hasn't been fully tested but seems to be the case). JS/WASM support will be added and,
//! hopefully, strategies can be written in Python. Multi-threading is not supported within these
//! contexts.
//!
//! # Missing features that you may expect
//!
//! * Leverage
//! * Multi-currency
//! * Shorting

#[allow(unused)]
pub mod broker;
pub mod perf;
pub mod schedule;
pub mod simcontext;
pub mod strategy;
pub mod types;
