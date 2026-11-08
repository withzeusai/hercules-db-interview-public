//! Datasets for the Stage 2 and Stage 3 benchmarks.
//!
//! Row count defaults to 1,000,000. Set `HERCULES_DB_BENCH_ROWS` to change it,
//! e.g. `HERCULES_DB_BENCH_ROWS=100000 cargo bench -p bench` while iterating.

use std::time::Instant;

use hercules_db::*;

pub const TENANT: TenantId = TenantId(1);

pub fn rows() -> i64 {
    std::env::var("HERCULES_DB_BENCH_ROWS")
        .ok()
        .and_then(|v| v.replace('_', "").parse().ok())
        .unwrap_or(1_000_000)
}

/// Stage 2: `Event` rows where `bucket` and `bucket_copy` hold the same value,
/// with 1,000 distinct values (0.1% selectivity). Only `bucket` is indexed.
pub fn stage2_store(n: i64) -> HerculesDb {
    let s = HerculesDb::new();
    s.define_object(
        TENANT,
        ObjectDef::new(
            "Event",
            vec![
                FieldDef::new("k", FieldType::Integer).required(),
                FieldDef::new("bucket", FieldType::Integer).required(),
                FieldDef::new("bucket_copy", FieldType::Integer).required(),
                FieldDef::new("label", FieldType::Text { max_len: 32 }),
            ],
        ),
    )
    .unwrap();
    timed(&format!("insert {n} events"), || {
        for k in 0..n {
            let b = scatter(k) % 1000;
            s.insert(
                TENANT,
                "Event",
                fields! { "k" => k, "bucket" => b, "bucket_copy" => b, "label" => "event" },
            )
            .unwrap();
        }
    });
    timed("index bucket", || {
        s.create_index(TENANT, "Event", "bucket").unwrap()
    });
    s
}

/// The customer that Stage 3's queries look for.
pub const STAGE3_CUSTOMER: i64 = 4242;

/// Stage 3: `Ticket` rows.
///
/// - `status`: 90% Open, 5% Pending, 5% Closed. Indexed (first).
/// - `customer_id`: 10,000 customers, so each matches 0.01% of rows. Indexed.
/// - `created_at`: unique and increasing. Indexed.
/// - `notes`: "urgent" on 0.1% of rows, otherwise empty. Not indexed.
pub fn stage3_store(n: i64) -> HerculesDb {
    let s = HerculesDb::new();
    s.define_object(
        TENANT,
        ObjectDef::new(
            "Ticket",
            vec![
                FieldDef::new(
                    "status",
                    FieldType::Picklist(vec!["Open".into(), "Pending".into(), "Closed".into()]),
                )
                .required(),
                FieldDef::new("customer_id", FieldType::Integer).required(),
                FieldDef::new("created_at", FieldType::Integer).required(),
                FieldDef::new("notes", FieldType::Text { max_len: 64 }),
            ],
        ),
    )
    .unwrap();
    timed(&format!("insert {n} tickets"), || {
        for i in 0..n {
            let r = scatter(i);
            let status = match r % 20 {
                0 => "Pending",
                1 => "Closed",
                _ => "Open",
            };
            let notes = if (r / 20) % 1000 == 7 { "urgent" } else { "" };
            s.insert(
                TENANT,
                "Ticket",
                fields! {
                    "status" => status,
                    "customer_id" => (r / 20_000) % 10_000,
                    "created_at" => i,
                    "notes" => notes,
                },
            )
            .unwrap();
        }
    });
    for field in ["status", "customer_id", "created_at"] {
        timed(&format!("index {field}"), || {
            s.create_index(TENANT, "Ticket", field).unwrap()
        });
    }
    s
}

/// The three named Stage 3 queries, plus a full-scan baseline.
pub fn stage3_queries() -> Vec<(&'static str, Query)> {
    vec![
        (
            "scan_baseline",
            Query::new("Ticket").filter(Filter::eq("notes", "no such note")),
        ),
        (
            "q1_open_for_customer",
            Query::new("Ticket").filter(Filter::and([
                Filter::eq("status", "Open"),
                Filter::eq("customer_id", STAGE3_CUSTOMER),
            ])),
        ),
        (
            "q2_latest_10",
            Query::new("Ticket")
                .order_by("created_at", Dir::Desc)
                .limit(10),
        ),
        (
            "q3_customer_or_urgent",
            Query::new("Ticket").filter(Filter::or([
                Filter::eq("customer_id", STAGE3_CUSTOMER),
                Filter::eq("notes", "urgent"),
            ])),
        ),
    ]
}

/// A cheap deterministic hash so columns aren't correlated with insert order.
pub fn scatter(i: i64) -> i64 {
    let mut x = i as u64 ^ 0x5DEE_CE66_D1CE_4E5B;
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 1) as i64
}

fn timed<R>(what: &str, f: impl FnOnce() -> R) -> R {
    let start = Instant::now();
    let r = f();
    eprintln!("  {what}: {:.2?}", start.elapsed());
    r
}
