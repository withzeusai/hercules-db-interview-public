//! Stage 2: secondary indexes.

use std::collections::BTreeSet;
use std::ops::Bound::{Included, Unbounded};

use harness::*;

const REGIONS: [&str; 3] = ["NA", "EU", "APAC"];

fn order_def() -> ObjectDef {
    ObjectDef::new(
        "Order",
        vec![
            FieldDef::new("k", FieldType::Integer).required(),
            FieldDef::new("customer", FieldType::Integer).required(),
            FieldDef::new("region", picklist(&REGIONS)).required(),
            FieldDef::new("sku", FieldType::Text { max_len: 16 }).required(),
            FieldDef::new("qty", FieldType::Integer).required(),
        ],
    )
}

/// Row `k` of the generated dataset.
fn order(k: i64) -> Fields {
    fields! {
        "k" => k,
        "customer" => k % 50,
        "region" => REGIONS[(k % 3) as usize],
        "sku" => format!("SKU-{}", k % 20),
        "qty" => k % 7,
    }
}

fn orders(t: TenantId, n: i64) -> HerculesDb {
    let s = store();
    s.define_object(t, order_def()).unwrap();
    for k in 0..n {
        s.insert(t, "Order", order(k)).unwrap();
    }
    s
}

/// The `k` values of rows matching `filter`.
fn ks(s: &HerculesDb, t: TenantId, filter: Filter) -> BTreeSet<i64> {
    let recs = s.query(t, Query::new("Order").filter(filter)).unwrap();
    assert_eq!(ids(&recs).len(), recs.len(), "duplicate records in result");
    int_set(&recs, "k")
}

fn expected(n: i64, pred: impl Fn(i64) -> bool) -> BTreeSet<i64> {
    (0..n).filter(|&k| pred(k)).collect()
}

#[test]
fn create_index_backfills_existing_records() {
    let s = orders(ACME, 600);
    let eq = Filter::eq("customer", 7);
    let range = Filter::range(
        "customer",
        Included(Value::Int(10)),
        Included(Value::Int(12)),
    );
    let before_eq = ks(&s, ACME, eq.clone());
    let before_range = ks(&s, ACME, range.clone());

    s.create_index(ACME, "Order", "customer").unwrap();

    assert_eq!(ks(&s, ACME, eq), before_eq);
    assert_eq!(ks(&s, ACME, range), before_range);
    assert_eq!(before_eq, expected(600, |k| k % 50 == 7));
    assert_eq!(
        before_range,
        expected(600, |k| (10..=12).contains(&(k % 50)))
    );
}

#[test]
fn create_index_rejects_unknown_object_or_field() {
    let s = orders(ACME, 10);
    assert_eq!(
        s.create_index(ACME, "Shipment", "k"),
        Err(Error::UnknownObject)
    );
    assert_eq!(
        s.create_index(ACME, "Order", "nope"),
        Err(Error::UnknownField("nope".into()))
    );
    assert_eq!(
        s.create_index(GLOBEX, "Order", "k"),
        Err(Error::UnknownObject),
        "indexes are per tenant"
    );
}

#[test]
fn indexed_eq_returns_matching_records() {
    let n = 900;
    let s = orders(ACME, n);
    for field in ["customer", "region", "sku"] {
        s.create_index(ACME, "Order", field).unwrap();
    }

    assert_eq!(
        ks(&s, ACME, Filter::eq("customer", 42)),
        expected(n, |k| k % 50 == 42)
    );
    assert_eq!(
        ks(&s, ACME, Filter::eq("region", "EU")),
        expected(n, |k| k % 3 == 1)
    );
    assert_eq!(
        ks(&s, ACME, Filter::eq("sku", "SKU-13")),
        expected(n, |k| k % 20 == 13)
    );
    assert!(ks(&s, ACME, Filter::eq("customer", 50)).is_empty());
    assert!(ks(&s, ACME, Filter::eq("sku", "SKU-99")).is_empty());
}

