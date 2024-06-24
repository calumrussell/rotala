use rotala::{exchange::uist_v1::{Order, UistV1}, input::penelope::Penelope};

#[test]
fn test_that_uist_works() {
    let source = Penelope::random(1000);
    let mut exchange = UistV1::new();

    let order = Order::market_buy("ABC", 100.0);
    exchange.insert_order(order);

    exchange.tick(source.get_quotes_unchecked(&100));
}
