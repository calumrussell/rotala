//! Generic types used across package

use itertools::Itertools;
use std::ops::Deref;
use std::{collections::HashMap, ops::Add};

use rotala::clock::DateTime;

///Defines a set of base types that are used by multiple components.

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct CashValue(f64);

impl Deref for CashValue {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for CashValue {
    fn default() -> Self {
        Self(0.0)
    }
}

impl From<CashValue> for f64 {
    fn from(v: CashValue) -> Self {
        v.0
    }
}

impl From<f64> for CashValue {
    fn from(v: f64) -> Self {
        CashValue(v)
    }
}

impl Add<CashValue> for CashValue {
    type Output = CashValue;

    fn add(self, rhs: CashValue) -> Self::Output {
        CashValue::from(*self + *rhs)
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct PortfolioQty(f64);

impl Deref for PortfolioQty {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<f64> for PortfolioQty {
    fn from(v: f64) -> Self {
        Self(v)
    }
}

impl From<PortfolioQty> for f64 {
    fn from(v: PortfolioQty) -> Self {
        *v
    }
}

impl Default for PortfolioQty {
    fn default() -> Self {
        Self(0.0)
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct Price(f64);

impl Deref for Price {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for Price {
    fn default() -> Self {
        Self(0.0)
    }
}

impl From<Price> for f64 {
    fn from(v: Price) -> Self {
        v.0
    }
}

impl From<f64> for Price {
    fn from(v: f64) -> Self {
        Price(v)
    }
}

///Portfolio state in terms of the qty held (for example, shares) for each position. Postions are
///represented by the string name/ticker.
//TODO: this is fairly unclear, we also have values which should be computable from holdings so at
//least one of these structures is not needed.
#[derive(Clone, Debug)]
pub struct PortfolioHoldings(pub HashMap<String, PortfolioQty>);

impl PortfolioHoldings {
    pub fn get(&self, ticker: &str) -> Option<PortfolioQty> {
        self.0.get(ticker).cloned()
    }

    pub fn remove(&mut self, ticker: &str) {
        self.0.remove(ticker);
    }

    pub fn keys(&self) -> Vec<String> {
        self.0.keys().cloned().collect_vec()
    }

    pub fn insert(&mut self, ticker: &str, value: &PortfolioQty) {
        self.0.insert(ticker.to_string(), value.clone());
    }

    pub fn new() -> Self {
        let map: HashMap<String, PortfolioQty> = HashMap::new();
        Self(map)
    }
}

impl Default for PortfolioHoldings {
    fn default() -> Self {
        Self::new()
    }
}

///Portfolio state in terms of cash allocation to each position. Position is represented by string
///name/ticker.
#[derive(Clone, Debug)]
pub struct PortfolioValues(pub HashMap<String, CashValue>);

impl PortfolioValues {
    pub fn insert(&mut self, ticker: &str, value: &CashValue) {
        self.0.insert(ticker.to_string(), value.clone());
    }

    pub fn new() -> Self {
        let map: HashMap<String, CashValue> = HashMap::new();
        Self(map)
    }
}

impl Default for PortfolioValues {
    fn default() -> Self {
        Self::new()
    }
}

///Size of a position in a portfolio in percentage terms.
#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct PortfolioWeight(f64);

impl Deref for PortfolioWeight {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<PortfolioWeight> for f64 {
    fn from(v: PortfolioWeight) -> Self {
        v.0
    }
}

impl From<f64> for PortfolioWeight {
    fn from(v: f64) -> Self {
        PortfolioWeight(v)
    }
}

///Portfolio state in terms of percentage weight allocated to a stock represented by string name.
#[derive(Clone, Debug)]
//Previous version of this type was generic, saw no need for this because there are no cases where
//we need an allocation over some generic weighting. We are using a plain wrapper around HashMap
//because there may come a point when we need to add specific functionality.
pub struct PortfolioAllocation(HashMap<String, PortfolioWeight>);

impl PortfolioAllocation {
    pub fn get(&self, ticker: impl AsRef<str>) -> Option<&PortfolioWeight> {
        self.0.get(ticker.as_ref())
    }

    pub fn insert(&mut self, ticker: impl AsRef<str>, value: impl Into<PortfolioWeight>) {
        self.0.insert(ticker.as_ref().to_string(), value.into());
    }

    pub fn keys(&self) -> Vec<String> {
        self.0.keys().cloned().collect_vec()
    }

    pub fn new() -> Self {
        let map: HashMap<String, PortfolioWeight> = HashMap::new();
        Self(map)
    }
}

impl Default for PortfolioAllocation {
    fn default() -> Self {
        Self::new()
    }
}

/// A point=in-time representation of the current state of a strategy. These statistics are currently
/// recorded for use within performance calculations after the simulation has concluded. They are
/// distinct from the transaction logging performed by brokers.
///
/// Inflation is calculated over the snapshot period. No manipulation of the value is conducted to
/// change the frequency.
///
/// net_cash_flow variable is a sum, not a measure of flow within the period. To get flows, we have
/// to diff each value with the previous one.
#[derive(Clone, Debug)]
pub struct StrategySnapshot {
    pub date: DateTime,
    pub portfolio_value: CashValue,
    pub net_cash_flow: CashValue,
    pub inflation: f64,
}

impl StrategySnapshot {
    pub fn nominal(date: DateTime, portfolio_value: CashValue, net_cash_flow: CashValue) -> Self {
        Self {
            date,
            portfolio_value,
            net_cash_flow,
            inflation: 0.0,
        }
    }

    pub fn real(
        date: DateTime,
        portfolio_value: CashValue,
        net_cash_flow: CashValue,
        inflation: f64,
    ) -> Self {
        Self {
            date,
            portfolio_value,
            net_cash_flow,
            inflation,
        }
    }
}
