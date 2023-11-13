use std::cmp::Ordering;

use alator_clock::DateTime;

pub mod proto {
    tonic::include_proto!("exchange");
}

#[derive(Clone, Debug)]
pub struct Quote {
    pub bid: f64,
    pub ask: f64,
    pub date: i64,
    pub symbol: String,
}

impl From<Quote> for proto::Quote {
    fn from(value: Quote) -> Self {
        Self {
            bid: value.bid,
            ask: value.ask,
            date: value.date,
            symbol: value.symbol,
        }
    }
}

impl From<proto::Quote> for Quote {
    fn from(value: proto::Quote) -> Self {
        Self {
            bid: value.bid,
            ask: value.ask,
            date: value.date,
            symbol: value.symbol,
        }
    }
}

pub type DefaultExchangeOrderId = u64;
pub type DefaultSubscriberId = u64;

/// Order types supported by the exchange.
///
/// This may be different from order types supported by the broker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderType {
    MarketSell,
    MarketBuy,
    LimitSell,
    LimitBuy,
    StopSell,
    StopBuy,
}

impl From<i32> for OrderType {
    fn from(value: i32) -> Self {
        match value {
            0 => OrderType::MarketSell,
            1 => OrderType::MarketBuy,
            2 => OrderType::LimitSell,
            3 => OrderType::LimitBuy,
            4 => OrderType::StopSell,
            5 => OrderType::StopBuy,
            _ => unimplemented!("0/1/2/3 are only types supported"),
        }
    }
}

impl From<OrderType> for i32 {
    fn from(value: OrderType) -> Self {
        match value {
            OrderType::MarketSell => 0,
            OrderType::MarketBuy => 1,
            OrderType::LimitSell => 2,
            OrderType::LimitBuy => 3,
            OrderType::StopSell => 4,
            OrderType::StopBuy => 5,
        }
    }
}

/// Order supported by the exchange.
///
/// This may be different from order created by the broker.
#[derive(Clone, Debug)]
pub struct ExchangeOrder {
    // TODO: has pub fields and getters for some reason
    //This is used for multi-threaded and single. With single it is just constant.
    pub subscriber_id: DefaultSubscriberId,
    pub order_type: OrderType,
    pub symbol: String,
    pub shares: f64,
    pub price: Option<f64>,
}

impl ExchangeOrder {
    pub fn get_subscriber_id(&self) -> &DefaultSubscriberId {
        &self.subscriber_id
    }

    pub fn get_symbol(&self) -> &String {
        &self.symbol
    }

    pub fn get_shares(&self) -> &f64 {
        &self.shares
    }

    pub fn get_price(&self) -> &Option<f64> {
        &self.price
    }

    pub fn get_order_type(&self) -> &OrderType {
        &self.order_type
    }

    fn market(
        subscriber_id: DefaultSubscriberId,
        order_type: OrderType,
        symbol: impl Into<String>,
        shares: f64,
    ) -> Self {
        Self {
            subscriber_id,
            order_type,
            symbol: symbol.into(),
            shares,
            price: None,
        }
    }

    fn delayed(
        subscriber_id: DefaultSubscriberId,
        order_type: OrderType,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
    ) -> Self {
        Self {
            subscriber_id,
            order_type,
            symbol: symbol.into(),
            shares,
            price: Some(price),
        }
    }

    pub fn market_buy(
        subscriber_id: DefaultSubscriberId,
        symbol: impl Into<String>,
        shares: f64,
    ) -> Self {
        ExchangeOrder::market(subscriber_id, OrderType::MarketBuy, symbol, shares)
    }

    pub fn market_sell(
        subscriber_id: DefaultSubscriberId,
        symbol: impl Into<String>,
        shares: f64,
    ) -> Self {
        ExchangeOrder::market(subscriber_id, OrderType::MarketSell, symbol, shares)
    }

    pub fn stop_buy(
        subscriber_id: DefaultSubscriberId,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
    ) -> Self {
        ExchangeOrder::delayed(subscriber_id, OrderType::StopBuy, symbol, shares, price)
    }

    pub fn stop_sell(
        subscriber_id: DefaultSubscriberId,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
    ) -> Self {
        ExchangeOrder::delayed(subscriber_id, OrderType::StopSell, symbol, shares, price)
    }

    pub fn limit_buy(
        subscriber_id: DefaultSubscriberId,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
    ) -> Self {
        ExchangeOrder::delayed(subscriber_id, OrderType::LimitBuy, symbol, shares, price)
    }

    pub fn limit_sell(
        subscriber_id: DefaultSubscriberId,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
    ) -> Self {
        ExchangeOrder::delayed(subscriber_id, OrderType::LimitSell, symbol, shares, price)
    }
}

impl Eq for ExchangeOrder {}

impl PartialEq for ExchangeOrder {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
            && self.order_type == other.order_type
            && self.shares == other.shares
    }
}

