use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::broker::implement::multi::ConcurrentBroker;
use crate::broker::{BrokerCost, BrokerLog};
use crate::exchange::implement::multi::ConcurrentExchange;
use crate::input::{CorporateEventsSource, Dividendable, PriceSource, Quotable};
use crate::types::{CashValue, PortfolioHoldings};

/// Used to build [ConcurrentBroker].
/// 
/// Broker should be the only owner of a [CorporateEventsSource] in a backtest.
/// 
/// Trade costs are optional. If no trade costs are passed to the broker then no costs will be
/// taken when orders execute.
pub struct ConcurrentBrokerBuilder<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    //Cannot run without data but can run with empty trade_costs
    corporate_source: Option<T>,
    trade_costs: Vec<BrokerCost>,
    dividend: PhantomData<D>,
    quote: PhantomData<Q>,
}

impl<D, T, Q> ConcurrentBrokerBuilder<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    pub async fn build<P: PriceSource<Q>>(
        &mut self,
        exchange: &mut ConcurrentExchange<Q, P>,
    ) -> ConcurrentBroker<D, T, Q> {
        let (subscriber_id, mut price_rx, notify_rx, order_tx) = exchange.subscribe().await;

        let mut first_quotes = HashMap::new();
        while let Ok(quotes) = price_rx.try_recv() {
            for quote in &quotes {
                first_quotes.insert(quote.get_symbol().to_string(), Arc::clone(quote));
            }
        }
        let corporate_source = std::mem::take(&mut self.corporate_source);

        let holdings = PortfolioHoldings::new();
        let log = BrokerLog::new();

        ConcurrentBroker {
            corporate_source,
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
            dividend: PhantomData,
        }
    }

    pub fn with_corporate_source(&mut self, corporate_source: T) -> &mut Self {
        self.corporate_source = Some(corporate_source);
        self
    }

    pub fn with_trade_costs(&mut self, trade_costs: Vec<BrokerCost>) -> &mut Self {
        self.trade_costs = trade_costs;
        self
    }

    pub fn new() -> Self {
        ConcurrentBrokerBuilder {
            corporate_source: None,
            trade_costs: Vec::new(),
            dividend: PhantomData,
            quote: PhantomData,
        }
    }
}

impl<D, T, Q> Default for ConcurrentBrokerBuilder<D, T, Q>
where
    D: Dividendable,
    T: CorporateEventsSource<D>,
    Q: Quotable,
{
    fn default() -> Self {
        Self::new()
    }
}
