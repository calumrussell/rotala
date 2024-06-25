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

#[allow(unused)]
use crate::types::{CashValue, StrategySnapshot};

/// Used to log cash flows which may be used in performance calculations.
pub enum StrategyEvent {
    WithdrawSuccess(CashValue),
    WithdrawFailure(CashValue),
    DepositSuccess(CashValue),
}
