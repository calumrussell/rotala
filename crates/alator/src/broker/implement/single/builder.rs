use std::collections::HashMap;
use std::marker::PhantomData;

use alator_exchange::{ExchangeSync, SyncExchangeImpl};

use crate::broker::implement::single::SingleBroker;
use crate::broker::{BrokerCost, BrokerLog};
use crate::input::{CorporateEventsSource, DefaultCorporateEventsSource, Dividendable};
use crate::types::{CashValue, PortfolioHoldings};

/// Builds [SingleBroker].
pub struct SingleBrokerBuilder {
    //Cannot run without data but can run with empty trade_costs
    corporate_source: Option<DefaultCorporateEventsSource>,
    trade_costs: Vec<BrokerCost>,
    exchange: Option<SyncExchangeImpl>,
}

impl SingleBrokerBuilder {
    pub fn build(&mut self) -> SingleBroker {
        if self.exchange.is_none() {
            panic!("Cannot build broker without exchange");
        }

        //If we don't have quotes on first tick, we shouldn't error but we should expect every
        //`DataSource` to provide a first tick
        let mut first_quotes = HashMap::new();
        let quotes = self.exchange.as_ref().unwrap().fetch_quotes();
        for quote in &quotes {
            first_quotes.insert(quote.get_symbol().to_string(), quote.clone());
        }

        let holdings = PortfolioHoldings::new();
        let pending_orders = PortfolioHoldings::new();
        let log = BrokerLog::new();

        let exchange = std::mem::take(&mut self.exchange).unwrap();

        let corporate_source = std::mem::take(&mut self.corporate_source);

        SingleBroker {
            corporate_source,
            //Intialised as invalid so errors throw if client tries to run before init
            holdings,
            pending_orders,
            cash: CashValue::from(0.0),
            log,
            last_seen_trade: 0,
            exchange,
            trade_costs: self.trade_costs.clone(),
            latest_quotes: first_quotes,
            broker_state: super::BrokerState::Ready,
        }
    }

    pub fn with_corporate_source(&mut self, data: DefaultCorporateEventsSource) -> &mut Self {
        self.corporate_source = Some(data);
        self
    }

    pub fn with_exchange(&mut self, exchange: SyncExchangeImpl) -> &mut Self {
        self.exchange = Some(exchange);
        self
    }

    pub fn with_trade_costs(&mut self, trade_costs: Vec<BrokerCost>) -> &mut Self {
        self.trade_costs = trade_costs;
        self
    }

    pub fn new() -> Self {
        SingleBrokerBuilder {
            corporate_source: None,
            trade_costs: Vec::new(),
            exchange: None,
        }
    }
}

impl Default for SingleBrokerBuilder {
    fn default() -> Self {
        Self::new()
    }
}