impl From<ExchangeOrder> for proto::Order {
    fn from(value: ExchangeOrder) -> Self {
        Self {
            symbol: value.symbol,
            quantity: value.shares,
            price: value.price,
            order_type: value.order_type.into(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TradeType {
    Buy,
    Sell,
}

impl From<i32> for TradeType {
    fn from(value: i32) -> Self {
        match value {
            0 => TradeType::Buy,
            1 => TradeType::Sell,
            _ => unimplemented!("0/1 are only types supported"),
        }
    }
}

impl From<TradeType> for i32 {
    fn from(value: TradeType) -> Self {
        match value {
            TradeType::Buy => 0,
            TradeType::Sell => 1,
        }
    }
}

/// Trade generated by exchange when order is executed
#[derive(Clone, Debug)]
pub struct ExchangeTrade {
    //This is used for multi-threaded and single. With single it is just constant.
    pub subscriber_id: DefaultSubscriberId,
    pub symbol: String,
    pub value: f64,
    pub quantity: f64,
    pub date: DateTime,
    pub typ: TradeType,
}

impl ExchangeTrade {
    pub fn new(
        subscriber_id: DefaultSubscriberId,
        symbol: impl Into<String>,
        value: f64,
        quantity: f64,
        date: impl Into<DateTime>,
        typ: TradeType,
    ) -> Self {
        Self {
            subscriber_id,
            symbol: symbol.into(),
            value,
            quantity,
            date: date.into(),
            typ,
        }
    }
}

impl From<proto::Trade> for ExchangeTrade {
    fn from(value: proto::Trade) -> Self {
        Self {
            subscriber_id: value.subscriber_id,
            symbol: value.symbol,
            value: value.value,
            quantity: value.quantity,
            date: value.date.into(),
            typ: value.typ.into(),
        }
    }
}

impl From<ExchangeTrade> for proto::Trade {
    fn from(value: ExchangeTrade) -> Self {
        Self {
            typ: value.typ.into(),
            subscriber_id: value.subscriber_id,
            quantity: value.quantity,
            value: value.value,
            date: *value.date,
            symbol: value.symbol,
        }
    }
}

impl Ord for ExchangeTrade {
    fn cmp(&self, other: &Self) -> Ordering {
        self.date.cmp(&other.date)
    }
}

impl PartialOrd for ExchangeTrade {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for ExchangeTrade {}

impl PartialEq for ExchangeTrade {
    fn eq(&self, other: &Self) -> bool {
        self.date == other.date && self.symbol == other.symbol
    }
}

/// Notifications sent by exchange on the notification channel when operating in multi-threaded
/// environment.
pub enum ExchangeNotificationMessage {
    TradeCompleted(ExchangeTrade),
    OrderBooked(DefaultExchangeOrderId, ExchangeOrder),
    OrderDeleted(DefaultExchangeOrderId),
}

/// Orders that can be sent to exchange on the order channel when operating in multi-threaded
/// environment.
pub enum ExchangeOrderMessage {
    CreateOrder(ExchangeOrder),
    DeleteOrder(DefaultSubscriberId, DefaultExchangeOrderId),
    ClearOrdersBySymbol(DefaultSubscriberId, String),
}

impl ExchangeOrderMessage {
    pub fn market_buy(
        subscriber_id: DefaultSubscriberId,
        symbol: impl Into<String>,
        shares: f64,
    ) -> Self {
        ExchangeOrderMessage::CreateOrder(ExchangeOrder::market_buy(subscriber_id, symbol, shares))
    }

    pub fn market_sell(
        subscriber_id: DefaultSubscriberId,
        symbol: impl Into<String>,
        shares: f64,
    ) -> Self {
        ExchangeOrderMessage::CreateOrder(ExchangeOrder::market_sell(subscriber_id, symbol, shares))
    }

    pub fn stop_buy(
        subscriber_id: DefaultSubscriberId,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
    ) -> Self {
        ExchangeOrderMessage::CreateOrder(ExchangeOrder::stop_buy(
            subscriber_id,
            symbol,
            shares,
            price,
        ))
    }

    pub fn stop_sell(
        subscriber_id: DefaultSubscriberId,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
    ) -> Self {
        ExchangeOrderMessage::CreateOrder(ExchangeOrder::stop_sell(
            subscriber_id,
            symbol,
            shares,
            price,
        ))
    }

    pub fn limit_buy(
        subscriber_id: DefaultSubscriberId,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
    ) -> Self {
        ExchangeOrderMessage::CreateOrder(ExchangeOrder::limit_buy(
            subscriber_id,
            symbol,
            shares,
            price,
        ))
    }

    pub fn limit_sell(
        subscriber_id: DefaultSubscriberId,
        symbol: impl Into<String>,
        shares: f64,
        price: f64,
    ) -> Self {
        ExchangeOrderMessage::CreateOrder(ExchangeOrder::limit_sell(
            subscriber_id,
            symbol,
            shares,
            price,
        ))
    }
}
