use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use serde::{Deserialize, Serialize};

use crate::clock::Clock;
use crate::input::penelope::{Penelope, PenelopeBuilder, PenelopeQuote};
use crate::orderbook::diana::{
    Diana, DianaOrder, DianaOrderId, DianaOrderType, DianaTrade, DianaTradeType,
};

pub type UistTradeType = DianaTradeType;
pub type UistOrderType = DianaOrderType;
pub type UistOrderId = DianaOrderId;
pub type UistQuote = PenelopeQuote;
pub type UistTrade = DianaTrade;
pub type UistOrder = DianaOrder;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InitMessage {
    pub start: i64,
    pub frequency: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InfoMessage {
    pub version: String,
    pub dataset: String,
}

impl InfoMessage {
    fn v1(dataset: String) -> InfoMessage {
        InfoMessage {
            version: "1.0".to_string(),
            dataset,
        }
    }
}

#[derive(Debug)]
pub struct UistV1 {
    dataset: String,
    clock: Clock,
    price_source: Penelope,
    orderbook: Diana,
    trade_log: Vec<UistTrade>,
    //This is cleared on every tick
    order_buffer: Vec<UistOrder>,
}

impl UistV1 {
    pub fn from_binance() -> Self {
        let (penelope, clock) = Penelope::from_binance();
        Self::new(clock, penelope, "BINANCE")
    }

    pub fn new(clock: Clock, price_source: Penelope, dataset: &str) -> Self {
        Self {
            dataset: dataset.into(),
            clock,
            price_source,
            orderbook: Diana::default(),
            trade_log: Vec::new(),
            order_buffer: Vec::new(),
        }
    }

    fn sort_order_buffer(&mut self) {
        self.order_buffer.sort_by(|a, _b| match a.get_order_type() {
            UistOrderType::LimitSell | UistOrderType::StopSell | UistOrderType::MarketSell => {
                std::cmp::Ordering::Less
            }
            _ => std::cmp::Ordering::Greater,
        })
    }

    pub fn info(&self) -> InfoMessage {
        InfoMessage::v1(self.dataset.clone())
    }

    pub fn init(&self) -> InitMessage {
        InitMessage {
            start: *self.clock.now(),
            frequency: self.clock.frequency().clone().into(),
        }
    }

    pub fn fetch_quotes(&self) -> Vec<UistQuote> {
        if let Some(quotes) = self.price_source.get_quotes(&self.clock.now()) {
            return quotes;
        }
        vec![]
    }

    pub fn insert_order(&mut self, order: UistOrder) {
        // Orders are only inserted into the book when tick is called, this is to ensure proper
        // ordering of trades
        // This impacts order_id where an order X can come in before order X+1 but the latter can
        // have an order_id that is less than the former.
        self.order_buffer.push(order);
    }

    pub fn delete_order(&mut self, order_id: UistOrderId) {
        self.orderbook.delete_order(order_id);
    }

    pub fn tick(&mut self) -> (bool, Vec<UistTrade>, Vec<UistOrder>) {
        //To eliminate lookahead bias, we only start executing orders on the next
        //tick.
        self.clock.tick();

        dbg!(&self.order_buffer);
        self.sort_order_buffer();
        for order in self.order_buffer.iter_mut() {
            self.orderbook.insert_order(order);
        }

        let now = self.clock.now();
        let executed_trades = self.orderbook.execute_orders(*now, &self.price_source);
        for executed_trade in &executed_trades {
            self.trade_log.push(executed_trade.clone());
        }
        let inserted_orders = std::mem::take(&mut self.order_buffer);
        (self.clock.has_next(), executed_trades, inserted_orders)
    }
}

/// Generates random [Uist] for use in tests that don't depend on prices.
pub fn random_uist_generator(length: i64) -> (UistV1, Clock) {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    let mut source_builder = PenelopeBuilder::new();

    for date in 100..length + 100 {
        source_builder.add_quote(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "ABC",
        );
        source_builder.add_quote(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "BCD",
        );
    }

    let (penelope, clock) = source_builder.build_with_frequency(crate::clock::Frequency::Second);
    (UistV1::new(clock.clone(), penelope, "RANDOM"), clock)
}

#[cfg(test)]
mod tests {
    use super::UistV1;
    use crate::exchange::uist::UistOrder;
    use crate::input::penelope::PenelopeBuilder;
    use crate::orderbook::diana::DianaTradeType;

    fn setup() -> UistV1 {
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(101.00, 102.00, 100, "ABC".to_owned());
        source_builder.add_quote(102.00, 103.00, 101, "ABC".to_owned());
        source_builder.add_quote(105.00, 106.00, 102, "ABC".to_owned());

        let (source, clock) = source_builder.build_with_frequency(crate::clock::Frequency::Second);

        let exchange = UistV1::new(clock, source, "FAKE");
        exchange
    }

    #[test]
    fn test_that_buy_market_executes_incrementing_trade_log() {
        let mut exchange = setup();

        exchange.insert_order(UistOrder::market_buy("ABC", 100.0));
        exchange.tick();

        //TODO: no abstraction!
        assert_eq!(exchange.trade_log.len(), 1);
    }

    #[test]
    fn test_that_multiple_orders_are_executed_on_same_tick() {
        let mut exchange = setup();

        exchange.insert_order(UistOrder::market_buy("ABC", 25.0));
        exchange.insert_order(UistOrder::market_buy("ABC", 25.0));
        exchange.insert_order(UistOrder::market_buy("ABC", 25.0));
        exchange.insert_order(UistOrder::market_buy("ABC", 25.0));

        exchange.tick();
        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_multiple_orders_are_executed_on_consecutive_tick() {
        let mut exchange = setup();
        exchange.insert_order(UistOrder::market_buy("ABC", 25.0));
        exchange.insert_order(UistOrder::market_buy("ABC", 25.0));
        exchange.tick();

        exchange.insert_order(UistOrder::market_buy("ABC", 25.0));
        exchange.insert_order(UistOrder::market_buy("ABC", 25.0));
        exchange.tick();

        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_buy_market_executes_on_next_tick() {
        //Verifies that trades do not execute instaneously removing lookahead bias
        let mut exchange = setup();

        exchange.insert_order(UistOrder::market_buy("ABC", 100.0));
        exchange.tick();

        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 103
        assert_eq!(trade.value / trade.quantity, 103.00);
        assert_eq!(trade.date, 101);
    }

    #[test]
    fn test_that_sell_market_executes_on_next_tick() {
        //Verifies that trades do not execute instaneously removing lookahead bias
        let mut exchange = setup();

        exchange.insert_order(UistOrder::market_sell("ABC", 100.0));
        exchange.tick();

        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 103
        assert_eq!(trade.value / trade.quantity, 102.00);
        assert_eq!(trade.date, 101);
    }

    #[test]
    fn test_that_order_for_nonexistent_stock_fails_silently() {
        let mut exchange = setup();

        exchange.insert_order(UistOrder::market_buy("XYZ", 100.0));
        exchange.tick();

        assert_eq!(exchange.trade_log.len(), 0);
    }

    #[test]
    fn test_that_order_buffer_clears() {
        //Sounds redundant but accidentally removing the clear could cause unusual errors elsewhere
        let mut exchange = setup();

        exchange.insert_order(UistOrder::market_buy("ABC", 100.0));
        exchange.tick();

        assert!(exchange.order_buffer.is_empty());
    }

    #[test]
    fn test_that_order_with_missing_price_executes_later() {
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(101.00, 102.00, 100, "ABC".to_owned());
        source_builder.add_quote(105.00, 106.00, 102, "ABC".to_owned());

        let (source, clock) = source_builder.build_with_frequency(crate::clock::Frequency::Second);

        let mut exchange = UistV1::new(clock, source, "FAKE");

        exchange.insert_order(UistOrder::market_buy("ABC", 100.0));
        exchange.tick();
        //Orderbook should have one order and trade log has no executed trades
        assert_eq!(exchange.trade_log.len(), 0);

        exchange.tick();
        //Order should execute now
        assert_eq!(exchange.trade_log.len(), 1);
    }

    #[test]
    fn test_that_sells_are_executed_before_buy() {
        let mut exchange = setup();

        exchange.insert_order(UistOrder::market_buy("ABC", 100.0));
        exchange.insert_order(UistOrder::market_buy("ABC", 100.0));
        exchange.insert_order(UistOrder::market_sell("ABC", 100.0));
        let res = exchange.tick();

        assert_eq!(res.1.len(), 3);
        assert_eq!(res.1.get(0).unwrap().typ, DianaTradeType::Sell)
    }
}
