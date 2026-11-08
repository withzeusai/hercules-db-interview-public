//! Stage 3 benchmark: three named queries against a full-scan baseline.
//!
//!     cargo bench -p bench --bench stage3
//!
//! Targets with 1M rows, relative to `scan_baseline`:
//! - `q1_open_for_customer`: at least 50x faster.
//! - `q2_latest_10`: at least 50x faster.
//! - `q3_customer_or_urgent`: no more than 1.5x slower.

use std::hint::black_box;
use std::time::Duration;

use bench::{TENANT, rows, stage3_queries, stage3_store};
use criterion::{Criterion, criterion_group, criterion_main};
use hercules_db::*;

fn stage3(c: &mut Criterion) {
    let n = rows();
    eprintln!("stage3: building {n} rows");
    let store = stage3_store(n);
    let queries = stage3_queries();

    for (name, q) in &queries {
        let plan = store.explain(TENANT, q);
        let rows = store.query(TENANT, q.clone()).map(|r| r.len());
        eprintln!("stage3: {name}: {rows:?} rows, plan {plan:?}");
    }

    let mut group = c.benchmark_group("stage3");
    group
        .sample_size(20)
        .measurement_time(Duration::from_secs(5));
    for (name, q) in queries {
        group.bench_function(name, |b| {
            b.iter(|| black_box(store.query(TENANT, q.clone()).unwrap()))
        });
    }
    group.finish();
}

criterion_group!(benches, stage3);
criterion_main!(benches);
