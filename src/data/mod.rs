use itertools::Itertools;
use std::{
    collections::HashMap,
    ops::{Add, AddAssign, Div, Mul, MulAssign, Sub, SubAssign},
};

use crate::broker::{Dividend, Quote};

/* Abstracts basic data operations for components that use data.
 */
pub trait SimSource {
    fn get_quotes_dates(&self) -> Vec<DateTime>;
    //Added dividends, could make sense to support a range of *Actions* such as Quote or Dividend
    //but doesn't make sense when there is only too (and it wouldn't change the public interface).
    fn get_dividends_by_date(&self, date: &DateTime) -> Option<Vec<Dividend>>;
    fn get_quotes_by_date(&self, date: &DateTime) -> Option<Vec<Quote>>;
    fn get_quote_by_date_symbol(&self, date: &DateTime, symbol: &str) -> Option<Quote>;
    fn has_next(&self) -> bool;
    fn step(&mut self);
}

#[derive(Clone)]
pub struct DataSource {
    quotes: HashMap<DateTime, Vec<Quote>>,
    dividends: HashMap<DateTime, Vec<Dividend>>,
    pos: usize,
    keys: Vec<DateTime>,
}

impl SimSource for DataSource {
    fn get_quotes_dates(&self) -> Vec<DateTime> {
        self.quotes.keys().map(|v| v.to_owned()).collect_vec()
    }

    fn get_quote_by_date_symbol(&self, date: &DateTime, symbol: &str) -> Option<Quote> {
        if let Some(quotes) = self.get_quotes_by_date(date) {
            for quote in &quotes {
                if quote.symbol.eq(symbol) {
                    return Some(quote.clone());
                }
            }
        }
        None
    }

    fn get_quotes_by_date(&self, date: &DateTime) -> Option<Vec<Quote>> {
        if let Some(quotes) = self.quotes.get(date) {
            return Some(quotes.clone());
        }
        None
    }
    fn get_dividends_by_date(&self, date: &DateTime) -> Option<Vec<Dividend>> {
        if let Some(dividends) = self.dividends.get(date) {
            return Some(dividends.clone());
        }
        None
    }

    fn step(&mut self) {
        self.pos += 1;
    }

    fn has_next(&self) -> bool {
        self.pos < self.keys.len()
    }
}

