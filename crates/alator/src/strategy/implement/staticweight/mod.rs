mod builder;

pub use builder::{AsyncStaticWeightStrategyBuilder, StaticWeightStrategyBuilder};

use async_trait::async_trait;
use log::info;

use crate::broker::implement::multi::ConcurrentBroker;
use crate::broker::implement::single::SingleBroker;
use crate::broker::{
    BacktestBroker, BrokerCalculations, BrokerCashEvent, DividendPayment, EventLog, ReceivesOrders,
    ReceivesOrdersAsync, Trade,
};
use crate::clock::Clock;
use crate::input::{CorporateEventsSource, Dividendable, PriceSource, Quotable};
use crate::schedule::{DefaultTradingSchedule, TradingSchedule};
use crate::strategy::{
    AsyncStrategy, AsyncTransferFrom, Audit, History, Strategy, StrategyEvent, TransferFrom,
    TransferTo,
};
use crate::types::{CashValue, PortfolioAllocation, StrategySnapshot};

/// Fixed-weight allocations over the full simulation.
///
/// Broker accepts orders but the portfolio is modelled as target percentages.
pub struct AsyncStaticWeightStrategy<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    brkr: ConcurrentBroker<D, T, Q>,
    target_weights: PortfolioAllocation,
    net_cash_flow: CashValue,
    clock: Clock,
    history: Vec<StrategySnapshot>,
}

unsafe impl<D, T, Q> Send for AsyncStaticWeightStrategy<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
}

impl<D, T, Q> AsyncStaticWeightStrategy<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
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
impl<D, T, Q> AsyncStrategy for AsyncStaticWeightStrategy<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
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

impl<D, T, Q> TransferTo for AsyncStaticWeightStrategy<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    fn deposit_cash(&mut self, cash: &f64) -> StrategyEvent {
        info!("STRATEGY: Depositing {:?} into strategy", cash);
        self.brkr.deposit_cash(cash);
        self.net_cash_flow = CashValue::from(cash + *self.net_cash_flow);
        StrategyEvent::DepositSuccess(CashValue::from(*cash))
    }
}

#[async_trait]
impl<D, T, Q> AsyncTransferFrom for AsyncStaticWeightStrategy<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
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

impl<D, T, Q> Audit for AsyncStaticWeightStrategy<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    fn trades_between(&self, start: &i64, end: &i64) -> Vec<Trade> {
        self.brkr.trades_between(start, end)
    }

    fn dividends_between(&self, start: &i64, end: &i64) -> Vec<DividendPayment> {
        self.brkr.dividends_between(start, end)
    }
}

impl<D, T, Q> History for AsyncStaticWeightStrategy<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    fn get_history(&self) -> Vec<StrategySnapshot> {
        self.history.clone()
    }
}

///Basic implementation of an investment strategy which takes a set of fixed-weight allocations and
///rebalances over time towards those weights.
pub struct StaticWeightStrategy<D, T, Q, P>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
    P: PriceSource<Q>,
{
    brkr: SingleBroker<D, T, Q, P>,
    target_weights: PortfolioAllocation,
    net_cash_flow: CashValue,
    clock: Clock,
    history: Vec<StrategySnapshot>,
}

impl<D, T, Q, P> StaticWeightStrategy<D, T, Q, P>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
    P: PriceSource<Q>,
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

impl<D, T, Q, P> Strategy for StaticWeightStrategy<D, T, Q, P>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
    P: PriceSource<Q>,
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

impl<D, T, Q, P> TransferTo for StaticWeightStrategy<D, T, Q, P>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
    P: PriceSource<Q>,
{
    fn deposit_cash(&mut self, cash: &f64) -> StrategyEvent {
        info!("STRATEGY: Depositing {:?} into strategy", cash);
        self.brkr.deposit_cash(cash);
        self.net_cash_flow = CashValue::from(cash + *self.net_cash_flow);
        StrategyEvent::DepositSuccess(CashValue::from(*cash))
    }
}

impl<D, T, Q, P> TransferFrom for StaticWeightStrategy<D, T, Q, P>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
    P: PriceSource<Q>,
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

impl<D, T, Q, P> Audit for StaticWeightStrategy<D, T, Q, P>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
    P: PriceSource<Q>,
{
    fn trades_between(&self, start: &i64, end: &i64) -> Vec<Trade> {
        self.brkr.trades_between(start, end)
    }

    fn dividends_between(&self, start: &i64, end: &i64) -> Vec<DividendPayment> {
        self.brkr.dividends_between(start, end)
    }
}

impl<D, T, Q, P> History for StaticWeightStrategy<D, T, Q, P>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
    P: PriceSource<Q>,
{
    fn get_history(&self) -> Vec<StrategySnapshot> {
        self.history.clone()
    }
}
