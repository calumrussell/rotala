#[derive(Clone, Copy)]
pub enum OrderType {
    MarketSell,
    MarketBuy,
}

#[derive(Clone)]
pub struct Order {
    pub order_type: OrderType,
    pub symbol: String,
    pub shares: i64,
}

impl Order {
    pub fn build_order(qty: i64, symbol: &String) -> Self {
        if qty > 0 {
            Order {
                order_type: OrderType::MarketBuy,
                symbol: symbol.clone(),
                shares: qty,
            }
        } else {
            Order {
                order_type: OrderType::MarketSell,
                symbol: symbol.clone(),
                shares: qty,
            }
        }
    }
}
