mod builder;

pub use builder::{AsyncStaticWeightStrategyBuilder, StaticWeightStrategyBuilder};

use async_trait::async_trait;
use log::info;

use crate::broker::{
    BacktestBroker, BrokerCalculations, BrokerCashEvent, ConcurrentBroker, DividendPayment,
    EventLog, ReceievesOrders, ReceievesOrdersAsync, SingleBroker, Trade, TransferCash,
};
use crate::clock::Clock;
use crate::input::{DataSource, Dividendable, Quotable};
use crate::schedule::{DefaultTradingSchedule, TradingSchedule};
use crate::strategy::StrategyEvent;
use crate::types::{CashValue, PortfolioAllocation, StrategySnapshot};

use super::{AsyncStrategy, AsyncTransferFrom, Audit, History, Strategy, TransferFrom, TransferTo};

///Basic implementation of an investment strategy which takes a set of fixed-weight allocations and
///rebalances over time towards those weights.
pub struct AsyncStaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    brkr: ConcurrentBroker<T, Q, D>,
    target_weights: PortfolioAllocation,
    net_cash_flow: CashValue,
    clock: Clock,
    history: Vec<StrategySnapshot>,
}

unsafe impl<T, Q, D> Send for AsyncStaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
}

impl<T, Q, D> AsyncStaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    pub fn get_snapshot(&mut self) -> StrategySnapshot {
        // Defaults to zero inflation because most users probably aren't looking
        // for real returns calcs
        let now = self.clock.now();
        StrategySnapshot {
            date: now,
            portfolio_value: self.brkr.get_total_value(),
            net_cash_flow: self.net_cash_flow.clone(),
            inflation: 0.0,
        }
    }
}

#[async_trait]
impl<T, Q, D> AsyncStrategy for AsyncStaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    async fn init(&mut self, initital_cash: &f64) {
        self.deposit_cash(initital_cash);
        if DefaultTradingSchedule::should_trade(&self.clock.now()) {
            let orders = BrokerCalculations::diff_brkr_against_target_weights(
                &self.target_weights,
                &mut self.brkr,
            );
            if !orders.is_empty() {
                self.brkr.send_orders(&orders).await;
            }
        }
    }

    async fn update(&mut self) {
        self.brkr.check().await;
        let now = self.clock.now();
        if DefaultTradingSchedule::should_trade(&now) {
            let orders = BrokerCalculations::diff_brkr_against_target_weights(
                &self.target_weights,
                &mut self.brkr,
            );
            if !orders.is_empty() {
                self.brkr.send_orders(&orders).await;
            }
        }
        let snap = self.get_snapshot();
        self.history.push(snap);
    }
}

impl<T, Q, D> TransferTo for AsyncStaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn deposit_cash(&mut self, cash: &f64) -> StrategyEvent {
        info!("STRATEGY: Depositing {:?} into strategy", cash);
        self.brkr.deposit_cash(cash);
        self.net_cash_flow = CashValue::from(cash + *self.net_cash_flow);
        StrategyEvent::DepositSuccess(CashValue::from(*cash))
    }
}

#[async_trait]
impl<T, Q, D> AsyncTransferFrom for AsyncStaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn withdraw_cash(&mut self, cash: &f64) -> StrategyEvent {
        if let BrokerCashEvent::WithdrawSuccess(withdrawn) = self.brkr.withdraw_cash(cash) {
            info!("STRATEGY: Succesfully withdrew {:?} from strategy", cash);
            self.net_cash_flow = CashValue::from(*self.net_cash_flow - *withdrawn);
            return StrategyEvent::WithdrawSuccess(CashValue::from(*cash));
        }
        info!("STRATEGY: Failed to withdraw {:?} from strategy", cash);
        StrategyEvent::WithdrawFailure(CashValue::from(*cash))
    }

    async fn withdraw_cash_with_liquidation(&mut self, cash: &f64) -> StrategyEvent {
        if let BrokerCashEvent::WithdrawSuccess(withdrawn) =
            //No logging here because the implementation is fully logged due to the greater
            //complexity of this task vs standard withdraw
            BrokerCalculations::withdraw_cash_with_liquidation_async(cash, &mut self.brkr)
                    .await
        {
            self.net_cash_flow = CashValue::from(*self.net_cash_flow - *withdrawn);
            return StrategyEvent::WithdrawSuccess(CashValue::from(*cash));
        }
        StrategyEvent::WithdrawFailure(CashValue::from(*cash))
    }
}

