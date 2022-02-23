# What is Alator?

Alator is a Rust application that provides portfolio backtesting and the calculation of limited performance statistics. 

Whilst Alator can be used to test active or high-frequency strategies, the package was designed for low-latency backtesting of passive or low-frequency strategies. Most event-driven backtesting packages are meant to simulate one strategy at a time but if you need to perform many simulations quickly, for example creating Monte-Carlo simulations, then existing packages are typically slow. Vectorized backtesting is faster but sacrifices the ability to backtest dynamic strategies.

# How does Alator work?

Alator is designed to be flexible. All components of a backtest can be changed for user-defined components. The core functions of the package are the logic around simulating trade execution, code organization, and simple performance statistics.

A trading strategy is represented as a `Strategy` which has one `Portfolio` which has in turn one or more references to a `Broker` which can fulfill orders for the Portfolio. The `Strategy` encapsulates the logic and data inherent to a trading strategy, and dispatches orders to the `Portfolio`. The `Portfolio` abstraction defines how a certain set of instructions should be fulfilled and contains state that is potentially shared across multiple `Broker` objects. The `Broker` is purely concerned with order execution logic, for example is there enough cash in the portfolio to complete the order, and tracking the results of orders.

