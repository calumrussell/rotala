
pub mod rhea {
    use crate::{Order, Quote, ExchangeTrade, ExchangeOrder};

    pub type ExchangeOrderId = u64;

    pub struct InitMessage {
        pub start: i64,
        pub frequency: u64
    }

    pub trait RheaTrait {
        fn init() -> InitMessage;
        fn insert_order(&mut self, order: ExchangeOrder);
        fn delete_order(&mut self, order_id: ExchangeOrderId);
        fn fetch_quotes(&self) -> Vec<Quote>;
        fn fetch_trades(&self, from: usize) -> Vec<ExchangeTrade>;
        fn check(&mut self) -> Vec<ExchangeTrade>;
    }
}