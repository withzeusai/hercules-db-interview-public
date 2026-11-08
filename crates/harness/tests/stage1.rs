//! Stage 1: runtime schemas and tenant isolation.

use std::ops::Bound::{Excluded, Included, Unbounded};

use harness::*;

fn setup() -> HerculesDb {
    let s = store();
    s.define_object(ACME, invoice()).unwrap();
    s
}

/// Six invoices used by the query tests.
fn seed(s: &HerculesDb) {
    let rows = [
        ("INV-1", "Initech", "10.00", "Draft", 1),
        ("INV-2", "Initech", "25.50", "Open", 2),
        ("INV-3", "Hooli", "25.50", "Paid", 3),
        ("INV-4", "Hooli", "99.99", "Open", 4),
        ("INV-5", "Umbrella", "150.00", "Open", 5),
        ("INV-6", "Umbrella", "7.25", "Paid", 6),
    ];
    for (number, customer, amount, status, quantity) in rows {
        let mut v = invoice_values(number, customer, amount, status);
        v.insert("quantity".into(), Value::Int(quantity));
        s.insert(ACME, "Invoice", v).unwrap();
    }
}

/// Invoice numbers matching `filter`, sorted.
fn numbers(s: &HerculesDb, filter: Filter) -> Vec<String> {
    let recs = s.query(ACME, Query::new("Invoice").filter(filter)).unwrap();
    text_set(&recs, "number").into_iter().collect()
}

#[test]
fn insert_then_get_roundtrips() {
    let s = setup();
    let id = s
        .insert(
            ACME,
            "Invoice",
            fields! {
                "number" => "INV-1",
                "customer" => "Initech",
                "amount" => dec("120.50"),
                "status" => "Open",
                "quantity" => 3,
                "paid" => false,
                "notes" => "net 30",
            },
        )
        .unwrap();
    let r = s.get(ACME, "Invoice", id).unwrap();
    assert_eq!(r.id, id);
    assert_eq!(val(&r, "number"), &text("INV-1"));
    assert_eq!(val(&r, "customer"), &text("Initech"));
    assert_eq!(val(&r, "amount"), &dec("120.50"));
    assert_eq!(val(&r, "status"), &text("Open"));
    assert_eq!(val(&r, "quantity"), &Value::Int(3));
    assert_eq!(val(&r, "paid"), &Value::Bool(false));
    assert_eq!(val(&r, "notes"), &text("net 30"));

    let other = s
        .insert(
            ACME,
            "Invoice",
            invoice_values("INV-2", "Hooli", "1.00", "Draft"),
        )
        .unwrap();
    assert_ne!(id, other, "every insert gets a fresh id");
    assert_eq!(
        val(&s.get(ACME, "Invoice", other).unwrap(), "number"),
        &text("INV-2")
    );
}

#[test]
fn unknown_objects_and_fields_are_rejected() {
    let s = store();
    let v = invoice_values("INV-1", "Initech", "1.00", "Open");
    assert_eq!(
        s.insert(ACME, "Invoice", v.clone()),
        Err(Error::UnknownObject)
    );
    assert_eq!(
        s.get(ACME, "Invoice", RecordId(1)),
        Err(Error::UnknownObject)
    );
    assert_eq!(
        s.query(ACME, Query::new("Invoice")),
        Err(Error::UnknownObject)
    );
    assert_eq!(
        s.add_field(ACME, "Invoice", FieldDef::new("po", FieldType::Integer)),
        Err(Error::UnknownObject)
    );

    s.define_object(ACME, invoice()).unwrap();
    let mut bad = v.clone();
    bad.insert("discount".into(), dec("1.00"));
    assert_eq!(
        s.insert(ACME, "Invoice", bad),
        Err(Error::UnknownField("discount".into()))
    );
    assert_eq!(
        s.query(
            ACME,
            Query::new("Invoice").filter(Filter::eq("discount", 1))
        ),
        Err(Error::UnknownField("discount".into()))
    );
    assert_eq!(
        s.query(ACME, Query::new("Invoice").order_by("discount", Dir::Asc)),
        Err(Error::UnknownField("discount".into()))
    );
    assert_eq!(
        s.query(ACME, Query::new("Customer")),
        Err(Error::UnknownObject)
    );
}

