use rotala::exchange::uist::{UistOrder, random_uist_generator};

#[test]
fn test_that_uist_works() {
    let mut exchange = random_uist_generator(1000);

    let _init = exchange.init();

    let order = UistOrder::market_buy("ABC", 100.0);
    exchange.insert_order(order);
}
