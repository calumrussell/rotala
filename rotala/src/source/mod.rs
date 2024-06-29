//! Sources are external data sources that are used to create Inputs and then Exchanges. Source
//! creation should be hidden from users and embedded within the creation of Inputs. Each Source
//! should have its own internal format that is converted into an Input format within the Input.
pub mod binance;
pub mod hyperliquid;