#[test]
fn values_must_match_field_types() {
    let s = setup();
    let cases = [
        ("amount", text("12.00")),
        ("quantity", text("3")),
        ("quantity", dec("3.0")),
        ("paid", Value::Int(1)),
        ("number", Value::Int(7)),
        ("status", Value::Bool(true)),
    ];
    for (field, bad) in cases {
        let mut v = invoice_values("INV-1", "Initech", "1.00", "Open");
        v.insert(field.into(), bad.clone());
        assert_eq!(
            s.insert(ACME, "Invoice", v),
            Err(Error::TypeMismatch {
                field: field.into()
            }),
            "{field} = {bad:?}"
        );
    }
    assert!(
        s.query(ACME, Query::new("Invoice")).unwrap().is_empty(),
        "rejected inserts must not be stored"
    );
}

#[test]
fn required_fields_are_enforced() {
    let s = setup();

    let mut missing = invoice_values("INV-1", "Initech", "1.00", "Open");
    missing.remove("customer");
    assert_eq!(
        s.insert(ACME, "Invoice", missing),
        Err(Error::Required("customer".into()))
    );

    let mut null = invoice_values("INV-1", "Initech", "1.00", "Open");
    null.insert("customer".into(), Value::Null);
    assert_eq!(
        s.insert(ACME, "Invoice", null),
        Err(Error::Required("customer".into()))
    );

    let id = s
        .insert(
            ACME,
            "Invoice",
            invoice_values("INV-1", "Initech", "1.00", "Open"),
        )
        .unwrap();
    assert_eq!(
        s.update(ACME, "Invoice", id, fields! { "customer" => Value::Null }),
        Err(Error::Required("customer".into()))
    );

    // Optional fields may be omitted or null.
    let mut optional_null = invoice_values("INV-2", "Initech", "1.00", "Open");
    optional_null.insert("quantity".into(), Value::Null);
    s.insert(ACME, "Invoice", optional_null).unwrap();
}

#[test]
fn text_length_and_picklist_are_enforced() {
    let s = setup();
    let longest = "X".repeat(12);
    s.insert(
        ACME,
        "Invoice",
        invoice_values(&longest, "Initech", "1.00", "Open"),
    )
    .expect("12 characters fits max_len 12");

    let too_long = "X".repeat(13);
    let r = s.insert(
        ACME,
        "Invoice",
        invoice_values(&too_long, "Initech", "1.00", "Open"),
    );
    assert!(matches!(r, Err(Error::Invalid(_))), "13 characters: {r:?}");

    for status in ["Overdue", "open", ""] {
        let r = s.insert(
            ACME,
            "Invoice",
            invoice_values("INV-1", "Initech", "1.00", status),
        );
        assert!(
            matches!(r, Err(Error::Invalid(_))),
            "status {status:?}: {r:?}"
        );
    }

    let id = s
        .insert(
            ACME,
            "Invoice",
            invoice_values("INV-1", "Initech", "1.00", "Open"),
        )
        .unwrap();
    let r = s.update(ACME, "Invoice", id, fields! { "status" => "Void" });
    assert!(matches!(r, Err(Error::Invalid(_))), "update to Void: {r:?}");
}

#[test]
fn update_and_delete() {
    let s = setup();
    let mut v = invoice_values("INV-1", "Initech", "10.00", "Draft");
    v.insert("quantity".into(), Value::Int(1));
    let id = s.insert(ACME, "Invoice", v).unwrap();

    s.update(
        ACME,
        "Invoice",
        id,
        fields! { "status" => "Open", "quantity" => 5 },
    )
    .unwrap();
    let r = s.get(ACME, "Invoice", id).unwrap();
    assert_eq!(val(&r, "status"), &text("Open"));
    assert_eq!(val(&r, "quantity"), &Value::Int(5));
    assert_eq!(
        val(&r, "number"),
        &text("INV-1"),
        "fields not in the patch are kept"
    );
    assert_eq!(val(&r, "amount"), &dec("10.00"));

    // A rejected update changes nothing, including its valid fields.
    let r = s.update(
        ACME,
        "Invoice",
        id,
        fields! { "paid" => true, "quantity" => "many" },
    );
    assert_eq!(
        r,
        Err(Error::TypeMismatch {
            field: "quantity".into()
        })
    );
    let r = s.get(ACME, "Invoice", id).unwrap();
    assert!(
        val(&r, "paid").is_null(),
        "paid must not be set by a failed update"
    );
    assert_eq!(val(&r, "quantity"), &Value::Int(5));

    // Null clears an optional field.
    s.update(ACME, "Invoice", id, fields! { "quantity" => Value::Null })
        .unwrap();
    assert!(val(&s.get(ACME, "Invoice", id).unwrap(), "quantity").is_null());

    s.delete(ACME, "Invoice", id).unwrap();
    assert_eq!(s.get(ACME, "Invoice", id), Err(Error::NotFound));
    assert_eq!(
        s.update(ACME, "Invoice", id, fields! { "quantity" => 1 }),
        Err(Error::NotFound)
    );
    assert_eq!(s.delete(ACME, "Invoice", id), Err(Error::NotFound));
    assert!(s.query(ACME, Query::new("Invoice")).unwrap().is_empty());
}

