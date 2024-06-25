use rotala::exchange::jura_v1::{JuraV1, Order};
use rotala::input::penelope::Penelope;

#[test]
fn test_that_uist_works() {
    let source = Penelope::random(1000, vec!["0"]);
    let mut exchange = JuraV1::new();

    let order = Order::market_buy(0, "100.0", "97.00");
    exchange.insert_order(order);

    exchange.tick(source.get_quotes_unchecked(&100));
}