#[test]
fn indexed_range_queries() {
    let n = 700;
    let s = orders(ACME, n);
    s.create_index(ACME, "Order", "qty").unwrap();
    let int = |i: i64| Value::Int(i);

    assert_eq!(
        ks(
            &s,
            ACME,
            Filter::range("qty", Included(int(2)), Included(int(4)))
        ),
        expected(n, |k| (2..=4).contains(&(k % 7)))
    );
    assert_eq!(
        ks(&s, ACME, Filter::range("qty", Unbounded, Included(int(1)))),
        expected(n, |k| k % 7 <= 1)
    );
    assert_eq!(
        ks(&s, ACME, Filter::range("qty", Included(int(5)), Unbounded)),
        expected(n, |k| k % 7 >= 5)
    );
    assert_eq!(
        ks(
            &s,
            ACME,
            Filter::range("qty", Included(int(3)), Included(int(3)))
        ),
        expected(n, |k| k % 7 == 3)
    );
    assert_eq!(
        ks(&s, ACME, Filter::range("qty", Unbounded, Unbounded)).len(),
        n as usize
    );
}

#[test]
fn index_sees_inserts_after_creation() {
    let s = store();
    s.define_object(ACME, order_def()).unwrap();
    s.create_index(ACME, "Order", "customer").unwrap();
    for k in 0..100 {
        s.insert(ACME, "Order", order(k)).unwrap();
    }
    assert_eq!(ks(&s, ACME, Filter::eq("customer", 3)), [3, 53].into());
    for k in 100..200 {
        s.insert(ACME, "Order", order(k)).unwrap();
    }
    assert_eq!(
        ks(&s, ACME, Filter::eq("customer", 3)),
        [3, 53, 103, 153].into()
    );
}

#[test]
fn index_sees_updated_values() {
    let s = store();
    s.define_object(ACME, order_def()).unwrap();
    let ids: Vec<RecordId> = (0..100)
        .map(|k| s.insert(ACME, "Order", order(k)).unwrap())
        .collect();
    s.create_index(ACME, "Order", "customer").unwrap();
    s.create_index(ACME, "Order", "sku").unwrap();

    s.update(
        ACME,
        "Order",
        ids[3],
        fields! { "customer" => 49, "sku" => "SKU-NEW" },
    )
    .unwrap();

    assert_eq!(ks(&s, ACME, Filter::eq("customer", 49)), [3, 49, 99].into());
    assert_eq!(ks(&s, ACME, Filter::eq("sku", "SKU-NEW")), [3].into());
}

#[test]
fn indexes_are_scoped_per_tenant() {
    let s = store();
    for t in [ACME, GLOBEX] {
        s.define_object(t, order_def()).unwrap();
    }
    // ACME gets even k, GLOBEX gets odd k.
    for k in 0..400 {
        let t = if k % 2 == 0 { ACME } else { GLOBEX };
        s.insert(t, "Order", order(k)).unwrap();
    }
    let acme_7 = expected(400, |k| k % 2 == 0 && k % 50 == 7);
    let globex_7 = expected(400, |k| k % 2 == 1 && k % 50 == 7);
    assert!(acme_7.is_empty() && !globex_7.is_empty());

    s.create_index(ACME, "Order", "customer").unwrap();
    assert_eq!(ks(&s, ACME, Filter::eq("customer", 7)), acme_7);
    assert_eq!(ks(&s, GLOBEX, Filter::eq("customer", 7)), globex_7);

    s.create_index(GLOBEX, "Order", "customer").unwrap();
    s.insert(ACME, "Order", order(1007)).unwrap();
    assert_eq!(ks(&s, ACME, Filter::eq("customer", 7)), [1007].into());
    assert_eq!(ks(&s, GLOBEX, Filter::eq("customer", 7)), globex_7);
}

#[test]
fn indexed_filters_compose_with_order_and_limit() {
    let n = 1000;
    let s = orders(ACME, n);
    s.create_index(ACME, "Order", "customer").unwrap();

    let and = Filter::and([Filter::eq("customer", 7), Filter::eq("region", "EU")]);
    assert_eq!(
        ks(&s, ACME, and),
        expected(n, |k| k % 50 == 7 && k % 3 == 1)
    );

    let recs = s
        .query(
            ACME,
            Query::new("Order")
                .filter(Filter::eq("customer", 7))
                .order_by("k", Dir::Desc)
                .limit(3),
        )
        .unwrap();
    assert_eq!(ints(&recs, "k"), [957, 907, 857]);
}
