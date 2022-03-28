use rand::distributions::Uniform;
use rand::{thread_rng, Rng};
use rand_distr::{Distribution, Normal};
use std::ops::Range;

use alator::broker::Quote;

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