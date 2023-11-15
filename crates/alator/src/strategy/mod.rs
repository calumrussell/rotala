//! Generates orders

pub mod implement;

use async_trait::async_trait;

#[allow(unused)]
use crate::broker::BrokerEvent;
use crate::broker::{DividendPayment, Trade};
use crate::types::{CashValue, StrategySnapshot};

/// Generate changes for broker to act upon.
///
/// Within multi-threaded context, strategy
#[async_trait]
pub trait AsyncStrategy: TransferTo {
    async fn update(&mut self);
    async fn init(&mut self, initial_cash: &f64);
}

/// Generates changes for broker to act upon.
///
/// Within the single-threaded context, strategy triggers all downstream changes to other
/// components. `update` is called, strategy gathers information, calculates new target
/// portfolio and passes the required orders to broker passing the information down.
///
/// [Strategy] also manages snapshots which are used for performance calculation.
pub trait Strategy: TransferTo {
    fn update(&mut self);
    fn init(&mut self, initial_cash: &f64);
}

/// Logs certain events triggered by client.
///
/// Does not cover internally-generated events, such as order creation, but only events that are
/// triggered by the owning context at some point in the simulation.
///
/// These events are used to lock cash flows and, at this stage, are related to the [TransferTo]
/// and/or [TransferFrom] traits. Mirrors [BrokerEvent].
pub enum StrategyEvent {
    //Mirrors BrokerEvent
    WithdrawSuccess(CashValue),
    WithdrawFailure(CashValue),
    DepositSuccess(CashValue),
}

/// Set of functions for reporting events.
///
/// Used for tax calculations at the moment. Mirrors functions on broker.
pub trait Audit {
    fn trades_between(&self, start: &i64, end: &i64) -> Vec<Trade>;
    fn dividends_between(&self, start: &i64, end: &i64) -> Vec<DividendPayment>;
}

/// Transfer cash into a strategy at the start or whilst running.
pub trait TransferTo {
    fn deposit_cash(&mut self, cash: &f64) -> StrategyEvent;
}

/// Withdraw cash from a strategy at the start or whilst running.
#[async_trait]
pub trait AsyncTransferFrom {
    fn withdraw_cash(&mut self, cash: &f64) -> StrategyEvent;
    async fn withdraw_cash_with_liquidation(&mut self, cash: &f64) -> StrategyEvent;
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
