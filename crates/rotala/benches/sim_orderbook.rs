use criterion::{criterion_group, criterion_main, Criterion};

use rotala::exchange::uist::{UistV1, UistOrder};
use rotala::input::penelope::PenelopeBuilder;

fn uist_core_loop_test() {
    let mut source_builder = PenelopeBuilder::new();
    source_builder.add_quote(100.00, 101.00, 100, "ABC");
    source_builder.add_quote(10.00, 11.00, 100, "BCD");
    source_builder.add_quote(100.00, 101.00, 101, "ABC");
    source_builder.add_quote(10.00, 11.00, 101, "BCD");
    source_builder.add_quote(104.00, 105.00, 102, "ABC");
    source_builder.add_quote(10.00, 11.00, 102, "BCD");
    source_builder.add_quote(104.00, 105.00, 103, "ABC");
    source_builder.add_quote(12.00, 13.00, 103, "BCD");

    let (price_source, clock) = source_builder.build();
    let mut uist = UistV1::new(clock, price_source, "FAKE");

    uist.insert_order(UistOrder::market_buy("ABC", 100.0));
    uist.insert_order(UistOrder::market_buy("ABC", 100.0));

    uist.tick();
    uist.tick();
    uist.tick();
}

fn benchmarks(c: &mut Criterion) {
    c.bench_function("uist core loop", |b| b.iter(uist_core_loop_test));
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
