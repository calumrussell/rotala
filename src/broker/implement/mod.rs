//! Broker implementation
//! 
//! ### Single-threaded detail
//! 
//! [SingleBroker] holds a reference to an exchange to which orders are passed for
//! execution. Orders are not executed until the next tick so broker state has to be synchronized
//! with the exchange on every tick. Clients can trigger this synchronization by calling `check`. 
//! 
//! Broker should be the only owner of a [CorporateEventsSource] in a backtest.
//! 
//! Broker should be the only owner of a [SingleExchange] in a backtest.
//! 
//! Trade costs are optional. If no trade costs are passed to the broker then no costs will be
//! taken when orders execute.
//! 
//! ### Multi-threaded detail
//! 
//! [ConcurrentBroker] holds a reference to channels for:
//! * Receiving price updates from an `Exchange`
//! * Receiving notifications, for example completed trades, from an `Exchange`
//! * Sending orders to an `Exchange` 
//! 
//! Every strategy in a multi-threaded environment has a broker. Every broker is 
//! assigned a unique id by the `Exchange` when initiailizing channels to the `Exchange`. Strategy-
//! level metrics, such as position profit which could be an input used to create new trades, are
//! calculated without sharing between brokers. So all channels are shared but the unique id is
//! used to denote which broker is sending/receiving. 
//! 
//! Broker should be the only owner of a [CorporateEventsSource] in a backtest.
//! 
//! Trade costs are optional. If no trade costs are passed to the broker then no costs will be
//! taken when orders execute.
//! 
//! ### General comments
//! 
//! The broker can hold negative cash values due to the non-immediate execution of trades. Once a
//! broker has received the notification of completed trades and finds a negative value then
//! re-balancing is triggered automatically. Responsibility for moving the portfolio back to the
//! correct state is left with owner, broker implementations take responsibility for correcting
//! invalid internal state (like negative cash values).
//! 
//! If a series has high levels of volatility between periods then performance will fail to
//! replicate the strategy due to continued rebalancing.
//! 
//! To minimize the distortions due to rebalancing behaviour, the broker will target a minimum
//! cash value. This is currently set at an arbitrary level, 1_000, as this is something library
//! dependent and potentially difficult to explain.
//! 
//! If a portfolio has a negative value, the current behaviour is to continue trading potentially
//! producing unexpected results. Previous versions would exit early when this happened but this
//! behaviour was removed.
//! 
//! Default implementations support multiple [BrokerCost] models: Flat, PerShare, and PctOfValue.
//! 
//! Cash balances are held in single currency which is assumed to be the same currency used across
//! the simulation.
//! 
//! Keeps an internal log of trades executed and dividends received/paid. This is distinct from
//! performance calculations.

#[allow(unused)]
use single::SingleBroker;
#[allow(unused)]
use multi::ConcurrentBroker;
#[allow(unused)]
use crate::input::CorporateEventsSource;
#[allow(unused)]
use crate::exchange::implement::single::SingleExchange;
#[allow(unused)]
use crate::broker::BrokerCost;

pub mod single;
pub mod multi;