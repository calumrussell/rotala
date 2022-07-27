use crate::types::{CashValue, DateTime, PortfolioHoldings, PortfolioQty, PortfolioValues, Price};

pub mod record;
pub mod rules;

//Contains data structures and traits that refer solely to the data held and operations required
//for broker implementations.
#[derive(Clone, Debug)]
pub struct Quote {
    //TODO: more indirection is needed for this type, possibly implemented as trait
    pub bid: Price,
    pub ask: Price,
    pub date: DateTime,
    pub symbol: String,
}

#[derive(Clone, Debug)]
pub struct Dividend {
    //Dividend value is expressed in terms of per share values
    pub value: Price,
    pub symbol: String,
    pub date: DateTime,
}

#[derive(Clone, Debug)]
pub struct DividendPayment {
    pub value: CashValue,
    pub symbol: String,
    pub date: DateTime,
}

#[derive(Clone, Debug)]
pub enum TradeType {
    Buy,
    Sell,
}

#[derive(Clone, Debug)]
pub struct Trade {
    //TODO: more indirection is needed for this type, possibly implemented as trait
    pub symbol: String,
    pub value: CashValue,
    pub quantity: PortfolioQty,
    pub date: DateTime,
    pub typ: TradeType,
}

#[derive(Clone, Debug)]
pub enum BrokerEvent {
    TradeSuccess(Trade),
    TradeFailure(Order),
    OrderCreated(Order),
    OrderFailure(Order),
    WithdrawSuccess(CashValue),
    WithdrawFailure(CashValue),
    DepositSuccess(CashValue),
    //No value is returned for these variants because transactions are internal to broker
    TransactionSuccess,
    TransactionFailure,
}

#[derive(Clone, Debug)]
pub enum BrokerRecordedEvent {
    TradeCompleted(Trade),
    DividendPaid(DividendPayment),
}

impl From<Trade> for BrokerRecordedEvent {
    fn from(trade: Trade) -> Self {
        BrokerRecordedEvent::TradeCompleted(trade)
    }
}

impl From<DividendPayment> for BrokerRecordedEvent {
    fn from(divi: DividendPayment) -> Self {
        BrokerRecordedEvent::DividendPaid(divi)
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
    shares: PortfolioQty,
    price: Option<Price>,
}

impl Order {
    //TODO: should this be a trait?
    pub fn get_symbol(&self) -> String {
        self.symbol.clone()
    }

    pub fn get_shares(&self) -> PortfolioQty {
        self.shares
    }

    pub fn get_price(&self) -> Option<Price> {
        self.price
    }

    pub fn get_order_type(&self) -> OrderType {
        self.order_type
    }

