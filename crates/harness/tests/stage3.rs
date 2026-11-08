//! Stage 3: query planner.

use std::collections::BTreeSet;
use std::ops::Bound::Included;

use harness::*;

const N: i64 = 10_000;
const CUSTOMERS: i64 = 500;
const X: i64 = 123;

fn ticket_def() -> ObjectDef {
    ObjectDef::new(
        "Ticket",
        vec![
            FieldDef::new("status", picklist(&["Open", "Pending", "Closed"])).required(),
            FieldDef::new("customer_id", FieldType::Integer).required(),
            FieldDef::new("created_at", FieldType::Integer).required(),
            FieldDef::new("notes", FieldType::Text { max_len: 40 }),
        ],
    )
}

/// Status of row `i`: about 1% Closed, 10% Pending, the rest Open.
fn status(i: i64) -> &'static str {
    if i % 100 == 0 {
        "Closed"
    } else if i % 10 == 5 {
        "Pending"
    } else {
        "Open"
    }
}

fn notes(i: i64) -> &'static str {
    if i % 500 == 3 { "urgent" } else { "" }
}

/// `N` tickets: `created_at` is unique, each customer has 0.2% of rows.
fn tickets() -> HerculesDb {
    let s = store();
    s.define_object(ACME, ticket_def()).unwrap();
    for i in 0..N {
        s.insert(
            ACME,
            "Ticket",
            fields! {
                "status" => status(i),
                "customer_id" => i % CUSTOMERS,
                "created_at" => i,
                "notes" => notes(i),
            },
        )
        .unwrap();
    }
    s
}

fn index(s: &HerculesDb, fields: &[&str]) {
    for f in fields {
        s.create_index(ACME, "Ticket", f).unwrap();
    }
}

fn explain(s: &HerculesDb, q: &Query) -> Vec<String> {
    let plan = s.explain(ACME, q).unwrap();
    plan.index_fields().into_iter().map(String::from).collect()
}

fn created(s: &HerculesDb, q: Query) -> BTreeSet<i64> {
    int_set(&s.query(ACME, q).unwrap(), "created_at")
}

fn expected(pred: impl Fn(i64) -> bool) -> BTreeSet<i64> {
    (0..N).filter(|&i| pred(i)).collect()
}

fn q1() -> Query {
    Query::new("Ticket").filter(Filter::and([
        Filter::eq("status", "Open"),
        Filter::eq("customer_id", X),
    ]))
}

fn q2() -> Query {
    Query::new("Ticket")
        .order_by("created_at", Dir::Desc)
        .limit(10)
}

#[test]
fn explain_is_scan_without_a_usable_index() {
    let s = tickets();
    for q in [
        q1(),
        q2(),
        Query::new("Ticket").filter(Filter::eq("notes", "urgent")),
    ] {
        assert!(
            explain(&s, &q).is_empty(),
            "no indexes yet, so {q:?} must scan"
        );
    }

    index(&s, &["customer_id"]);
    let unindexed = Query::new("Ticket").filter(Filter::eq("notes", "urgent"));
    assert!(explain(&s, &unindexed).is_empty());
    assert_eq!(created(&s, unindexed), expected(|i| notes(i) == "urgent"));
}

#[test]
fn explain_uses_an_index_for_a_single_predicate() {
    let s = tickets();
    index(&s, &["customer_id", "created_at"]);

    let eq = Query::new("Ticket").filter(Filter::eq("customer_id", X));
    assert_eq!(explain(&s, &eq), ["customer_id"]);
    assert_eq!(created(&s, eq), expected(|i| i % CUSTOMERS == X));

    let range = Query::new("Ticket").filter(Filter::range(
        "created_at",
        Included(Value::Int(100)),
        Included(Value::Int(120)),
    ));
    assert_eq!(explain(&s, &range), ["created_at"]);
    assert_eq!(created(&s, range), (100..=120).collect());
}

#[test]
fn planner_picks_the_selective_index() {
    let s = tickets();
    // status is indexed first and listed first, but almost every row is Open.
    index(&s, &["status", "customer_id"]);

    let plan = explain(&s, &q1());
    assert_eq!(
        plan.first().map(String::as_str),
        Some("customer_id"),
        "plan reads {plan:?}"
    );
    assert_eq!(
        created(&s, q1()),
        expected(|i| status(i) == "Open" && i % CUSTOMERS == X)
    );
}

#[test]
fn order_by_with_limit_walks_the_index() {
    let s = tickets();
    index(&s, &["created_at"]);

    assert_eq!(explain(&s, &q2()), ["created_at"]);
    let recs = s.query(ACME, q2()).unwrap();
    assert_eq!(
        ints(&recs, "created_at"),
        (N - 10..N).rev().collect::<Vec<_>>()
    );
}

#[test]
fn or_of_indexed_predicates_uses_both_indexes() {
    let s = tickets();
    index(&s, &["customer_id", "status"]);

    let q = Query::new("Ticket").filter(Filter::or([
        Filter::eq("customer_id", X),
        Filter::eq("status", "Closed"),
    ]));
    let plan = explain(&s, &q);
    assert!(
        plan.contains(&"customer_id".to_string()) && plan.contains(&"status".to_string()),
        "expected a union over both indexes, plan reads {plan:?}"
    );
    let recs = s.query(ACME, q).unwrap();
    assert_eq!(ids(&recs).len(), recs.len(), "duplicate records in result");
    assert_eq!(
        int_set(&recs, "created_at"),
        expected(|i| i % CUSTOMERS == X || status(i) == "Closed")
    );
}

#[test]
fn explain_validates_like_query() {
    let s = tickets();
    index(&s, &["customer_id"]);

    assert_eq!(
        s.explain(ACME, &Query::new("Nope")),
        Err(Error::UnknownObject)
    );
    assert_eq!(
        s.explain(GLOBEX, &Query::new("Ticket")),
        Err(Error::UnknownObject)
    );
    assert_eq!(
        s.explain(ACME, &Query::new("Ticket").filter(Filter::eq("nope", 1))),
        Err(Error::UnknownField("nope".into()))
    );
    assert_eq!(
        s.explain(ACME, &Query::new("Ticket").order_by("nope", Dir::Asc)),
        Err(Error::UnknownField("nope".into()))
    );

    // explain must not change what query returns.
    let q = Query::new("Ticket").filter(Filter::eq("customer_id", X));
    let before = created(&s, q.clone());
    s.explain(ACME, &q).unwrap();
    s.explain(ACME, &q).unwrap();
    assert_eq!(created(&s, q), before);
}