#[test]
fn add_field_extends_the_schema() {
    let s = setup();
    let old = s
        .insert(
            ACME,
            "Invoice",
            invoice_values("INV-1", "Initech", "1.00", "Open"),
        )
        .unwrap();

    s.add_field(
        ACME,
        "Invoice",
        FieldDef::new("po_number", FieldType::Text { max_len: 20 }),
    )
    .unwrap();

    let mut v = invoice_values("INV-2", "Hooli", "2.00", "Open");
    v.insert("po_number".into(), text("PO-7"));
    let new = s.insert(ACME, "Invoice", v).unwrap();
    assert_eq!(
        val(&s.get(ACME, "Invoice", new).unwrap(), "po_number"),
        &text("PO-7")
    );

    s.update(ACME, "Invoice", old, fields! { "po_number" => "PO-1" })
        .unwrap();
    assert_eq!(
        val(&s.get(ACME, "Invoice", old).unwrap(), "po_number"),
        &text("PO-1")
    );
    let found = s
        .query(
            ACME,
            Query::new("Invoice").filter(Filter::eq("po_number", "PO-1")),
        )
        .unwrap();
    assert_eq!(ids(&found), [old].into());

    let dup = s.add_field(
        ACME,
        "Invoice",
        FieldDef::new("po_number", FieldType::Integer),
    );
    assert!(
        matches!(dup, Err(Error::Invalid(_))),
        "duplicate field: {dup:?}"
    );
    let dup = s.define_object(ACME, invoice());
    assert!(
        matches!(dup, Err(Error::Invalid(_))),
        "duplicate object: {dup:?}"
    );
}

#[test]
fn tenants_are_isolated() {
    let s = store();
    s.define_object(ACME, invoice()).unwrap();
    s.define_object(
        GLOBEX,
        ObjectDef::new(
            "Invoice",
            vec![
                FieldDef::new("ref", FieldType::Integer).required(),
                FieldDef::new("total", FieldType::Integer),
            ],
        ),
    )
    .unwrap();

    let a = s
        .insert(
            ACME,
            "Invoice",
            invoice_values("INV-1", "Initech", "1.00", "Open"),
        )
        .unwrap();
    let g = s
        .insert(GLOBEX, "Invoice", fields! { "ref" => 1, "total" => 99 })
        .unwrap();

    // Each tenant validates against its own schema.
    assert_eq!(
        s.insert(
            GLOBEX,
            "Invoice",
            fields! { "ref" => 2, "number" => "INV-1" }
        ),
        Err(Error::UnknownField("number".into()))
    );
    let mut v = invoice_values("INV-2", "Initech", "1.00", "Open");
    v.insert("ref".into(), Value::Int(3));
    assert_eq!(
        s.insert(ACME, "Invoice", v),
        Err(Error::UnknownField("ref".into()))
    );

    // Each tenant sees only its own records.
    let acme = s.query(ACME, Query::new("Invoice")).unwrap();
    assert_eq!(ids(&acme), [a].into());
    let globex = s.query(GLOBEX, Query::new("Invoice")).unwrap();
    assert_eq!(ids(&globex), [g].into());
    assert_eq!(
        s.query(GLOBEX, Query::new("Invoice").filter(Filter::eq("ref", 1)))
            .unwrap()
            .len(),
        1
    );

    // Reaching across tenants by id never exposes or changes the other
    // tenant's data. (Ids may be scoped per tenant, so the same number can
    // legitimately name a record in both.)
    match s.get(ACME, "Invoice", g) {
        Err(Error::NotFound) => {}
        Ok(r) => assert!(
            r.fields.contains_key("number"),
            "ACME read GLOBEX's record: {r:?}"
        ),
        Err(e) => panic!("unexpected error: {e:?}"),
    }
    let _ = s.delete(ACME, "Invoice", g);
    let _ = s.update(ACME, "Invoice", g, fields! { "notes" => "x" });
    assert_eq!(
        s.get(GLOBEX, "Invoice", g).unwrap().fields,
        fields! { "ref" => 1, "total" => 99 }
    );

    assert_eq!(
        s.query(TenantId(3), Query::new("Invoice")),
        Err(Error::UnknownObject)
    );
}

