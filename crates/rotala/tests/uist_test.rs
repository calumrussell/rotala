use rotala::exchange::uist::{Uist, UistOrder};
use rotala::input::penelope::random_penelope_generator;

#[test]
fn test_that_uist_works() {
    let (penelope, clock) = random_penelope_generator(100);
    let mut exchange = Uist::new(clock, penelope);

    let _init = exchange.init();

    let order = UistOrder::market_buy("ABC", 100.0);
    exchange.insert_order(order);
}
