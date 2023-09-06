use std::collections::HashMap;
use std::marker::PhantomData;

use crate::broker::{BrokerCost, BrokerLog};
use crate::exchange::SingleExchange;
use crate::input::{Dividendable, Quotable, CorporateEventsSource, PriceSource};
use crate::types::{CashValue, PortfolioHoldings};

use super::SingleBroker;

pub struct SingleBrokerBuilder<D, T, Q, P>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
    P: PriceSource<Q>,
{
    //Cannot run without data but can run with empty trade_costs
    corporate_source: Option<T>,
    trade_costs: Vec<BrokerCost>,
    exchange: Option<SingleExchange<Q, P>>,
    dividend: PhantomData<D>,
}

impl <D, T, Q, P> SingleBrokerBuilder<D, T, Q, P>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
    P: PriceSource<Q>,
{
    pub fn build(&mut self) -> SingleBroker<D, T, Q, P> {
        if self.exchange.is_none() {
            panic!("Cannot build broker without exchange");
        }

        //If we don't have quotes on first tick, we shouldn't error but we should expect every
        //`DataSource` to provide a first tick
        let mut first_quotes = HashMap::new();
        let quotes = self.exchange.as_ref().unwrap().fetch_quotes();
        for quote in &quotes {
            first_quotes.insert(quote.get_symbol().to_string(), std::sync::Arc::clone(quote));
        }

        let holdings = PortfolioHoldings::new();
        let log = BrokerLog::new();

        let exchange = std::mem::take(&mut self.exchange).unwrap();

        let corporate_source = std::mem::take(&mut self.corporate_source);

        SingleBroker {
            corporate_source,
            //Intialised as invalid so errors throw if client tries to run before init
            holdings,
            cash: CashValue::from(0.0),
            log,
            last_seen_trade: 0,
            exchange,
            trade_costs: self.trade_costs.clone(),
            latest_quotes: first_quotes,
            dividend: PhantomData,
        }
    }

    pub fn with_corporate_source(&mut self, data: T) -> &mut Self {
        self.corporate_source = Some(data);
        self
    }

    pub fn with_exchange(&mut self, exchange: SingleExchange<Q, P>) -> &mut Self {
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
            dividend: PhantomData,
        }
    }
}

impl<D, T, Q, P> Default for SingleBrokerBuilder<D, T, Q, P>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
    P: PriceSource<Q>,
{
    fn default() -> Self {
        Self::new()
    }
}
 