impl DataSource {
    pub fn from_hashmap(
        quotes: HashMap<DateTime, Vec<Quote>>,
        dividends: HashMap<DateTime, Vec<Dividend>>,
    ) -> DataSource {
        let keys = quotes.keys().copied().collect_vec();
        DataSource {
            quotes,
            pos: 0,
            keys,
            dividends,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialOrd, PartialEq)]
pub struct CashValue(f64);

impl CashValue {
    pub fn abs(&self) -> Self {
        if self.0 > 0.0 {
            Self(self.0)
        } else {
            Self(self.0 * -1.0)
        }
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

impl PartialEq<f64> for CashValue {
    fn eq(&self, other: &f64) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<f64> for CashValue {
    fn partial_cmp(&self, other: &f64) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

impl Mul for CashValue {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Self(self.0 * rhs.0)
    }
}

impl Div for CashValue {
    type Output = Self;

    fn div(self, rhs: Self) -> Self {
        Self(self.0 / rhs.0)
    }
}

impl Div<Price> for CashValue {
    type Output = PortfolioQty;

    fn div(self, rhs: Price) -> Self::Output {
        PortfolioQty(self.0 / rhs.0)
    }
}

impl Div<PortfolioQty> for CashValue {
    type Output = Price;

    fn div(self, rhs: PortfolioQty) -> Self::Output {
        Price(self.0 / rhs.0)
    }
}

impl Mul<PortfolioWeight> for CashValue {
    type Output = Self;

    fn mul(self, rhs: PortfolioWeight) -> Self {
        Self(self.0 * rhs.0)
    }
}

impl Add for CashValue {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Sub for CashValue {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl AddAssign for CashValue {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl SubAssign for CashValue {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0
    }
}

impl MulAssign for CashValue {
    fn mul_assign(&mut self, rhs: Self) {
        self.0 *= rhs.0
    }
}

//TODO: add date-related functions, this has been replicated across the code base in client
//projects so there is no need not to add that functionality here
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DateTime(i64);

impl From<DateTime> for i64 {
    fn from(v: DateTime) -> Self {
        v.0
    }
}

impl From<i64> for DateTime {
    fn from(v: i64) -> Self {
        DateTime(v)
    }
}

impl PartialEq<i64> for DateTime {
    fn eq(&self, other: &i64) -> bool {
        self.0 == *other
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct PortfolioQty(f64);

impl PortfolioQty {
    pub fn ceil(&self) -> Self {
        Self(f64::ceil(self.0))
    }

    pub fn floor(&self) -> Self {
        Self(f64::floor(self.0))
    }
}

impl From<f64> for PortfolioQty {
    fn from(v: f64) -> Self {
        Self(v)
    }
}

impl Mul<Price> for PortfolioQty {
    type Output = CashValue;

    fn mul(self, rhs: Price) -> Self::Output {
        CashValue(self.0 * rhs.0)
    }
}

impl Add for PortfolioQty {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Sub for PortfolioQty {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl AddAssign for PortfolioQty {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl SubAssign for PortfolioQty {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0
    }
}

impl PartialEq<f64> for PortfolioQty {
    fn eq(&self, other: &f64) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<f64> for PortfolioQty {
    fn partial_cmp(&self, other: &f64) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

impl Default for PortfolioQty {
    fn default() -> Self {
        Self(0.0)
    }
}

impl Default for &PortfolioQty {
    fn default() -> Self {
        &PortfolioQty(0.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Price(f64);

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

impl PartialEq<f64> for Price {
    fn eq(&self, other: &f64) -> bool {
        self.0 == *other
    }
}

impl AddAssign for Price {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl SubAssign for Price {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0
    }
}

impl PartialOrd<f64> for Price {
    fn partial_cmp(&self, other: &f64) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

impl Mul for Price {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Price(self.0 * rhs.0)
    }
}

impl Mul<PortfolioQty> for Price {
    type Output = CashValue;

    fn mul(self, rhs: PortfolioQty) -> Self::Output {
        CashValue(self.0 * rhs.0)
    }
}

impl Sub for Price {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Price(self.0 - rhs.0)
    }
}

#[derive(Clone, Debug)]
pub struct PortfolioHoldings(pub HashMap<String, PortfolioQty>);

impl PortfolioHoldings {
    pub fn get(&self, ticker: &str) -> Option<&PortfolioQty> {
        self.0.get(ticker)
    }

    pub fn keys(&self) -> Vec<String> {
        self.0.keys().cloned().collect_vec()
    }

    pub fn insert(&mut self, ticker: &str, value: &PortfolioQty) {
        self.0.insert(ticker.to_string(), *value);
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

pub trait Weight: Mul<CashValue, Output = CashValue> + Into<f64> {}

#[derive(Clone, Copy, Debug)]
pub struct PortfolioWeight(f64);

impl Weight for PortfolioWeight {}

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

impl Mul<CashValue> for PortfolioWeight {
    type Output = CashValue;

    fn mul(self, rhs: CashValue) -> Self::Output {
        CashValue(self.0 * rhs.0)
    }
}

#[derive(Clone, Debug)]
pub struct PortfolioAllocation<T: Weight>(pub HashMap<String, T>);

impl PortfolioAllocation<PortfolioWeight> {
    pub fn get(&self, ticker: &str) -> Option<&PortfolioWeight> {
        self.0.get(ticker)
    }

    pub fn insert(&mut self, ticker: &str, value: &PortfolioWeight) {
        self.0.insert(ticker.to_string(), *value);
    }

    pub fn keys(&self) -> Vec<String> {
        self.0.keys().cloned().collect_vec()
    }

    pub fn new() -> Self {
        let map: HashMap<String, PortfolioWeight> = HashMap::new();
        Self(map)
    }
}

impl Default for PortfolioAllocation<PortfolioWeight> {
    fn default() -> Self {
        Self::new()
    }
}
