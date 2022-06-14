use std::collections::HashMap;

pub mod record;
pub mod rules;

#[derive(Clone, Debug)]
pub struct Quote {
    pub bid: f64,
    pub ask: f64,
    pub date: i64,
    pub symbol: String,
}

#[derive(Clone, Debug)]
pub struct Dividend {
    pub value: f64,
    pub symbol: String,
    pub date: i64,
}

#[derive(Clone, Debug)]
pub struct Trade {
    pub symbol: String,
    pub value: f64,
    pub quantity: f64,
    pub date: i64,
}

#[derive(Clone, Debug)]
pub enum BrokerEvent {
    TradeSuccess(Trade),
    TradeFailure(Order),
    OrderCreated(Order),
    OrderFailure(Order),
    WithdrawSuccess(u64),
    WithdrawFailure(u64),
    DepositSuccess(u64),
    //No value is returned to client because transactions are internal to
    //the broker
    TransactionSuccess,
    TransactionFailure,
}

#[derive(Clone, Debug)]
pub enum BrokerRecordedEvents {
    TradeCompleted(Trade),
    DividendPaid(Dividend),
}

impl From<Trade> for BrokerRecordedEvents {
    fn from(trade: Trade) -> Self {
        BrokerRecordedEvents::TradeCompleted(trade)
    }
}

impl From<&Trade> for BrokerRecordedEvents {
    fn from(trade: &Trade) -> Self {
        BrokerRecordedEvents::TradeCompleted(trade.clone())
    }
}

impl From<Dividend> for BrokerRecordedEvents {
    fn from(dividend: Dividend) -> Self {
        BrokerRecordedEvents::DividendPaid(dividend)
    }
}

impl From<&Dividend> for BrokerRecordedEvents {
    fn from(dividend: &Dividend) -> Self {
        BrokerRecordedEvents::DividendPaid(dividend.clone())
    }
}

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

    pub fn new(order_type: OrderType, symbol: String, shares: f64, price: Option<f64>) -> Self {
        Order {
            order_type,
            symbol,
            shares,
            price,
        }
    }
}

#[derive(Clone)]
pub enum BrokerCost {
    PerShare(f64),
    PctOfValue(f64),
    Flat(f64),
}

impl BrokerCost {
    pub fn calc(&self, trade: &Trade) -> f64 {
        match self {
            BrokerCost::PerShare(cost) => trade.quantity * cost,
            BrokerCost::PctOfValue(pct) => trade.value * pct,
            BrokerCost::Flat(val) => *val,
        }
    }

    //Returns a valid trade given trading costs given a current budget
    //and price of security
    pub fn trade_impact(&self, gross_budget: &f64, gross_price: &f64, is_buy: bool) -> (f64, f64) {
        let mut net_budget = gross_budget.clone();
        let mut net_price = gross_price.clone();
        match self {
            BrokerCost::PerShare(val) => {
                if is_buy {
                    net_price += val;
                } else {
                    net_price -= val;
                }
            }
            BrokerCost::PctOfValue(pct) => {
                net_budget = net_budget * (1.0 - pct);
            }
            BrokerCost::Flat(val) => net_budget -= val,
        }
        (net_budget, net_price)
    }

    pub fn trade_impact_total(
        trade_costs: &Vec<BrokerCost>,
        gross_budget: &f64,
        gross_price: &f64,
        is_buy: bool,
    ) -> (f64, f64) {
        let mut res = (gross_budget.clone(), gross_price.clone());
        for cost in trade_costs {
            res = cost.trade_impact(&res.0, &res.1, is_buy);
        }
        res
    }
}

pub trait CashManager {
    fn withdraw_cash(&mut self, cash: u64) -> BrokerEvent;
    fn deposit_cash(&mut self, cash: u64) -> BrokerEvent;
    fn debit(&mut self, value: u64) -> BrokerEvent;
    fn credit(&mut self, value: u64) -> BrokerEvent;
    fn get_cash_balance(&self) -> u64;
}

pub trait PositionInfo {
    fn get_position_qty(&self, symbol: &String) -> Option<f64>;
    fn get_position_value(&self, symbol: &String) -> Option<f64>;
    fn get_position_cost(&self, symbol: &String) -> Option<f64>;
    fn get_position_liquidation_value(&self, symbol: &String) -> Option<f64>;
    fn get_position_profit(&self, symbol: &String) -> Option<f64>;
}

pub trait PriceQuote {
    fn get_quote(&self, symbol: &String) -> Option<Quote>;
    fn get_quotes(&self) -> Option<Vec<Quote>>;
}

pub trait ClientControlled {
    fn get_positions(&self) -> Vec<String>;
    fn update_holdings(&mut self, symbol: &String, change: &f64);
    fn get_holdings(&self) -> HashMap<String, f64>;
    fn get(&self, symbol: &String) -> Option<&f64>;
}

pub trait PendingOrders {
    fn insert_order(&mut self, order: &Order);
    fn delete_order(&mut self, id: &u8);
}

pub trait OrderExecutor {
    fn execute_order(&mut self, order: &Order) -> BrokerEvent;
    fn execute_orders(&mut self, orders: Vec<Order>) -> Vec<BrokerEvent>;
}

pub trait TradeCosts {
    fn get_trade_costs(&self, trade: &Trade) -> f64;
    fn calc_trade_impact(&self, budget: &f64, price: &f64, is_buy: bool) -> (f64, f64);
}

pub trait PaysDividends {
    fn pay_dividends(&mut self);
}

pub trait HasTime {
    fn now(&self) -> i64;
}

pub trait FindTrades {
    fn trades_between(&self, start: &i64, end: &i64) -> Vec<Trade>;
}

#[cfg(test)]
mod tests {

    use super::BrokerCost;

    #[test]
    fn can_estimate_trade_costs_of_proposed_trade() {
        let pershare = BrokerCost::PerShare(0.1);
        let flat = BrokerCost::Flat(10.0);
        let pct = BrokerCost::PctOfValue(0.01);

        let res = pershare.trade_impact(&1000.0, &1.0, true);
        assert!(res.1 == 1.1);

        let res = pershare.trade_impact(&1000.0, &1.0, false);
        assert!(res.1 == 0.9);

        let res = flat.trade_impact(&1000.0, &1.0, true);
        assert!(res.0 == 990.00);

        let res = pct.trade_impact(&100.0, &1.0, true);
        assert!(res.0 == 99.0);

        let costs = vec![pershare, flat];
        let initial = BrokerCost::trade_impact_total(&costs, &1000.0, &1.0, true);
        assert!(initial.0 == 990.00);
        assert!(initial.1 == 1.1);
    }
}