    pub fn new(
        order_type: OrderType,
        symbol: String,
        shares: PortfolioQty,
        price: Option<Price>,
    ) -> Self {
        Order {
            order_type,
            symbol,
            shares,
            price,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BrokerCost {
    PerShare(Price),
    PctOfValue(f64),
    Flat(CashValue),
}

impl BrokerCost {
    pub fn calc(&self, trade: &Trade) -> CashValue {
        match self {
            BrokerCost::PerShare(cost) => trade.quantity * *cost,
            BrokerCost::PctOfValue(pct) => CashValue::from(f64::from(trade.value) * pct),
            BrokerCost::Flat(val) => *val,
        }
    }

    //Returns a valid trade given trading costs given a current budget
    //and price of security
    pub fn trade_impact(
        &self,
        gross_budget: &CashValue,
        gross_price: &Price,
        is_buy: bool,
    ) -> (CashValue, Price) {
        let mut net_budget = *gross_budget;
        let mut net_price = *gross_price;
        match self {
            BrokerCost::PerShare(val) => {
                if is_buy {
                    net_price += *val;
                } else {
                    net_price -= *val;
                }
            }
            BrokerCost::PctOfValue(pct) => {
                net_budget *= CashValue::from(1.0 - *pct);
            }
            BrokerCost::Flat(val) => net_budget -= *val,
        }
        (net_budget, net_price)
    }

    pub fn trade_impact_total(
        trade_costs: &Vec<BrokerCost>,
        gross_budget: &CashValue,
        gross_price: &Price,
        is_buy: bool,
    ) -> (CashValue, Price) {
        let mut res = (*gross_budget, *gross_price);
        for cost in trade_costs {
            res = cost.trade_impact(&res.0, &res.1, is_buy);
        }
        res
    }
}

//Key traits for broker implementations.
//
//Whilst broker is implemented within this package as a singular broker, the intention of these
//traits is to hide the implementation from the user so that it could be one or a combination of
//brokers returning the data. Similarly, strategy implementations should not create any
//dependencies on the underlying state of the broker.
pub trait TransferCash {
    fn withdraw_cash(&mut self, cash: CashValue) -> BrokerEvent;
    fn deposit_cash(&mut self, cash: CashValue) -> BrokerEvent;
    fn debit(&mut self, value: CashValue) -> BrokerEvent;
    fn credit(&mut self, value: CashValue) -> BrokerEvent;
    fn get_cash_balance(&self) -> CashValue;
}

pub trait PositionInfo {
    fn get_position_qty(&self, symbol: &str) -> Option<&PortfolioQty>;
    fn get_position_value(&self, symbol: &str) -> Option<CashValue>;
    fn get_position_cost(&self, symbol: &str) -> Option<Price>;
    fn get_position_liquidation_value(&self, symbol: &str) -> Option<CashValue>;
    fn get_position_profit(&self, symbol: &str) -> Option<CashValue>;
    fn get_liquidation_value(&self) -> CashValue;
    fn get_total_value(&self) -> CashValue;
    fn get_positions(&self) -> Vec<String>;
    fn get_values(&self) -> PortfolioValues;
    fn get_holdings(&self) -> PortfolioHoldings;
}

pub trait GetsQuote {
    fn get_quote(&self, symbol: &str) -> Option<Quote>;
    fn get_quotes(&self) -> Option<&Vec<Quote>>;
}

pub trait CanUpdate {
    fn update_holdings(&mut self, symbol: &str, change: &PortfolioQty);
}

pub trait PendingOrder {
    fn insert_order(&mut self, order: &Order);
    fn delete_order(&mut self, id: &u8);
}

pub trait ExecutesOrder {
    fn execute_order(&mut self, order: &Order) -> BrokerEvent;
    fn execute_orders(&mut self, orders: Vec<Order>) -> Vec<BrokerEvent>;
}

pub trait TradeCost {
    fn get_trade_costs(&self, trade: &Trade) -> CashValue;
    fn calc_trade_impact(
        &self,
        budget: &CashValue,
        price: &Price,
        is_buy: bool,
    ) -> (CashValue, Price);
}

pub trait PayDividend {
    fn pay_dividends(&mut self);
}

pub trait EventLog {
    fn trades_between(&self, start: &DateTime, end: &DateTime) -> Vec<Trade>;
    fn dividends_between(&self, start: &DateTime, end: &DateTime) -> Vec<DividendPayment>;
}

#[cfg(test)]
mod tests {

    use super::BrokerCost;

    #[test]
    fn can_estimate_trade_costs_of_proposed_trade() {
        let pershare = BrokerCost::PerShare(0.1.into());
        let flat = BrokerCost::Flat(10.0.into());
        let pct = BrokerCost::PctOfValue(0.01);

        let res = pershare.trade_impact(&1000.0.into(), &1.0.into(), true);
        assert!(res.1 == 1.1);

        let res = pershare.trade_impact(&1000.0.into(), &1.0.into(), false);
        assert!(res.1 == 0.9);

        let res = flat.trade_impact(&1000.0.into(), &1.0.into(), true);
        assert!(res.0 == 990.00);

        let res = pct.trade_impact(&100.0.into(), &1.0.into(), true);
        assert!(res.0 == 99.0);

        let costs = vec![pershare, flat];
        let initial = BrokerCost::trade_impact_total(&costs, &1000.0.into(), &1.0.into(), true);
        assert!(initial.0 == 990.00);
        assert!(initial.1 == 1.1);
    }
}
