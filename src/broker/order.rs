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
    order_type: OrderType,
    symbol: String,
    shares: f64,
    price: Option<f64>,
}

impl Order {
    pub fn get_symbol(&self) -> String {
        self.symbol.clone()
    }

    pub fn get_shares(&self) -> f64 {
        self.shares
    }

    pub fn get_price(&self) -> Option<f64> {
        self.price
    }

    pub fn get_order_type(&self) -> OrderType {
        self.order_type
    }

    pub fn new(order_type: OrderType, symbol: String, shares: f64, price: Option<f64> ) -> Self {
        Order {
            order_type,
            symbol,
            shares,
            price
        }
    }
}

pub trait OrderExecutor {
    fn execute_order(&mut self, order: &Order) -> BrokerEvent;
    fn execute_orders(&mut self, orders: Vec<Order>) -> Vec<BrokerEvent>;
}
