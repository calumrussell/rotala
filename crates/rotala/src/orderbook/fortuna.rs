use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

pub type FortunaOrderId = u64;

#[derive(Clone, Debug, Deserialize, Serialize)]
enum TimeInForce {
    Alo,
    Ioc,
    Gtc,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FortunaLimitOrder {
    tif: TimeInForce,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum TriggerType {
    Tp,
    Sl,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FortunaTriggerOrder {
    trigger_px: f64,
    is_market: bool,
    tpsl: TriggerType,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum FortunaOrderType {
    FortunaLimitOrder,
    FortunaTriggerOrder
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FortunaOrder {
    asset: u64,
    is_buy: bool,
    limit_px: String,
    sz: String,
    reduce_only: bool,
    //This is client order id, need to check whether test impl should reasonably use this.
    cloid: Option<String>,
    order_type: FortunaOrderType, 
}

struct FortunaInnerOrder {
    pub order_id: FortunaOrderId,
    pub order: FortunaOrder,
}

/// Fortuna is an implementation of the HyperLiquid API running against a local server. This allows
/// testing of strategies using the same API/order types/etc.
pub struct Fortuna {
    inner: VecDeque<FortunaInnerOrder>,
    last_inserted: u64,
}

impl Default for Fortuna {
    fn default() -> Self {
        Self::new()
    }
}

impl Fortuna {
    pub fn new() -> Self {
        Self {
            inner: std::collections::VecDeque::new(),
            last_inserted: 0,
        }
    }

    // Hyperliquid returns an error if we try to cancel a non-existent order
    pub fn delete_order(&mut self, asset: u64, order_id: u64) -> bool {
        let mut delete_position: Option<usize> = None;
        for (position, order) in self.inner.iter().enumerate() {
            if order_id == order.order_id && asset == order.order.asset {
                delete_position = Some(position);
                break;
            }
        }
        if let Some(position) = delete_position {
            self.inner.remove(position);
            return true;
        }
        false
    }

    // Hyperliquid immediately returns an oid to the user whether the order is resting or filled on
    // the next tick. Because we need to guard against lookahead bias, we cannot execute immediately
    // but we have to return order id here.
    pub fn insert_order(&mut self, order: FortunaOrder) -> FortunaOrderId {
        let order_id = self.last_inserted.clone();
        let inner_order = FortunaInnerOrder {
            order_id,
            order,
        };
        self.inner.push_back(inner_order);
        self.last_inserted += 1;
        order_id
    }

    pub fn execute_orders(&mut self, date: i64, source: &Penelope) -> Vec<u64> {
        unimplemented!("To implement")
    }

}