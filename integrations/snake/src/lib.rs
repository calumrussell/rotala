use alator::exchange::SingleExchangeBuilder;
use pyo3::prelude::*;

use alator::clock::ClockBuilder;
use alator::input::PyInput;
use alator::strategy::StaticWeightStrategyBuilder;
use alator::broker::{BrokerCost, PyQuote, PyDividend, SingleBrokerBuilder};
use alator::simcontext::SimContextBuilder;
use alator::types::{CashValue, Frequency, PortfolioAllocation};
use pyo3::types::PyDict;

#[pyfunction]
fn staticweight_example(quotes_any: &PyAny, dividends_any: &PyAny, tickers_any: &PyAny) -> PyResult<String> {

    let clock = ClockBuilder::with_length_in_seconds(1, 100_000)
        .with_frequency(&Frequency::Second)
        .build();

    let quotes: &PyDict = quotes_any.downcast()?;
    let dividends: &PyDict = dividends_any.downcast()?;
    let tickers: &PyDict = tickers_any.downcast()?;

    let input = PyInput {
        quotes,
        dividends,
        tickers,
        clock: clock.clone(),
    };

    let initial_cash: CashValue = 100_000.0.into();

    let mut weights: PortfolioAllocation = PortfolioAllocation::new();
    weights.insert("ABC", 0.5);
    weights.insert("BCD", 0.5);

    let exchange = SingleExchangeBuilder::<PyInput, PyQuote, PyDividend>::new()
        .with_data_source(input.clone())
        .with_clock(clock.clone())
        .build();

    let simbrkr = SingleBrokerBuilder::new()
        .with_data(input)
        .with_exchange(exchange)
        .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
        .build();

    let strat = StaticWeightStrategyBuilder::new()
        .with_brkr(simbrkr)
        .with_weights(weights)
        .with_clock(clock.clone())
        .default();

    let mut sim = SimContextBuilder::new()
        .with_clock(clock.clone())
        .with_strategy(strat)
        .init(&initial_cash);

    sim.run();

    let perf = sim.perf(Frequency::Daily);
    Ok(perf.cagr.to_string())
}

#[pymodule]
fn snake(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(staticweight_example, m)?)?;
    m.add_class::<PyQuote>()?;
    m.add_class::<PyDividend>()?;
    Ok(())
}
