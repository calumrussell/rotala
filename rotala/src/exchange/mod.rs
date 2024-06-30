//! Exchanges are the main interface presented to clients. They support a set of operations that
//! are used to run and manage a backtest. However, the majority of the execution logic is passed
//! to Orderbooks and the logic contained within the Exchange itself primarily relates to the
//! orchestration of the backtest (for example, ticking forward or synchronizing state with clients
//! ).
pub mod jura_v1;
pub mod uist_v1;
pub mod uist_v2;