#[test]
fn query_filters_by_scan() {
    let s = setup();
    seed(&s);

    assert_eq!(
        numbers(&s, Filter::eq("status", "Open")),
        ["INV-2", "INV-4", "INV-5"]
    );
    assert_eq!(
        numbers(&s, Filter::eq("amount", dec("25.50"))),
        ["INV-2", "INV-3"]
    );
    assert_eq!(
        numbers(&s, Filter::eq("customer", "Nobody")),
        Vec::<&str>::new()
    );
    assert_eq!(
        numbers(
            &s,
            Filter::range("amount", Included(dec("25.50")), Excluded(dec("150.00")))
        ),
        ["INV-2", "INV-3", "INV-4"]
    );
    assert_eq!(
        numbers(
            &s,
            Filter::range("quantity", Excluded(Value::Int(2)), Unbounded)
        ),
        ["INV-3", "INV-4", "INV-5", "INV-6"]
    );
    assert_eq!(
        numbers(
            &s,
            Filter::range("quantity", Unbounded, Included(Value::Int(2)))
        ),
        ["INV-1", "INV-2"]
    );
    assert_eq!(
        numbers(
            &s,
            Filter::and([
                Filter::eq("customer", "Hooli"),
                Filter::eq("status", "Open")
            ])
        ),
        ["INV-4"]
    );
    assert_eq!(
        numbers(
            &s,
            Filter::or([
                Filter::eq("customer", "Initech"),
                Filter::eq("status", "Paid")
            ])
        ),
        ["INV-1", "INV-2", "INV-3", "INV-6"]
    );
    assert_eq!(
        numbers(
            &s,
            Filter::and([
                Filter::or([Filter::eq("status", "Draft"), Filter::eq("status", "Paid")]),
                Filter::range("amount", Included(dec("10.00")), Unbounded),
            ])
        ),
        ["INV-1", "INV-3"]
    );
    assert_eq!(s.query(ACME, Query::new("Invoice")).unwrap().len(), 6);
}

#[test]
fn query_orders_and_limits() {
    let s = setup();
    seed(&s);

    let by_amount = s
        .query(ACME, Query::new("Invoice").order_by("amount", Dir::Asc))
        .unwrap();
    assert_eq!(
        vals(&by_amount, "amount"),
        ["7.25", "10.00", "25.50", "25.50", "99.99", "150.00"].map(dec)
    );

    let top_qty = s
        .query(
            ACME,
            Query::new("Invoice")
                .order_by("quantity", Dir::Desc)
                .limit(3),
        )
        .unwrap();
    assert_eq!(ints(&top_qty, "quantity"), [6, 5, 4]);

    let biggest_open = s
        .query(
            ACME,
            Query::new("Invoice")
                .filter(Filter::eq("status", "Open"))
                .order_by("amount", Dir::Desc)
                .limit(2),
        )
        .unwrap();
    assert_eq!(
        vals(&biggest_open, "number"),
        [text("INV-5"), text("INV-4")]
    );

    let by_customer = s
        .query(ACME, Query::new("Invoice").order_by("customer", Dir::Asc))
        .unwrap();
    assert_eq!(
        vals(&by_customer, "customer"),
        [
            "Hooli", "Hooli", "Initech", "Initech", "Umbrella", "Umbrella"
        ]
        .map(text)
    );

    assert_eq!(
        s.query(ACME, Query::new("Invoice").limit(4)).unwrap().len(),
        4
    );
    assert_eq!(
        s.query(ACME, Query::new("Invoice").limit(100))
            .unwrap()
            .len(),
        6
    );
}
