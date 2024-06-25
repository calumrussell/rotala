use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::input::penelope::{PenelopeQuote, PenelopeQuoteByDate};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UistQuote {
    pub bid: f64,
    pub ask: f64,
    pub date: i64,
    pub symbol: String,
}

impl From<PenelopeQuote> for UistQuote {
    fn from(value: PenelopeQuote) -> Self {
        Self {
            bid: value.bid,
            ask: value.ask,
            date: value.date,
            symbol: value.symbol,
        }
    }
}

pub type OrderId = u64;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum TradeType {
    Buy,
    Sell,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum OrderType {
    MarketSell,
    MarketBuy,
    LimitSell,
    LimitBuy,
    StopSell,
    StopBuy,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Trade {
    pub symbol: String,
    pub value: f64,
    pub quantity: f64,
    pub date: i64,
    pub typ: TradeType,
}

impl Trade {
    pub fn new(
        symbol: impl Into<String>,
        value: f64,
        quantity: f64,
        date: i64,
        typ: TradeType,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            value,
            quantity,
            date,
            typ,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Order {
    pub order_id: Option<OrderId>,
    pub order_type: OrderType,
    pub symbol: String,
    pub shares: f64,
    pub price: Option<f64>,
}

impl Order {
    pub fn get_shares(&self) -> f64 {
        self.shares
    }

    pub fn get_symbol(&self) -> &str {
        &self.symbol
    }
    pub fn get_price(&self) -> &Option<f64> {
        &self.price
    }

    pub fn get_order_type(&self) -> &OrderType {
        &self.order_type
    }

    fn set_order_id(&mut self, order_id: u64) {
        self.order_id = Some(order_id);
    }

    fn market(order_type: OrderType, symbol: impl Into<String>, shares: f64) -> Self {
        Self {
            order_id: None,
            order_type,
            symbol: symbol.into(),
            shares,
            price: None,
        }
    }

    fn delayed(order_type: OrderType, symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        Self {
            order_id: None,
            order_type,
            symbol: symbol.into(),
            shares,
            price: Some(price),
        }
    }

    pub fn market_buy(symbol: impl Into<String>, shares: f64) -> Self {
        Order::market(OrderType::MarketBuy, symbol, shares)
    }

    pub fn market_sell(symbol: impl Into<String>, shares: f64) -> Self {
        Order::market(OrderType::MarketSell, symbol, shares)
    }

    pub fn stop_buy(symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        Order::delayed(OrderType::StopBuy, symbol, shares, price)
    }

    pub fn stop_sell(symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        Order::delayed(OrderType::StopSell, symbol, shares, price)
    }

    pub fn limit_buy(symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        Order::delayed(OrderType::LimitBuy, symbol, shares, price)
    }

    pub fn limit_sell(symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        Order::delayed(OrderType::LimitSell, symbol, shares, price)
    }
}

impl Eq for Order {}

impl PartialEq for Order {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
            && self.order_type == other.order_type
            && self.shares == other.shares
    }
}

#[derive(Clone, Debug)]
pub struct UistV1 {
    orderbook: OrderBook,
    trade_log: Vec<Trade>,
    //This is cleared on every tick
    order_buffer: Vec<Order>,
}

impl UistV1 {
    pub fn new() -> Self {
        Self {
            orderbook: OrderBook::default(),
            trade_log: Vec::new(),
            order_buffer: Vec::new(),
        }
    }

    fn sort_order_buffer(&mut self) {
        self.order_buffer.sort_by(|a, _b| match a.get_order_type() {
            OrderType::LimitSell | OrderType::StopSell | OrderType::MarketSell => {
                std::cmp::Ordering::Less
            }
            _ => std::cmp::Ordering::Greater,
        })
    }

    pub fn insert_order(&mut self, order: Order) {
        // Orders are only inserted into the book when tick is called, this is to ensure proper
        // ordering of trades
        // This impacts order_id where an order X can come in before order X+1 but the latter can
        // have an order_id that is less than the former.
        self.order_buffer.push(order);
    }

    pub fn delete_order(&mut self, order_id: OrderId) {
        self.orderbook.delete_order(order_id);
    }

    pub fn tick(&mut self, quotes: &PenelopeQuoteByDate) -> (Vec<Trade>, Vec<Order>) {
        //To eliminate lookahead bias, we only insert new orders after we have executed any orders
        //that were on the stack first
        let executed_trades = self.orderbook.execute_orders(quotes);
        for executed_trade in &executed_trades {
            self.trade_log.push(executed_trade.clone());
        }

        self.sort_order_buffer();
        for order in self.order_buffer.iter_mut() {
            self.orderbook.insert_order(order);
        }

        let inserted_orders = std::mem::take(&mut self.order_buffer);
        (executed_trades, inserted_orders)
    }
}

impl Default for UistV1 {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
struct OrderBook {
    inner: VecDeque<Order>,
    last_inserted: u64,
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            inner: std::collections::VecDeque::new(),
            last_inserted: 0,
        }
    }

    pub fn delete_order(&mut self, delete_order_id: u64) {
        let mut delete_position: Option<usize> = None;
        for (position, order) in self.inner.iter().enumerate() {
            if let Some(order_id) = order.order_id {
                if order_id == delete_order_id {
                    delete_position = Some(position);
                    break;
                }
            }
        }
        if let Some(position) = delete_position {
            self.inner.remove(position);
        }
    }

    pub fn insert_order(&mut self, order: &mut Order) {
        order.set_order_id(self.last_inserted);
        self.inner.push_back(order.clone());
        self.last_inserted += 1;
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn execute_buy(quote: UistQuote, order: &Order, date: i64) -> Trade {
        let trade_price = quote.ask;
        let value = trade_price * order.get_shares();
        Trade {
            symbol: order.get_symbol().to_string(),
            value,
            quantity: order.get_shares(),
            date,
            typ: TradeType::Buy,
        }
    }

    fn execute_sell(quote: UistQuote, order: &Order, date: i64) -> Trade {
        let trade_price = quote.bid;
        let value = trade_price * order.get_shares();
        Trade {
            symbol: order.get_symbol().to_string(),
            value,
            quantity: order.get_shares(),
            date,
            typ: TradeType::Sell,
        }
    }

    pub fn execute_orders(&mut self, quotes: &PenelopeQuoteByDate) -> Vec<Trade> {
        let mut completed_orderids = Vec::new();
        let mut trade_results = Vec::new();
        if self.is_empty() {
            return trade_results;
        }
        for order in self.inner.iter() {
            let security_id = &order.symbol;
            if let Some(quote) = quotes.get(security_id) {
                let quote_copy: UistQuote = quote.clone().into();
                let date = quote_copy.date;
                let result = match order.order_type {
                    OrderType::MarketBuy => Some(Self::execute_buy(quote_copy, order, date)),
                    OrderType::MarketSell => Some(Self::execute_sell(quote_copy, order, date)),
                    OrderType::LimitBuy => {
                        //Unwrap is safe because LimitBuy will always have a price
                        let order_price = order.price;
                        if order_price >= Some(quote_copy.ask) {
                            Some(Self::execute_buy(quote_copy, order, date))
                        } else {
                            None
                        }
                    }
                    OrderType::LimitSell => {
                        //Unwrap is safe because LimitSell will always have a price
                        let order_price = order.price;
                        if order_price <= Some(quote_copy.bid) {
                            Some(Self::execute_sell(quote_copy, order, date))
                        } else {
                            None
                        }
                    }
                    OrderType::StopBuy => {
                        //Unwrap is safe because StopBuy will always have a price
                        let order_price = order.price;
                        if order_price <= Some(quote_copy.ask) {
                            Some(Self::execute_buy(quote_copy, order, date))
                        } else {
                            None
                        }
                    }
                    OrderType::StopSell => {
                        //Unwrap is safe because StopSell will always have a price
                        let order_price = order.price;
                        if order_price >= Some(quote_copy.bid) {
                            Some(Self::execute_sell(quote_copy, order, date))
                        } else {
                            None
                        }
                    }
                };
                if let Some(trade) = &result {
                    completed_orderids.push(order.order_id.unwrap());
                    trade_results.push(trade.clone());
                }
            }
        }
        for order_id in completed_orderids {
            self.delete_order(order_id);
        }
        trade_results
    }
}

#[cfg(test)]
mod tests {
    use super::UistV1;
    use crate::exchange::uist_v1::OrderType;
    use crate::input::penelope::Penelope;

    use super::Order;

    fn setup() -> (Penelope, UistV1) {
        let mut source = Penelope::new();
        source.add_quote(101.00, 102.00, 100, "ABC".to_owned());
        source.add_quote(102.00, 103.00, 101, "ABC".to_owned());
        source.add_quote(105.00, 106.00, 102, "ABC".to_owned());

        let exchange = UistV1::new();
        (source, exchange)
    }

    #[test]
    fn test_that_buy_market_executes_incrementing_trade_log() {
        let (source, mut exchange) = setup();

        exchange.insert_order(Order::market_buy("ABC", 100.0));
        exchange.tick(source.get_quotes_unchecked(&100));
        exchange.tick(source.get_quotes_unchecked(&101));

        //TODO: no abstraction!
        assert_eq!(exchange.trade_log.len(), 1);
    }

    #[test]
    fn test_that_multiple_orders_are_executed_on_same_tick() {
        let (source, mut exchange) = setup();

        exchange.insert_order(Order::market_buy("ABC", 25.0));
        exchange.insert_order(Order::market_buy("ABC", 25.0));
        exchange.insert_order(Order::market_buy("ABC", 25.0));
        exchange.insert_order(Order::market_buy("ABC", 25.0));

        exchange.tick(source.get_quotes_unchecked(&100));
        exchange.tick(source.get_quotes_unchecked(&101));
        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_multiple_orders_are_executed_on_consecutive_tick() {
        let (source, mut exchange) = setup();
        exchange.insert_order(Order::market_buy("ABC", 25.0));
        exchange.insert_order(Order::market_buy("ABC", 25.0));
        exchange.tick(source.get_quotes_unchecked(&100));

        exchange.insert_order(Order::market_buy("ABC", 25.0));
        exchange.insert_order(Order::market_buy("ABC", 25.0));
        exchange.tick(source.get_quotes_unchecked(&101));
        exchange.tick(source.get_quotes_unchecked(&102));

        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_buy_market_executes_on_next_tick() {
        //Verifies that trades do not execute instaneously removing lookahead bias
        let (source, mut exchange) = setup();

        exchange.insert_order(Order::market_buy("ABC", 100.0));
        exchange.tick(source.get_quotes_unchecked(&100));
        exchange.tick(source.get_quotes_unchecked(&101));

        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 103
        assert_eq!(trade.value / trade.quantity, 103.00);
        assert_eq!(trade.date, 101);
    }

    #[test]
    fn test_that_sell_market_executes_on_next_tick() {
        //Verifies that trades do not execute instaneously removing lookahead bias
        let (source, mut exchange) = setup();

        exchange.insert_order(Order::market_sell("ABC", 100.0));
        exchange.tick(source.get_quotes_unchecked(&100));
        exchange.tick(source.get_quotes_unchecked(&101));

        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 103
        assert_eq!(trade.value / trade.quantity, 102.00);
        assert_eq!(trade.date, 101);
    }

    #[test]
    fn test_that_order_for_nonexistent_stock_fails_silently() {
        let (source, mut exchange) = setup();

        exchange.insert_order(Order::market_buy("XYZ", 100.0));
        exchange.tick(source.get_quotes_unchecked(&100));

        assert_eq!(exchange.trade_log.len(), 0);
    }

    #[test]
    fn test_that_order_buffer_clears() {
        //Sounds redundant but accidentally removing the clear could cause unusual errors elsewhere
        let (source, mut exchange) = setup();

        exchange.insert_order(Order::market_buy("ABC", 100.0));
        exchange.tick(source.get_quotes_unchecked(&100));

        assert!(exchange.order_buffer.is_empty());
    }

    #[test]
    fn test_that_order_with_missing_price_executes_later() {
        let mut source = Penelope::new();
        source.add_quote(101.00, 102.00, 100, "ABC".to_owned());
        source.add_quote(105.00, 106.00, 102, "ABC".to_owned());
        let mut exchange = UistV1::new();

        exchange.insert_order(Order::market_buy("ABC", 100.0));
        exchange.tick(source.get_quotes_unchecked(&100));
        //Orderbook should have one order and trade log has no executed trades
        assert_eq!(exchange.trade_log.len(), 0);

        exchange.tick(source.get_quotes_unchecked(&102));
        //Order should execute now
        assert_eq!(exchange.trade_log.len(), 1);
    }

    #[test]
    fn test_that_sells_are_executed_before_buy() {
        let (source, mut exchange) = setup();

        exchange.insert_order(Order::market_buy("ABC", 100.0));
        exchange.insert_order(Order::market_buy("ABC", 100.0));
        exchange.insert_order(Order::market_sell("ABC", 100.0));
        let res = exchange.tick(source.get_quotes_unchecked(&100));

        assert_eq!(res.1.len(), 3);
        assert_eq!(
            res.1.get(0).unwrap().clone().order_type,
            OrderType::MarketSell
        )
    }
}
