//! Inputs wrap around a dataset providing a simple transparent interface producing a custom quote
//! type that clients should build their operations around.
//!
//! This creates a binding between an orderbook and an input type as the orderbook has to work on
//! quotes of a specific type. It is possible that this becomes more flexible in the future.
//!
//! Sources should be called through inputs so that clients do not have to marshall data into internal
//! types.
pub mod athena;
pub mod minerva;
pub mod penelope;
