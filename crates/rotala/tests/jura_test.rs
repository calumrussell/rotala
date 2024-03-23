use rotala::exchange::jura_v1::{random_jura_generator, Order};

#[test]
fn test_that_uist_works() {
    let (mut exchange, _clock) = random_jura_generator(1000);

    let _init = exchange.init();

    let order = Order::market_buy(0, "100.0", "97.00");
    exchange.insert_order(order);
}
