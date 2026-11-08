//! Stage 2 benchmark: indexed Eq versus a full scan at 0.1% selectivity.
//!
//!     cargo bench -p bench --bench stage2
//!
//! Target on a laptop with 1M rows: `eq_indexed` p50 under 1 ms, versus tens of
//! ms for `eq_scan`.

use std::hint::black_box;
use std::time::Duration;

use bench::{TENANT, rows, stage2_store};
use criterion::{Criterion, criterion_group, criterion_main};
use hercules_db::*;

fn stage2(c: &mut Criterion) {
    let n = rows();
    eprintln!("stage2: building {n} rows");
    let store = stage2_store(n);

    let q = |field: &str, b: i64| Query::new("Event").filter(Filter::eq(field, b));
    let indexed = store.query(TENANT, q("bucket", 7)).unwrap().len();
    let scanned = store.query(TENANT, q("bucket_copy", 7)).unwrap().len();
    eprintln!("stage2: bucket = 7 matches {indexed} rows (indexed) / {scanned} rows (scan)");

    let mut group = c.benchmark_group("stage2");
    group
        .sample_size(20)
        .measurement_time(Duration::from_secs(5));
    let mut b = 0;
    group.bench_function("eq_indexed", |bench| {
        bench.iter(|| {
            b = (b + 1) % 1000;
            black_box(store.query(TENANT, q("bucket", b)).unwrap())
        })
    });
    group.bench_function("eq_scan", |bench| {
        bench.iter(|| {
            b = (b + 1) % 1000;
            black_box(store.query(TENANT, q("bucket_copy", b)).unwrap())
        })
    });
    group.finish();
}

criterion_group!(benches, stage2);
criterion_main!(benches);
