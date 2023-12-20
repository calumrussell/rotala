use crate::clock::Clock;
use crate::input::penelope::{Penelope, PenelopeQuote};
use crate::orderbook::diana::{Diana, DianaOrderId, DianaOrderImpl, DianaTrade};

#[derive(Clone, Debug)]
pub enum TradeType {
    Buy,
    Sell,
}

#[derive(Clone, Debug)]
pub struct Trade {
    pub symbol: String,
    pub value: f64,
    pub quantity: f64,
    pub date: i64,
    pub typ: TradeType,
}

pub struct InitMessage {
    pub start: i64,
    pub frequency: u64,
}

pub struct Uist {
    clock: Clock,
    price_source: Penelope,
    orderbook: Diana,
    trade_log: Vec<DianaTrade>,
    //This is cleared on every tick
    order_buffer: Vec<DianaOrderImpl>,
}

impl Uist {
    pub fn new(clock: Clock, price_source: Penelope) -> Self {
        Self {
            clock,
            price_source,
            orderbook: Diana::default(),
            trade_log: Vec::new(),
            order_buffer: Vec::new(),
        }
    }

    fn init() -> InitMessage {
        InitMessage {
            start: 100,
            frequency: 100,
        }
    }

    fn fetch_quotes(&self) -> Vec<PenelopeQuote> {
        if let Some(quotes) = self.price_source.get_quotes(&self.clock.now()) {
            return quotes;
        }
        vec![]
    }

    fn fetch_trades(&self, from: usize) -> Vec<DianaTrade> {
        self.trade_log[from..].to_vec()
    }

    fn insert_order(&mut self, order: DianaOrderImpl) {
        self.order_buffer.push(order);
    }

    fn delete_order(&mut self, order_id: DianaOrderId) {
        self.orderbook.delete_order(order_id);
    }

    fn check(&mut self) -> Vec<DianaTrade> {
        //To eliminate lookahead bias, we only start executing orders on the next
        //tick.
        self.clock.tick();

        for order in &self.order_buffer {
            self.orderbook.insert_order(order.clone());
        }

        let now = self.clock.now();
        let executed_trades = self.orderbook.execute_orders(*now, &self.price_source);
        self.trade_log.extend(executed_trades.clone());
        self.order_buffer.clear();
        executed_trades
    }
}

#[cfg(test)]
mod tests {
    use super::Uist;
    use crate::input::penelope::Penelope;
    use crate::orderbook::diana::DianaOrderImpl;

    fn setup() -> Uist {
        let clock = crate::clock::ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&crate::clock::Frequency::Second)
            .build();

        let mut price_source = Penelope::new();
        price_source.add_quotes(101.00, 102.00, 100, "ABC".to_owned());
        price_source.add_quotes(102.00, 103.00, 101, "ABC".to_owned());
        price_source.add_quotes(105.00, 106.00, 102, "ABC".to_owned());

        let exchange = Uist::new(clock, price_source);
        exchange
    }

    #[test]
    fn test_that_buy_market_executes_incrementing_trade_log() {
        let mut exchange = setup();

        exchange.insert_order(DianaOrderImpl::market_buy("ABC", 100.0));
        exchange.check();

        //TODO: no abstraction!
        assert_eq!(exchange.trade_log.len(), 1);
    }

    #[test]
    fn test_that_multiple_orders_are_executed_on_same_tick() {
        let mut exchange = setup();

        exchange.insert_order(DianaOrderImpl::market_buy("ABC", 25.0));
        exchange.insert_order(DianaOrderImpl::market_buy("ABC", 25.0));
        exchange.insert_order(DianaOrderImpl::market_buy("ABC", 25.0));
        exchange.insert_order(DianaOrderImpl::market_buy("ABC", 25.0));

        exchange.check();
        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_multiple_orders_are_executed_on_consecutive_tick() {
        let mut exchange = setup();
        exchange.insert_order(DianaOrderImpl::market_buy("ABC", 25.0));
        exchange.insert_order(DianaOrderImpl::market_buy("ABC", 25.0));
        exchange.check();

        exchange.insert_order(DianaOrderImpl::market_buy("ABC", 25.0));
        exchange.insert_order(DianaOrderImpl::market_buy("ABC", 25.0));
        exchange.check();

        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_buy_market_executes_on_next_tick() {
        //Verifies that trades do not execute instaneously removing lookahead bias
        let mut exchange = setup();

        exchange.insert_order(DianaOrderImpl::market_buy("ABC", 100.0));
        exchange.check();

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

        exchange.insert_order(DianaOrderImpl::market_sell("ABC", 100.0));
        exchange.check();

        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 103
        assert_eq!(trade.value / trade.quantity, 102.00);
        assert_eq!(trade.date, 101);
    }

    #[test]
    fn test_that_order_for_nonexistent_stock_fails_silently() {
        let mut exchange = setup();

        exchange.insert_order(DianaOrderImpl::market_buy("XYZ", 100.0));
        exchange.check();

        assert_eq!(exchange.trade_log.len(), 0);
    }

    #[test]
    fn test_that_order_buffer_clears() {
        //Sounds redundant but accidentally removing the clear could cause unusual errors elsewhere
        let mut exchange = setup();

        exchange.insert_order(DianaOrderImpl::market_buy("ABC", 100.0));
        exchange.check();

        assert!(exchange.order_buffer.is_empty());
    }

    #[test]
    fn test_that_order_with_missing_price_executes_later() {
        let clock = crate::clock::ClockBuilder::with_length_in_seconds(100, 3)
            .with_frequency(&crate::clock::Frequency::Second)
            .build();

        let mut price_source = Penelope::new();
        price_source.add_quotes(101.00, 102.00, 100, "ABC".to_owned());
        price_source.add_quotes(105.00, 106.00, 102, "ABC".to_owned());

        let mut exchange = Uist::new(clock, price_source);

        exchange.insert_order(DianaOrderImpl::market_buy("ABC", 100.0));
        exchange.check();
        //Orderbook should have one order and trade log has no executed trades
        assert_eq!(exchange.trade_log.len(), 0);

        exchange.check();
        //Order should execute now
        assert_eq!(exchange.trade_log.len(), 1);
    }
}