impl<T, Q, D> Audit for AsyncStaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn trades_between(&self, start: &i64, end: &i64) -> Vec<Trade> {
        self.brkr.trades_between(start, end)
    }

    fn dividends_between(&self, start: &i64, end: &i64) -> Vec<DividendPayment> {
        self.brkr.dividends_between(start, end)
    }
}

impl<T, Q, D> History for AsyncStaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn get_history(&self) -> Vec<StrategySnapshot> {
        self.history.clone()
    }
}

///Basic implementation of an investment strategy which takes a set of fixed-weight allocations and
///rebalances over time towards those weights.
pub struct StaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    brkr: SingleBroker<T, Q, D>,
    target_weights: PortfolioAllocation,
    net_cash_flow: CashValue,
    clock: Clock,
    history: Vec<StrategySnapshot>,
}

impl<T, Q, D> StaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    pub fn get_snapshot(&mut self) -> StrategySnapshot {
        // Defaults to zero inflation because most users probably aren't looking
        // for real returns calcs
        let now = self.clock.now();
        StrategySnapshot {
            date: now,
            portfolio_value: self.brkr.get_total_value(),
            net_cash_flow: self.net_cash_flow.clone(),
            inflation: 0.0,
        }
    }
}

impl<T, Q, D> Strategy for StaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn init(&mut self, initital_cash: &f64) {
        self.deposit_cash(initital_cash);
        if DefaultTradingSchedule::should_trade(&self.clock.now()) {
            let orders = BrokerCalculations::diff_brkr_against_target_weights(
                &self.target_weights,
                &mut self.brkr,
            );
            if !orders.is_empty() {
                self.brkr.send_orders(&orders);
            }
        }
    }

    fn update(&mut self) {
        self.brkr.check();
        let now = self.clock.now();
        if DefaultTradingSchedule::should_trade(&now) {
            let orders = BrokerCalculations::diff_brkr_against_target_weights(
                &self.target_weights,
                &mut self.brkr,
            );
            if !orders.is_empty() {
                self.brkr.send_orders(&orders);
            }
        }
        let snap = self.get_snapshot();
        self.history.push(snap);
    }
}

impl<T, Q, D> TransferTo for StaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn deposit_cash(&mut self, cash: &f64) -> StrategyEvent {
        info!("STRATEGY: Depositing {:?} into strategy", cash);
        self.brkr.deposit_cash(cash);
        self.net_cash_flow = CashValue::from(cash + *self.net_cash_flow);
        StrategyEvent::DepositSuccess(CashValue::from(*cash))
    }
}

impl<T, Q, D> TransferFrom for StaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn withdraw_cash(&mut self, cash: &f64) -> StrategyEvent {
        if let BrokerCashEvent::WithdrawSuccess(withdrawn) = self.brkr.withdraw_cash(cash) {
            info!("STRATEGY: Succesfully withdrew {:?} from strategy", cash);
            self.net_cash_flow = CashValue::from(*self.net_cash_flow - *withdrawn);
            return StrategyEvent::WithdrawSuccess(CashValue::from(*cash));
        }
        info!("STRATEGY: Failed to withdraw {:?} from strategy", cash);
        StrategyEvent::WithdrawFailure(CashValue::from(*cash))
    }

    fn withdraw_cash_with_liquidation(&mut self, cash: &f64) -> StrategyEvent {
        if let BrokerCashEvent::WithdrawSuccess(withdrawn) =
            //No logging here because the implementation is fully logged due to the greater
            //complexity of this task vs standard withdraw
            BrokerCalculations::withdraw_cash_with_liquidation(cash, &mut self.brkr)
        {
            self.net_cash_flow = CashValue::from(*self.net_cash_flow - *withdrawn);
            return StrategyEvent::WithdrawSuccess(CashValue::from(*cash));
        }
        StrategyEvent::WithdrawFailure(CashValue::from(*cash))
    }
}

impl<T, Q, D> Audit for StaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn trades_between(&self, start: &i64, end: &i64) -> Vec<Trade> {
        self.brkr.trades_between(start, end)
    }

    fn dividends_between(&self, start: &i64, end: &i64) -> Vec<DividendPayment> {
        self.brkr.dividends_between(start, end)
    }
}

impl<T, Q, D> History for StaticWeightStrategy<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn get_history(&self) -> Vec<StrategySnapshot> {
        self.history.clone()
    }
}
