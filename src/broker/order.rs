use super::BrokerEvent;

#[derive(Clone, Copy, Debug)]
pub enum OrderType {
    MarketSell,
    MarketBuy,
    LimitSell,
    LimitBuy,
    StopSell,
    StopBuy,
}

#[derive(Clone, Debug)]
pub struct Order {
    pub order_type: OrderType,
    pub symbol: String,
    pub shares: f64,
    pub price: Option<f64>,
}

pub trait OrderExecutor {
    fn execute_order(&mut self, order: &Order) -> BrokerEvent;
    fn execute_orders(&mut self, orders: Vec<Order>) -> Vec<BrokerEvent>;
}
