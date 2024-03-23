//! Strategy is responsible for generating new orders. All execution logic should be left to the
//! broker.
//!
//! Orchestration of the backtest should occur primarly within strategy code in `update`. This can
//! be thought as the "top-level" of the application and the timing of the broker should be
//! controlled from strategy.
//!
//! When running over a network it is possible for multiple strategies to be running concurrently.
//! The available exchange implementations currently available leave all that orchestration on
//! clients but future exchange implementations will have some protection for environments with
//! multiple strategies running concurrently.

pub mod staticweight;

use crate::broker::BrokerTrade;
#[allow(unused)]
use crate::types::{CashValue, StrategySnapshot};

/// `init` should set up broker with initial_cash and initialize any data sources used
/// by strategy. `update` is called on every loop and handles the creation of new orders
/// passed to the broker.
pub trait Strategy: TransferTo {
    fn update(&mut self);
    fn init(&mut self, initial_cash: &f64);
}

/// Used to log cash flows which may be used in performance calculations.
pub enum StrategyEvent {
    WithdrawSuccess(CashValue),
    WithdrawFailure(CashValue),
    DepositSuccess(CashValue),
}

/// Used to record trades for external functions (i.e. tax reporting).
pub trait Audit<T: BrokerTrade> {
    fn trades_between(&self, start: &i64, end: &i64) -> Vec<T>;
}

/// Transfer cash into a strategy at the start or whilst running.
pub trait TransferTo {
    fn deposit_cash(&mut self, cash: &f64) -> StrategyEvent;
}

/// Withdraw cash from a strategy ast the start or whilst running.
pub trait TransferFrom {
    fn withdraw_cash(&mut self, cash: &f64) -> StrategyEvent;
    fn withdraw_cash_with_liquidation(&mut self, cash: &f64) -> StrategyEvent;
}

/// Strategy records and can return history to client.
///
/// Records using [StrategySnapshot].
pub trait History {
    fn get_history(&self) -> Vec<StrategySnapshot>;
}
