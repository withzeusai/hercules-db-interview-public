//! Shared helpers for the hercules-db test suites. Use them in your own tests too.

use std::collections::BTreeSet;

pub use hercules_db::*;

pub const ACME: TenantId = TenantId(1);
pub const GLOBEX: TenantId = TenantId(2);

pub fn store() -> HerculesDb {
    HerculesDb::new()
}

/// A decimal value from a literal. The scale is kept as written, so
/// `dec("1.50")` has scale 2.
pub fn dec(s: &str) -> Value {
    Value::Decimal(s.parse().expect("valid decimal literal"))
}

pub fn text(s: &str) -> Value {
    Value::Text(s.to_owned())
}

pub fn picklist(options: &[&str]) -> FieldType {
    FieldType::Picklist(options.iter().map(|o| o.to_string()).collect())
}

/// The value of `field` in `record`. An absent field reads as `Value::Null`.
pub fn val<'a>(record: &'a Record, field: &str) -> &'a Value {
    record.fields.get(field).unwrap_or(&Value::Null)
}

/// The values of `field` across `records`, in order.
pub fn vals(records: &[Record], field: &str) -> Vec<Value> {
    records.iter().map(|r| val(r, field).clone()).collect()
}

/// The integer values of `field` across `records`, in order.
pub fn ints(records: &[Record], field: &str) -> Vec<i64> {
    records
        .iter()
        .map(|r| match val(r, field) {
            Value::Int(i) => *i,
            other => panic!("expected an integer in `{field}`, got {other:?}"),
        })
        .collect()
}

/// The integer values of `field` across `records`, as a set.
pub fn int_set(records: &[Record], field: &str) -> BTreeSet<i64> {
    ints(records, field).into_iter().collect()
}

/// The text values of `field` across `records`, as a set.
pub fn text_set(records: &[Record], field: &str) -> BTreeSet<String> {
    records
        .iter()
        .map(|r| match val(r, field) {
            Value::Text(s) => s.clone(),
            other => panic!("expected text in `{field}`, got {other:?}"),
        })
        .collect()
}

pub fn ids(records: &[Record]) -> BTreeSet<RecordId> {
    records.iter().map(|r| r.id).collect()
}

/// The `Invoice` object used across the suites.
pub fn invoice() -> ObjectDef {
    ObjectDef::new(
        "Invoice",
        vec![
            FieldDef::new("number", FieldType::Text { max_len: 12 }).required(),
            FieldDef::new("customer", FieldType::Text { max_len: 40 }).required(),
            FieldDef::new("amount", FieldType::Decimal { scale: 2 }).required(),
            FieldDef::new("status", picklist(&["Draft", "Open", "Paid"])).required(),
            FieldDef::new("quantity", FieldType::Integer),
            FieldDef::new("paid", FieldType::Bool),
            FieldDef::new("notes", FieldType::Text { max_len: 200 }),
        ],
    )
}

/// Values for a valid `Invoice` with only the required fields set.
pub fn invoice_values(number: &str, customer: &str, amount: &str, status: &str) -> Fields {
    fields! {
        "number" => number,
        "customer" => customer,
        "amount" => dec(amount),
        "status" => status,
    }
}

/// A small deterministic PRNG (xorshift64*) for building datasets and
/// randomised tests without extra crates.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A value in `0..n`.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    /// True with probability `p`.
    pub fn chance(&mut self, p: f64) -> bool {
        ((self.next_u64() >> 11) as f64 / (1u64 << 53) as f64) < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
}
