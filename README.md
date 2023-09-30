# What is Alator?

Components for backtesting a financial portfolio. Built with Rust.

# Development priorities

* Improving performance/test coverage/documentation
* Adding integration with other languages, currently missing JS/WASM and implementation of a trading strategy in Python that can run in Rust.
* Making the code simpler, when type underlying `PriceSource` was made generic that added a lot of weight to component definitions.

# Change Log

v0.3.0 - Added concurrent strategies. Library still supports running synchronously but multiple strategies can now be updated at the same time significantly improving performance. Perf in inner trade loop was also improved 25% and 5% in a full backtest. Docs haven't been updated as a further architectural change needs to be pushed but tests cover how to run sync/async backtests. 

v0.2.11 - Added (hopefully) zero-copy Python integration. No other way to do this but making everything generic on `Quotable`/`Dividendable` traits which has led to substantial changes in the signature of Broker/Strategy/Exchange implementations. This also comes with perf improvements for the `DataSource` trait and general improvements throughout the library.

v0.2.10 - Added licence/CI. Perf improvements. Bug fix in CAGR calculation and clarifying comments. Updated deps.

v0.2.8 - Bugfixes for perf reporting. Perf improvements.

v0.2.4 - Added more info to `BacktestOutput`. `CashValue` implements `Add`. Fix `DataSource` return values. Added inflation variable to `StrategySnapshot`, breaking change.

v0.2.3 - Added `DateTime` parser from date string.

v0.2.1 - Performance now runs against `SimContext` due to issue with borrow check on `Strategy` in full simulation.

v0.2.0 - `Exchange` added onto `Broker` struct. Significant changes to core data structures to improve readability. More documentation. Simplification of performance calculations.

v0.1.7 - `Series` offers a set of stateless calcluations over values (f64, because we need to consistently support log). `PortfolioCalculations` now separate within perf, also stateless but includes those operations that relate to an underlying portfolio. Some portfolio calculations were incorrect when the underlying portfolio had significant numbers of cash transactions, this has been fixed as `PortfolioCalculations` now calculate returns after cashflows.

v0.1.6 - When broker attempts to query the value of a security without price and fails to find a quote for the current time, it will use the cached last seen bid instead of returning no value. This has changed the mutability guarantees of certain operations - for example, get_position_value - that sound like they should be immutable. Added tests to verify working functionality.

v0.1.5 - Backtest won't panic when the client generates target weights containing a security without a quote for the current date. Assumes that the client knows that the security will be tradable at some point, just not today (for example, weekends). If the client has created a target weight for a non-existent security, no error will be thrown. This reversion is required to account for non-trading days/bank holidays/etc. without requiring an ex-ante calculation of these dates. Added test cases for this scenario, added test to panic when diffing against a zero-value broker, added test to verify the behaviour of ClockBuilder when creating a fixed date Clock.

v0.1.4 - Added Clone to `PerfStruct`.

v0.1.3 - Clock creation methods are more descriptive, and define more clearly what kind of frequency and time scale the clock should run at.

v0.1.2 - Transfer traits in strategy now return events to caller that communicate the result of the operation.

v0.1.1 - Added test case showing how to create data source. Changed behaviour of `PositionInfo`.`get_portfolio_qty` so that ambiguous zero values aren't shown to client. 
