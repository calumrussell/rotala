use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::rc::Rc;

use alator::broker::{BacktestBroker, GetsQuote, Order, OrderType, Quote, TransferCash};
use alator::clock::{Clock, ClockBuilder};
use alator::exchange::DefaultExchangeBuilder;
use alator::input::{HashMapInput, HashMapInputBuilder, QuotesHashMap};
use alator::sim::{SimulatedBroker, SimulatedBrokerBuilder};
use alator::simcontext::SimContextBuilder;
use alator::strategy::{History, Strategy, StrategyEvent, TransferTo};
use alator::types::{CashValue, Frequency, StrategySnapshot};

/* Get the data from Binance, build quote from open and close of candle, insert the quotes into
 * QuotesHashMap using those dates.
 * We also need to work out the start and end of the simulation to initialise the clock with
 */
fn build_data() -> (QuotesHashMap, (i64, i64)) {
    let url =
        "https://data.binance.vision/data/spot/daily/klines/BTCUSDT/1m/BTCUSDT-1m-2022-08-03.zip";
    let mut quotes: QuotesHashMap = HashMap::new();
    let mut min_date = i64::MAX;
    let mut max_date = i64::MIN;
    if let Ok(resp) = reqwest::blocking::get(url) {
        if let Ok(contents) = resp.bytes() {
            let mut c = Cursor::new(Vec::new());
            let _res = c.write(&contents);

            if let Ok(mut zip) = zip::ZipArchive::new(c) {
                for i in 0..zip.len() {
                    if let Ok(mut zip_file) = zip.by_index(i) {
                        let mut rdr = csv::Reader::from_reader(&mut zip_file);
                        for result in rdr.records() {
                            if let Ok(row) = result {
                                /*
                                 * Binance data format:
                                 * 1607444700000,          // Open time
                                 * "18879.99",             // Open
                                 * "18900.00",             // High
                                 * "18878.98",             // Low
                                 * "18896.13",             // Close (or latest price)
                                 * "492.363",              // Volume
                                 * 1607444759999,          // Close time
                                 * "9302145.66080",        // Quote asset volume
                                 * 1874,                   // Number of trades
                                 * "385.983",              // Taker buy volume
                                 * "7292402.33267",        // Taker buy quote asset volume
                                 * "0"                     // Ignore.
                                 */
                                let open_date = (row[0].parse::<i64>().unwrap()) / 1000;
                                if open_date < min_date {
                                    min_date = open_date;
                                }
                                let quote = Quote {
                                    bid: row[1].parse::<f64>().unwrap().into(),
                                    ask: row[1].parse::<f64>().unwrap().into(),
                                    date: open_date.into(),
                                    symbol: "BTC".into(),
                                };
                                quotes.insert(open_date.into(), vec![quote]);
                                let close_date = (row[6].parse::<i64>().unwrap()) / 1000;
                                if close_date > max_date {
                                    max_date = close_date;
                                }
                                let quote1 = Quote {
                                    bid: row[4].parse::<f64>().unwrap().into(),
                                    ask: row[4].parse::<f64>().unwrap().into(),
                                    date: close_date.into(),
                                    symbol: "BTC".into(),
                                };
                                quotes.insert(close_date.into(), vec![quote1]);
                            }
                        }
                    }
                }
            }
        }
    }
    (quotes, (min_date, max_date))
}

//This is a simple moving average data structure used to generate a trading signal.
#[derive(Clone)]
struct MovingAverage {
    max: usize,
    data: Vec<f64>,
}

impl MovingAverage {
    pub fn avg(&self) -> f64 {
        let mut sum = 0.0;
        for price in &self.data {
            sum += price;
        }
        sum / (self.data.len() as f64)
    }

    pub fn full(&self) -> bool {
        self.data.len() == self.max
    }

    pub fn update(&mut self, quote: &Quote) {
        if self.full() {
            let (_first, values) = self.data.split_first().unwrap();
            let mut without_first = values.to_vec();
            without_first.push(f64::from(quote.ask.clone()));
            self.data = without_first;
        } else {
            self.data.push(f64::from(quote.ask.clone()));
        }
    }

    pub fn new(max: usize) -> Self {
        Self {
            max,
            data: Vec::new(),
        }
    }
}

#[derive(Clone)]
//Our strategy needs a reference to a Broker, and the broker needs a data source that implements
//the DataSource trait. We are using the default HashMapInput, but it is possible to create your
//own source.
//
//Note that we are populating `MovingAverage` with data from the broker, as this strategy just
//relies on prices. If we had a dependency on some source of data that wasn't price then we would
//need to make sure that the additional data source takes a shared reference to Clock, to keep
//everything in time.
//
//We also do not implement performance tracking. `StaticWeightStrategy` shows you how to hook this
//tracking into the simulation lifecycle.
struct MovingAverageStrategy {
    clock: Clock,
    brkr: SimulatedBroker<HashMapInput>,
    ten: MovingAverage,
    fifty: MovingAverage,
    history: Vec<StrategySnapshot>,
}

impl TransferTo for MovingAverageStrategy {
    fn deposit_cash(&mut self, cash: &f64) -> StrategyEvent {
        self.brkr.deposit_cash(&cash);
        StrategyEvent::DepositSuccess(CashValue::from(*cash))
    }
}

