use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::ops::Deref;

use crate::clock::Clock;
use crate::input::penelope::{Penelope, PenelopeQuote};
use crate::orderbook::diana::{Diana, DianaOrder, DianaOrderId, DianaOrderType, DianaTrade};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum UistOrderType {
    MarketBuy,
    MarketSell,
    LimitBuy,
    LimitSell,
    StopBuy,
    StopSell,
}

impl From<UistOrderType> for DianaOrderType {
    fn from(value: UistOrderType) -> Self {
        match value {
            UistOrderType::MarketBuy => DianaOrderType::MarketBuy,
            UistOrderType::MarketSell => DianaOrderType::MarketSell,
            UistOrderType::LimitBuy => DianaOrderType::LimitBuy,
            UistOrderType::LimitSell => DianaOrderType::LimitSell,
            UistOrderType::StopBuy => DianaOrderType::StopBuy,
            UistOrderType::StopSell => DianaOrderType::StopSell,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UistOrderId(DianaOrderId);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UistTrade(DianaTrade);

impl Deref for UistTrade {
    type Target = DianaTrade;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UistOrder(DianaOrder);

impl Deref for UistOrder {
    type Target = DianaOrder;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl UistOrder {
    fn market(order_type: UistOrderType, symbol: impl Into<String>, shares: f64) -> Self {
        Self(DianaOrder {
            order_type: order_type.into(),
            symbol: symbol.into(),
            shares,
            price: None,
        })
    }

    fn delayed(
        order_type: UistOrderType,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
    ) -> Self {
        Self(DianaOrder {
            order_type: order_type.into(),
            symbol: symbol.into(),
            shares,
            price: Some(price),
        })
    }

    pub fn market_buy(symbol: impl Into<String>, shares: f64) -> Self {
        UistOrder::market(UistOrderType::MarketBuy, symbol, shares)
    }

    pub fn market_sell(symbol: impl Into<String>, shares: f64) -> Self {
        UistOrder::market(UistOrderType::MarketSell, symbol, shares)
    }

    pub fn stop_buy(symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        UistOrder::delayed(UistOrderType::StopBuy, symbol, shares, price)
    }

    pub fn stop_sell(symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        UistOrder::delayed(UistOrderType::StopSell, symbol, shares, price)
    }

    pub fn limit_buy(symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        UistOrder::delayed(UistOrderType::LimitBuy, symbol, shares, price)
    }

    pub fn limit_sell(symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        UistOrder::delayed(UistOrderType::LimitSell, symbol, shares, price)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InitMessage {
    pub start: i64,
    pub frequency: u8,
}

pub struct Uist {
    clock: Clock,
    price_source: Penelope,
    orderbook: Diana,
    trade_log: Vec<UistTrade>,
    //This is cleared on every tick
    order_buffer: Vec<UistOrder>,
}

impl Uist {
    pub fn from_binance() -> Self {
        let (penelope, clock) = Penelope::from_binance();
        Self::new(clock, penelope)
    }

    pub fn new(clock: Clock, price_source: Penelope) -> Self {
        Self {
            clock,
            price_source,
            orderbook: Diana::default(),
            trade_log: Vec::new(),
            order_buffer: Vec::new(),
        }
    }

    pub fn init(&self) -> InitMessage {
        InitMessage {
            start: *self.clock.now(),
            frequency: self.clock.frequency().clone().into(),
        }
    }

    pub fn fetch_quotes(&self) -> Vec<PenelopeQuote> {
        if let Some(quotes) = self.price_source.get_quotes(&self.clock.now()) {
            return quotes;
        }
        vec![]
    }

    pub fn fetch_trades(&self, from: usize) -> Vec<UistTrade> {
        self.trade_log[from..].to_vec()
    }

    pub fn insert_order(&mut self, order: UistOrder) {
        self.order_buffer.push(order);
    }

    pub fn delete_order(&mut self, order_id: UistOrderId) {
        self.orderbook.delete_order(order_id.0);
    }

    pub fn check(&mut self) -> Vec<UistTrade> {
        //To eliminate lookahead bias, we only start executing orders on the next
        //tick.
        self.clock.tick();

        for order in &self.order_buffer {
            self.orderbook.insert_order(order.0.clone());
        }

        let now = self.clock.now();
        let executed_trades = self.orderbook.execute_orders(*now, &self.price_source);
        let mut executed_trades_internal_format = Vec::new();
        for executed_trade in executed_trades {
            self.trade_log.push(UistTrade(executed_trade.clone()));
            executed_trades_internal_format.push(UistTrade(executed_trade.clone()));
        }
        self.order_buffer.clear();
        executed_trades_internal_format
    }
}

/// Generates random [Uist] for use in tests that don't depend on prices.
pub fn random_uist_generator(length: i64) -> Uist {
    let clock = crate::clock::ClockBuilder::with_length_in_seconds(100, length)
        .with_frequency(&crate::clock::Frequency::Second)
        .build();

    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    let mut penelope = Penelope::new();
    for date in clock.peek() {
        penelope.add_quotes(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            *date,
            "ABC",
        );
        penelope.add_quotes(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            *date,
            "BCD",
        );
    }

    Uist::new(clock, penelope)
}

#[cfg(test)]
mod tests {
    use super::Uist;
    use crate::exchange::uist::UistOrder;
    use crate::input::penelope::Penelope;

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

        exchange.insert_order(UistOrder::market_buy("ABC", 100.0));
        exchange.check();

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

        exchange.check();
        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_multiple_orders_are_executed_on_consecutive_tick() {
        let mut exchange = setup();
        exchange.insert_order(UistOrder::market_buy("ABC", 25.0));
        exchange.insert_order(UistOrder::market_buy("ABC", 25.0));
        exchange.check();

        exchange.insert_order(UistOrder::market_buy("ABC", 25.0));
        exchange.insert_order(UistOrder::market_buy("ABC", 25.0));
        exchange.check();

        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_buy_market_executes_on_next_tick() {
        //Verifies that trades do not execute instaneously removing lookahead bias
        let mut exchange = setup();

        exchange.insert_order(UistOrder::market_buy("ABC", 100.0));
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

        exchange.insert_order(UistOrder::market_sell("ABC", 100.0));
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

        exchange.insert_order(UistOrder::market_buy("XYZ", 100.0));
        exchange.check();

        assert_eq!(exchange.trade_log.len(), 0);
    }

    #[test]
    fn test_that_order_buffer_clears() {
        //Sounds redundant but accidentally removing the clear could cause unusual errors elsewhere
        let mut exchange = setup();

        exchange.insert_order(UistOrder::market_buy("ABC", 100.0));
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

        exchange.insert_order(UistOrder::market_buy("ABC", 100.0));
        exchange.check();
        //Orderbook should have one order and trade log has no executed trades
        assert_eq!(exchange.trade_log.len(), 0);

        exchange.check();
        //Order should execute now
        assert_eq!(exchange.trade_log.len(), 1);
    }
}
