use alator::exchange::implement::single::SingleExchangeBuilder;
use pyo3::prelude::*;

use alator::clock::ClockBuilder;
use alator::input::{PyPriceSource, PyCorporateEventsSource};
use alator::strategy::StaticWeightStrategyBuilder;
use alator::broker::implement::single::{SingleBroker, SingleBrokerBuilder};
use alator::broker::{BrokerCost, PyQuote, PyDividend};
use alator::simcontext::SimContextBuilder;
use alator::types::{CashValue, Frequency, PortfolioAllocation};
use pyo3::types::PyDict;

#[pyfunction]
fn staticweight_example(quotes_any: &PyAny, dividends_any: &PyAny, tickers_any: &PyAny) -> PyResult<String> {

    let clock = ClockBuilder::with_length_in_seconds(1, 100_000)
        .with_frequency(&Frequency::Second)
        .build();

    let quotes: &PyDict = quotes_any.downcast()?;
    let _dividends: &PyDict = dividends_any.downcast()?;
    let tickers: &PyDict = tickers_any.downcast()?;

    let price_source = PyPriceSource {
        quotes,
        tickers,
        clock: clock.clone(),
    };

    let initial_cash: CashValue = 100_000.0.into();

    let mut weights: PortfolioAllocation = PortfolioAllocation::new();
    weights.insert("ABC", 0.5);
    weights.insert("BCD", 0.5);

    let exchange = SingleExchangeBuilder::<PyQuote, PyPriceSource>::new()
        .with_price_source(price_source)
        .with_clock(clock.clone())
        .build();

    let simbrkr: SingleBroker<PyDividend, PyCorporateEventsSource, PyQuote, PyPriceSource> = SingleBrokerBuilder::new()
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
