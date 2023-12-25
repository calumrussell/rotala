use crate::types::{CashValue, PortfolioQty, Price};
use rotala::clock::DateTime;
use rotala::exchange::uist::UistTrade;

#[allow(unused)]
use crate::types::PortfolioAllocation;

pub trait BrokerTrade {
    fn get_quantity(&self) -> f64;
    fn get_value(&self) -> f64;
}

impl BrokerTrade for UistTrade {
    fn get_quantity(&self) -> f64 {
        self.quantity
    }
    fn get_value(&self) -> f64 {
        self.value
    }
}

///Implementation of various cost models for brokers. Broker implementations would either define or
///cost model or would provide the user the option of intializing one; the broker impl would then
///call the variant's calculation methods as trades are executed.
#[derive(Clone, Debug)]
pub enum BrokerCost {
    PerShare(Price),
    PctOfValue(f64),
    Flat(CashValue),
}

impl BrokerCost {
    pub fn per_share(val: f64) -> Self {
        BrokerCost::PerShare(Price::from(val))
    }

    pub fn pct_of_value(val: f64) -> Self {
        BrokerCost::PctOfValue(val)
    }

    pub fn flat(val: f64) -> Self {
        BrokerCost::Flat(CashValue::from(val))
    }

    pub fn calc(&self, trade: impl BrokerTrade) -> CashValue {
        match self {
            BrokerCost::PerShare(cost) => CashValue::from(*cost.clone() * trade.get_quantity().clone()),
            BrokerCost::PctOfValue(pct) => CashValue::from(trade.get_value() * *pct),
            BrokerCost::Flat(val) => val.clone(),
        }
    }

    //Returns a valid trade given trading costs given a current budget
    //and price of security
    pub fn trade_impact(
        &self,
        gross_budget: &f64,
        gross_price: &f64,
        is_buy: bool,
    ) -> (CashValue, Price) {
        let mut net_budget = *gross_budget;
        let mut net_price = *gross_price;
        match self {
            BrokerCost::PerShare(val) => {
                if is_buy {
                    net_price += *val.clone();
                } else {
                    net_price -= *val.clone();
                }
            }
            BrokerCost::PctOfValue(pct) => {
                net_budget *= 1.0 - pct;
            }
            BrokerCost::Flat(val) => net_budget -= *val.clone(),
        }
        (CashValue::from(net_budget), Price::from(net_price))
    }

    pub fn trade_impact_total(
        trade_costs: &[BrokerCost],
        gross_budget: &f64,
        gross_price: &f64,
        is_buy: bool,
    ) -> (CashValue, Price) {
        let mut res = (CashValue::from(*gross_budget), Price::from(*gross_price));
        for cost in trade_costs {
            res = cost.trade_impact(&res.0, &res.1, is_buy);
        }
        res
    }
}
