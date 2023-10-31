[Docs](https://docs.rs/alator)

# What is Alator?

Components for backtesting a financial portfolio. Built with Rust.

One of the initial priorities with alator was high-performance: it was possible to run a backtest across tens of thousands of time units in milliseconds. Over time, this single priority changed as I became more interested in writing a more general purpose backtesting library (initally this library was just a backend for another application). The first stage of this transition was implementing multithreaded code and separating the exchange from other components. The second stage is adding a gRPC implementation of an exchange which should allow easier cross-language backtests.

Implementing the gRPC exchange has required building out a second crate which has meant significant changes to the file structure. The gRPC exchange will be integrated into/reach parity with the main library over time, and implementation code in the main library will be simplified significantly.

# Development priorities

* Improving performance/test coverage/documentation
* Adding integration with other languages, currently missing JS/WASM and implementation of a trading strategy in Python that can run in Rust.
* Making the code simpler, when type underlying `PriceSource` was made generic that added a lot of weight to component definitions.