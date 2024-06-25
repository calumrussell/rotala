use alator::broker::uist::UistBrokerBuilder;
use alator::broker::{BrokerCost, CashOperations, SendOrder, Update};
use criterion::{criterion_group, criterion_main, Criterion};
use std::collections::HashMap;

use alator::strategy::staticweight::StaticWeightStrategyBuilder;
use rotala::http::uist::uistv1_client::{TestClient, UistClient};
use rotala::input::penelope::Penelope;

async fn full_backtest_random_data() {
    let source = Penelope::random(100, vec!["ABC", "BCD"]);

    let initial_cash = 100_000.0;

    let mut weights = HashMap::new();
    weights.insert("ABC".to_string(), 0.5);
    weights.insert("BCD".to_string(), 0.5);

    let mut client = TestClient::single("Random", source);
    let resp = client.init("Random".to_string()).await.unwrap();

    let simbrkr = UistBrokerBuilder::new()
        .with_client(client, resp.backtest_id)
        .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
        .build()
        .await;

    let mut strat = StaticWeightStrategyBuilder::new()
        .with_brkr(simbrkr)
        .with_weights(weights)
        .default();

    strat.init(&initial_cash);
    strat.run().await;
}

async fn trade_execution_logic() {
    let mut source = Penelope::new();
    source.add_quote(100.00, 101.00, 100, "ABC");
    source.add_quote(10.00, 11.00, 100, "BCD");
    source.add_quote(100.00, 101.00, 101, "ABC");
    source.add_quote(10.00, 11.00, 101, "BCD");
    source.add_quote(104.00, 105.00, 102, "ABC");
    source.add_quote(10.00, 11.00, 102, "BCD");
    source.add_quote(104.00, 105.00, 103, "ABC");
    source.add_quote(12.00, 13.00, 103, "BCD");

    let mut client = TestClient::single("Random", source);
    let resp = client.init("Random".to_string()).await.unwrap();

    let mut brkr = UistBrokerBuilder::new()
        .with_client(client, resp.backtest_id)
        .build()
        .await;

    brkr.deposit_cash(&100_000.0);
    brkr.send_order(rotala::exchange::uist_v1::Order::market_buy("ABC", 100.0));
    brkr.send_order(rotala::exchange::uist_v1::Order::market_buy("BCD", 100.0));

    brkr.check().await;

    brkr.check().await;

    brkr.check().await;
}

fn benchmarks(c: &mut Criterion) {
    c.bench_function("full backtest", |b| b.iter(full_backtest_random_data));
    c.bench_function("trade test", |b| b.iter(trade_execution_logic));
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
