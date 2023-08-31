use std::collections::HashMap;
use std::marker::PhantomData;

use crate::broker::{BrokerCost, BrokerLog};
use crate::exchange::SingleExchange;
use crate::input::{DataSource, Dividendable, Quotable};
use crate::types::{CashValue, PortfolioHoldings};

use super::SingleBroker;

pub struct SingleBrokerBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    //Cannot run without data but can run with empty trade_costs
    data: Option<T>,
    trade_costs: Vec<BrokerCost>,
    exchange: Option<SingleExchange<T, Q, D>>,
}

impl<T, Q, D> SingleBrokerBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    pub fn build(
        &mut self,
    ) -> SingleBroker<T, Q, D> {
        if self.data.is_none() {
            panic!("Cannot build broker without data");
        }

        if self.exchange.is_none() {
            panic!("Cannot build broker without exchange");
        }

        //If we don't have quotes on first tick, we shouldn't error but we should expect every
        //`DataSource` to provide a first tick
        let mut first_quotes = HashMap::new();
        if let Some(quotes) = self.data.as_ref().unwrap().get_quotes() {
            for quote in &quotes {
                first_quotes.insert(quote.get_symbol().to_string(), std::sync::Arc::clone(quote));
            }
        }

        let holdings = PortfolioHoldings::new();
        let log = BrokerLog::new();

        let exchange = std::mem::take(&mut self.exchange).unwrap();

        SingleBroker {
            //TODO: !!!!!!!
            data: self.data.as_ref().unwrap().clone(),
            //Intialised as invalid so errors throw if client tries to run before init
            holdings,
            cash: CashValue::from(0.0),
            log,
            last_seen_trade: 0,
            exchange,
            trade_costs: self.trade_costs.clone(),
            latest_quotes: first_quotes,
            _dividend: PhantomData,
        }
    }

    pub fn with_data(&mut self, data: T) -> &mut Self {
        self.data = Some(data);
        self
    }

    pub fn with_exchange(&mut self, exchange: SingleExchange<T, Q, D>) -> &mut Self {
        self.exchange = Some(exchange);
        self
    }

    pub fn with_trade_costs(&mut self, trade_costs: Vec<BrokerCost>) -> &mut Self {
        self.trade_costs = trade_costs;
        self
    }

    pub fn new() -> Self {
        SingleBrokerBuilder {
            data: None,
            trade_costs: Vec::new(),
            exchange: None,
        }
    }
}

impl<T, Q, D> Default for SingleBrokerBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn default() -> Self {
        Self::new()
    }
}

