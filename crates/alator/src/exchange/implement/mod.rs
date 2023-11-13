//! Exchange implementations
//!
//! ### Single-threaded exchange
//!
//! Exchanges cannot execute orders instaneously in order to prevent lookahead bias. The exchange
//! owner will pass order to the exchange and then have to check back on the next tick to reconcile
//! any completed trades against internal state.
//!
//! The exchange owner must, therefore, call `check` on exchange and synchronize the tick forward
//! with its own update cycle.
//!
//! Within a single-threaded context, the exchange owner only has to make sure that the call to
//! `check` on the exchange is synchronized correctly with modifications to internal state.
//!
//! Internally, the exchange buffers any orders received and only inserts them into the internal
//! book to be executed once `check` has been called and we tick forward.
//!
//! Within library implementations, the exchange also operates as [PriceSource]. Passing price data
//! up to the broker. In some previous versions, each component held a shared reference to the
//! [PriceSource] but, for various reasons, it seems simpler to just have this reference in one
//! place.
//!
//! Within library implementations, the exchange is also responsible for [Clock] ticking forward.
//! In some previous versions, this was done at the top-level of the application and required
//! complex guarantees to ensure that calling functions were ticking forward when every component
//! had completed their operations it is all stuff that has never been tried n the correct order.
//! Moving the tick down to the lowest level removes the requirement for this code. But does
//! also require understanding that calling `check` mutates state across the application.
//!
//! The exchange performs no correctness checks on orders received. The exchange assumes, for example,
//! that clients have the funds to settle the trade. The exchange assumes, for example, that an order
//! is issued for a security that has price data at some point. All checking for this kind of error
//! should be performed outside of the exchange.
//!
//! ### Multi-threaded exchange
//!
//! Exchange accepts messages containing orders and executes them over time.
//!
//! Generic information about exchanges is included in [SingleExchange].
//!
//! Multi-threaded exchanges operate through three channels that:
//! * Send updated prices to recievers.
//! * Send notifications to recievers.
//! * Recieve orders to be executed.
//!
//! An exchange may interact with multiple brokers so issues a unique id to each one that is used
//! for communications that are relevant to individual brokers, for example completed trades.

#[allow(unused)]
use alator_clock::Clock;
#[allow(unused)]
use crate::input::PriceSource;
#[allow(unused)]
use single::SingleExchange;

pub mod multi;
pub mod single;
