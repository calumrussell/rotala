use rand::distributions::Uniform;
use rand::{thread_rng, Rng};
use rand_distr::{Distribution, Normal};
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use alator::broker::sim::SimulatedBroker;
use alator::broker::Quote;
use alator::data::universe::{DefinedUniverse, StaticUniverse};
use alator::data::{DataSourceSim, DefaultDataSource};

pub fn build_fake_quote_stream(
    stock: &String,
    price_dist: Uniform<f64>,
    vol_dist: Uniform<f64>,
    range: Range<i64>,
    step: Option<usize>,
) -> Vec<Quote> {
    let mut rng = thread_rng();
    let price = rng.sample(price_dist);
    let vol = rng.sample(vol_dist);
    let ret_dist = Normal::new(0.0, vol).unwrap();
    let mut quotes: Vec<Quote> = Vec::new();

    let mut range_step = 1;
    if step.is_some() {
        range_step = step.unwrap();
    }

    for date in range.step_by(range_step) {
        let period_ret = ret_dist.sample(&mut rng);
        let new_price = price * (1.0 + period_ret);

        let q = Quote {
            symbol: stock.clone(),
            date: date,
            bid: new_price * 0.995,
            ask: new_price,
        };

        quotes.push(q);
    }
    quotes
}

pub fn build_data(universe: &StaticUniverse) -> HashMap<i64, Vec<Quote>> {
    let price_dist = Uniform::new(1.0, 100.0);
    let vol_dist = Uniform::new(0.01, 0.2);

    let start_date = 100;
    let end_date = 1000;

    let mut res: HashMap<i64, Vec<Quote>> = HashMap::new();

    for stock in universe.get_assets() {
        let quotes =
            build_fake_quote_stream(stock, price_dist, vol_dist, start_date..end_date, None);
        for quote in quotes {
            let date = quote.date;
            if res.contains_key(&date) {
                let mut current = res.get(&date).unwrap().to_owned();
                current.push(quote);
                res.insert(date, current);
            } else {
                res.insert(date, vec![quote]);
            }
        }
    }
    res
}

pub fn get_universe_weights() -> (Rc<StaticUniverse>, HashMap<String, f64>) {
    let uni = Rc::new(StaticUniverse::new(vec![
        "ABC", "BCD", "CDE", "DEF", "EFG", "FGH", "GHI", "HIJ", "IJK", "JKL", "KLM", "LMN", "MNO",
        "NOP",
    ]));

    let psize = 1.0 / uni.get_assets().len() as f64;
    let mut weights: HashMap<String, f64> = HashMap::new();
    for a in uni.get_assets() {
        weights.insert(a.clone(), psize);
    }
    (uni, weights)
}

pub fn build_fake_data() -> (SimulatedBroker<DefaultDataSource>, Rc<StaticUniverse>) {
    let mut raw_data: HashMap<i64, Vec<Quote>> = HashMap::new();

    let quote = Quote {
        symbol: String::from("ABC"),
        date: 100,
        bid: 101.0,
        ask: 102.0,
    };

    let quote1 = Quote {
        symbol: String::from("ABC"),
        date: 101,
        bid: 102.0,
        ask: 103.0,
    };

    let quote2 = Quote {
        symbol: String::from("BCD"),
        date: 100,
        bid: 501.0,
        ask: 502.0,
    };

    let quote3 = Quote {
        symbol: String::from("BCD"),
        date: 101,
        bid: 503.0,
        ask: 504.0,
    };

    raw_data.insert(100, vec![quote, quote2]);
    raw_data.insert(101, vec![quote1, quote3]);

    let source: DataSourceSim<DefaultDataSource> =
        DataSourceSim::<DefaultDataSource>::from_hashmap(raw_data);
    let sb = SimulatedBroker::new(source);
    let universe = Rc::new(StaticUniverse::new(vec!["ABC", "BCD"]));

    (sb, universe)
}
