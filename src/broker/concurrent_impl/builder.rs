use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::broker::{BrokerCost, BrokerLog, ConcurrentBroker};
use crate::exchange::ConcurrentExchange;
use crate::input::{DataSource, Dividendable, Quotable};
use crate::types::{CashValue, PortfolioHoldings};

pub struct ConcurrentBrokerBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    //Cannot run without data but can run with empty trade_costs
    data: Option<T>,
    trade_costs: Vec<BrokerCost>,
    _quote: PhantomData<Q>,
    _dividend: PhantomData<D>,
}

impl<T, Q, D> ConcurrentBrokerBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    pub async fn build(
        &mut self,
        exchange: &mut ConcurrentExchange<T, Q, D>,
    ) -> ConcurrentBroker<T, Q, D> {
        if self.data.is_none() {
            panic!("Cannot build broker without data");
        }

        let (subscriber_id, mut price_rx, notify_rx, order_tx) = exchange.subscribe().await;

        let mut first_quotes = HashMap::new();
        while let Ok(quotes) = price_rx.try_recv() {
            for quote in &quotes {
                first_quotes.insert(quote.get_symbol().to_string(), Arc::clone(quote));
            }
        }

        let holdings = PortfolioHoldings::new();
        let log = BrokerLog::new();

        ConcurrentBroker {
            data: self.data.as_ref().unwrap().clone(),
            //Intialised as invalid so errors throw if client tries to run before init
            holdings,
            cash: CashValue::from(0.0),
            log,
            trade_costs: self.trade_costs.clone(),
            //Initialized as ready because there is no state to catch up with when we create it
            price_receiver: price_rx,
            order_sender: order_tx,
            notify_receiver: notify_rx,
            exchange_subscriber_id: subscriber_id,
            latest_quotes: first_quotes,
            _dividend: PhantomData,
        }
    }

    pub fn with_data(&mut self, data: T) -> &mut Self {
        self.data = Some(data);
        self
    }

    pub fn with_trade_costs(&mut self, trade_costs: Vec<BrokerCost>) -> &mut Self {
        self.trade_costs = trade_costs;
        self
    }

    pub fn new() -> Self {
        ConcurrentBrokerBuilder {
            data: None,
            trade_costs: Vec::new(),
            _quote: PhantomData,
            _dividend: PhantomData,
        }
    }
}

impl<T, Q, D> Default for ConcurrentBrokerBuilder<T, Q, D>
where
    Q: Quotable,
    D: Dividendable,
    T: DataSource<Q, D>,
{
    fn default() -> Self {
        Self::new()
    }
}