impl History for MovingAverageStrategy {
    fn get_history(&self) -> Vec<alator::types::StrategySnapshot> {
        self.history.clone()
    }
}

impl Strategy for MovingAverageStrategy {
    fn init(&mut self, initial_cash: &f64) {
        self.deposit_cash(initial_cash);
    }

    fn update(&mut self) -> CashValue {
        //If you need to use dividends or place non-market orders then we need to call:
        //self.brkr.check(); somewhere here. We don't use these features so this call is
        //excluded.

        //The simulation does not run at the same frequency as the strategy, we are only trading
        //when we have information from our data source.
        //
        //This is possible because the underlying representation of data here is just a Quote, and
        //means that we can take full advantage of the greater flexibility of the event-driven
        //backtesting system with regard to taking any kind of frequency of input.
        //
        //However, if we are implementing some kind of trading strategy then the periodicity of the
        //underlying quotes may be relevant. For example, we are currently using 1m data but if we
        //added 5m data then that would impact our calculation of moving averages (which is
        //dependent on candlesticks implicitly, even if our simulation is not).
        //
        //Errors should be less frequent with this approach but if we attempt to trade a symbol for
        //which there is no quote at that time then the system will panic and exit. This behaviour
        //may change in the future but as this implies that the strategy is in some kind of
        //incorrect state, a runtime failure seems most appropriate.
        if let Some(quote) = self.brkr.get_quote("BTC") {
            //Update our moving averages with the latest quote
            self.ten.update(&quote);
            self.fifty.update(&quote);

            //If we are at the start of the simulation and don't have full data for each moving
            //average then don't trade
            if !self.ten.full() || !self.fifty.full() {
                return self.brkr.get_total_value();
            }

            //If the 10 period MA is above the 50 period then we go long with 10% of our portfolio.
            //If the 10 period MA is below the 50 period then we sell any position that we have.
            //
            //At the moment, the broker can't go short and long. This is a feature that may be
            //added in the future but it adds dependencies on the underlying asset which is not
            //ideal currently.
            if self.ten.avg() > self.fifty.avg() {
                if let None = self.brkr.get_position_qty("BTC") {
                    let value = self.brkr.get_liquidation_value();
                    let pct_value = CashValue::from(*value * 0.1);
                    //All this casting is required because, at the moment, we haven't moved fully
                    //away from positions reqpresented in whole numbers. Strategies should work but
                    //I am not sure if the result is correct.
                    let qty = (f64::from(pct_value) / f64::from(quote.ask)).floor();
                    let order = Order::market(OrderType::MarketBuy, "BTC", qty.clone());
                    self.brkr.send_order(order);
                }
            } else {
                if let Some(qty) = self.brkr.get_position_qty("BTC") {
                    let order = Order::market(OrderType::MarketSell, "BTC", qty.clone());
                    self.brkr.send_order(order);
                }
            }
        }

        let val = self.brkr.get_total_value();

        let snap = StrategySnapshot {
            date: self.clock.borrow().now(),
            portfolio_value: val.clone(),
            net_cash_flow: CashValue::from(0.0),
        };

        self.history.push(snap);
        val
    }
}

impl MovingAverageStrategy {
    fn new(brkr: SimulatedBroker<HashMapInput>, clock: Clock) -> Self {
        let ten = MovingAverage::new(10);
        let fifty = MovingAverage::new(50);
        let history = Vec::new();
        Self {
            brkr,
            ten,
            fifty,
            clock,
            history,
        }
    }
}

#[test]
fn binance_test() {
    //If you past RUST_LOG=info and fail this test then you will be able to see what each component
    //of the system is doing.
    env_logger::init();

    /* The components of a backtest should use the minimum amount of shared state as possible:
     * here, only Clock is shared and each component is passed into and owned by the next stage in
     * the simulation up until the actual sim context: Clock>DataSource>Broker>Strategy>SimContext.
     *
     * Compared to libraries that use messages to communicate between components, this is quite
     * brittle. However, we gain simplicity about how the components interact, and ownership.
     * Messaging systems are far more scalable horizontally but this library is just intended to
     * run backtests, that is it so scalability really isn't a huge consideration.
     */
    let (quotes, dates) = build_data();
    //Clock is the only shared reference between backtesting components: it keeps the time inside
    //the simulation and should guard against the accidental use of future data.
    let clock = ClockBuilder::with_length_in_dates(dates.0, dates.1)
        .with_frequency(&Frequency::Second)
        .build();

    let data = HashMapInputBuilder::new()
        .with_clock(Rc::clone(&clock))
        .with_quotes(quotes)
        .build();

    let exchange = DefaultExchangeBuilder::new()
        .with_clock(Rc::clone(&clock))
        .with_data_source(data.clone())
        .build();

    let simbrkr = SimulatedBrokerBuilder::new()
        .with_data(data)
        .with_exchange(exchange)
        .build();

    let strat = MovingAverageStrategy::new(simbrkr, Rc::clone(&clock));

    let mut sim = SimContextBuilder::new()
        .with_clock(Rc::clone(&clock))
        .with_strategy(strat)
        .init(&1_000_000.0.into());

    sim.run();
}
