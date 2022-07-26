use itertools::Itertools;
use std::collections::HashMap;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

///Defines a set of base types that are used by multiple components.

#[derive(Clone, Copy, Debug, PartialOrd, PartialEq)]
pub struct CashValue(f64);

impl CashValue {
    pub const MAX: f64 = f64::MAX;

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

impl Mul<f64> for CashValue {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self {
        Self(self.0 * rhs)
    }
}

impl Div for CashValue {
    type Output = Self;

    fn div(self, rhs: Self) -> Self {
        Self(self.0 / rhs.0)
    }
}

impl Div<f64> for CashValue {
    type Output = Self;

    fn div(self, rhs: f64) -> Self {
        Self(self.0 / rhs)
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

impl Add<f64> for CashValue {
    type Output = Self;

    fn add(self, rhs: f64) -> Self {
        Self(self.0 + rhs)
    }
}

impl Sub for CashValue {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl Sub<f64> for CashValue {
    type Output = Self;

    fn sub(self, rhs: f64) -> Self {
        Self(self.0 - rhs)
    }
}

impl AddAssign for CashValue {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl AddAssign<f64> for CashValue {
    fn add_assign(&mut self, rhs: f64) {
        self.0 += rhs
    }
}

impl SubAssign for CashValue {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0
    }
}

impl SubAssign<f64> for CashValue {
    fn sub_assign(&mut self, rhs: f64) {
        self.0 -= rhs
    }
}

impl MulAssign for CashValue {
    fn mul_assign(&mut self, rhs: Self) {
        self.0 *= rhs.0
    }
}

impl MulAssign<f64> for CashValue {
    fn mul_assign(&mut self, rhs: f64) {
        self.0 *= rhs
    }
}

impl DivAssign for CashValue {
    fn div_assign(&mut self, rhs: Self) {
        self.0 /= rhs.0
    }
}

impl DivAssign<f64> for CashValue {
    fn div_assign(&mut self, rhs: f64) {
        self.0 /= rhs
    }
}

impl Sum for CashValue {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut res = CashValue::default();
        for v in iter {
            res += v.0
        }
        res
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

impl Add for DateTime {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Add<i64> for DateTime {
    type Output = Self;

    fn add(self, rhs: i64) -> Self {
        Self(self.0 + rhs)
    }
}

impl AddAssign for DateTime {
    fn add_assign(&mut self, rhs: DateTime) {
        self.0 += rhs.0
    }
}

impl AddAssign<i64> for DateTime {
    fn add_assign(&mut self, rhs: i64) {
        self.0 += rhs
    }
}

impl Sub for DateTime {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl SubAssign for DateTime {
    fn sub_assign(&mut self, rhs: DateTime) {
        self.0 -= rhs.0
    }
}

impl Sub<i64> for DateTime {
    type Output = Self;

    fn sub(self, rhs: i64) -> Self {
        Self(self.0 - rhs)
    }
}

impl SubAssign<i64> for DateTime {
    fn sub_assign(&mut self, rhs: i64) {
        self.0 -= rhs
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

//Represents the current state of a portfolio in terms of the number of shares held
//TODO: this is fairly unclear, we also have values which should be computable from holdings so at
//least one of these structures is not needed.
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

//Represents the current state of a portfolio in terms of the value of each position
#[derive(Clone, Debug)]
pub struct PortfolioValues(pub HashMap<String, CashValue>);

impl PortfolioValues {
    pub fn insert(&mut self, ticker: &str, value: &CashValue) {
        self.0.insert(ticker.to_string(), *value);
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

//Represents the state of the portfolio in terms of the percentage of total value assigned to each
//ticker
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
