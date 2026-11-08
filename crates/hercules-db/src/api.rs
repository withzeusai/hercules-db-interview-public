//! The fixed public contract for hercules-db.
//!
//! The graders compile against these types, so do not change the [`Store`]
//! trait or the existing types. You may add inherent methods and trait impls,
//! and you may add variants to [`Plan`]. If you add variants, keep
//! [`Plan::index_fields`] accurate.

use std::collections::BTreeMap;
use std::ops::Bound;

pub use rust_decimal::Decimal;

/// A tenant. Every call is scoped to exactly one tenant. Tenants must never see
/// each other's schemas or records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TenantId(pub u64);

/// A record's identity, assigned by the store on insert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecordId(pub u64);

/// The type of a field. Tenants define field types at runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    /// UTF-8 text of at most `max_len` characters (Unicode scalar values, as
    /// counted by `str::chars`). Holds [`Value::Text`].
    Text { max_len: u32 },
    /// A 64-bit signed integer. Holds [`Value::Int`].
    Integer,
    /// A decimal with at most `scale` digits after the decimal point. Holds
    /// [`Value::Decimal`].
    Decimal { scale: u8 },
    /// Holds [`Value::Bool`].
    Bool,
    /// Text that must be one of the listed options. Holds [`Value::Text`].
    Picklist(Vec<String>),
}

/// A field value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Text(String),
    Int(i64),
    Decimal(Decimal),
    Bool(bool),
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Text(s.to_owned())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Text(s)
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::Int(i)
    }
}

impl From<Decimal> for Value {
    fn from(d: Decimal) -> Self {
        Value::Decimal(d)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

/// One field of an object.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub name: String,
    pub ty: FieldType,
    /// Every record must have a non-null value for this field.
    pub required: bool,
    /// An optional default value. You define its exact semantics, and you
    /// should document them.
    pub default: Option<Value>,
}

impl FieldDef {
    /// An optional field with no default.
    pub fn new(name: impl Into<String>, ty: FieldType) -> Self {
        FieldDef {
            name: name.into(),
            ty,
            required: false,
            default: None,
        }
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn with_default(mut self, value: impl Into<Value>) -> Self {
        self.default = Some(value.into());
        self
    }
}

/// An object (think: table) and its initial fields.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

impl ObjectDef {
    pub fn new(name: impl Into<String>, fields: Vec<FieldDef>) -> Self {
        ObjectDef {
            name: name.into(),
            fields,
        }
    }
}

/// Field values keyed by field name, for inserts and update patches.
pub type Fields = BTreeMap<String, Value>;

/// Builds a [`Fields`] map:
///
/// ```
/// use hercules_db::{fields, Value};
/// let f = fields! { "name" => "Acme", "seats" => 12, "trial" => false, "notes" => Value::Null };
/// assert_eq!(f["seats"], Value::Int(12));
/// ```
#[macro_export]
macro_rules! fields {
    ($($name:expr => $value:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut fields = $crate::Fields::new();
        $(fields.insert(::std::string::String::from($name), $crate::Value::from($value));)*
        fields
    }};
}

/// A record, as returned by [`Store::get`] and [`Store::query`].
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: RecordId,
    pub fields: Fields,
}

impl Record {
    pub fn get(&self, field: &str) -> Option<&Value> {
        self.fields.get(field)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dir {
    Asc,
    Desc,
}

/// A query filter.
#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    /// The field equals the value.
    Eq(String, Value),
    /// The field lies within the bounds. Use `Bound::Unbounded` for an open end.
    Range(String, Bound<Value>, Bound<Value>),
    /// Every sub-filter matches.
    And(Vec<Filter>),
    /// At least one sub-filter matches.
    Or(Vec<Filter>),
}

impl Filter {
    pub fn eq(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Filter::Eq(field.into(), value.into())
    }

    pub fn range(field: impl Into<String>, lower: Bound<Value>, upper: Bound<Value>) -> Self {
        Filter::Range(field.into(), lower, upper)
    }

    pub fn and(filters: impl IntoIterator<Item = Filter>) -> Self {
        Filter::And(filters.into_iter().collect())
    }

    pub fn or(filters: impl IntoIterator<Item = Filter>) -> Self {
        Filter::Or(filters.into_iter().collect())
    }
}

/// A query over one object: filter, then order, then limit.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub object: String,
    pub filter: Option<Filter>,
    /// Sort by this field. Without it, result order is unspecified.
    pub order_by: Option<(String, Dir)>,
    /// Return at most this many records.
    pub limit: Option<usize>,
}

impl Query {
    /// Every record of `object`.
    pub fn new(object: impl Into<String>) -> Self {
        Query {
            object: object.into(),
            filter: None,
            order_by: None,
            limit: None,
        }
    }

    pub fn filter(mut self, filter: Filter) -> Self {
        self.filter = Some(filter);
        self
    }

    pub fn order_by(mut self, field: impl Into<String>, dir: Dir) -> Self {
        self.order_by = Some((field.into(), dir));
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// How the store executes a query, as returned by [`Store::explain`].
///
/// This is an open enum. Add variants or detail if it helps someone debug a
/// slow query. The graders only call [`Plan::index_fields`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Plan {
    /// Read every record of the object and apply the filter to each one.
    Scan,
    /// Probe the index on `field` with an `Eq` or `Range`, then apply the rest
    /// of the filter to each hit.
    IndexLookup { field: String },
    /// Walk the index on the `order_by` field in order, applying the filter to
    /// each record, until `limit` records match.
    IndexOrder { field: String, dir: Dir },
    /// Run one sub-plan per branch of an `Or` and union the results.
    Union(Vec<Plan>),
}

impl Plan {
    /// The fields whose indexes this plan reads, in plan order. An empty list
    /// means a full scan. The graders rely on this, so keep it accurate if you
    /// add variants.
    pub fn index_fields(&self) -> Vec<&str> {
        match self {
            Plan::Scan => Vec::new(),
            Plan::IndexLookup { field } | Plan::IndexOrder { field, .. } => vec![field.as_str()],
            Plan::Union(plans) => plans.iter().flat_map(Plan::index_fields).collect(),
        }
    }
}

/// Errors returned by the store.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The tenant has no object with that name.
    #[error("unknown object")]
    UnknownObject,
    /// The object has no field with that name.
    #[error("unknown field `{0}`")]
    UnknownField(String),
    /// The value's variant does not fit the field's type.
    #[error("type mismatch for field `{field}`")]
    TypeMismatch { field: String },
    /// A required field is missing or null.
    #[error("required field `{0}` is missing or null")]
    Required(String),
    /// No record with that id exists in this tenant's object.
    #[error("record not found")]
    NotFound,
    /// Any other invalid request, such as a duplicate name, text that is too
    /// long, or a value outside its picklist.
    #[error("invalid: {0}")]
    Invalid(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// The hercules-db API.
///
/// Every method takes `&self` and the store must be `Send + Sync`, so you need
/// an interior-mutability and locking strategy.
pub trait Store: Send + Sync {
    /// Defines a new object for tenant `t`.
    fn define_object(&self, t: TenantId, def: ObjectDef) -> Result<()>;

    /// Adds a field to an existing object.
    fn add_field(&self, t: TenantId, object: &str, field: FieldDef) -> Result<()>;

    /// Validates `values` against the object's schema and inserts a record.
    fn insert(&self, t: TenantId, object: &str, values: Fields) -> Result<RecordId>;

    /// Returns one record.
    fn get(&self, t: TenantId, object: &str, id: RecordId) -> Result<Record>;

    /// Applies a partial update. Fields in `patch` are set (`Value::Null`
    /// clears a field) and the rest are left alone. An update that fails
    /// validation changes nothing.
    fn update(&self, t: TenantId, object: &str, id: RecordId, patch: Fields) -> Result<()>;

    /// Deletes one record.
    fn delete(&self, t: TenantId, object: &str, id: RecordId) -> Result<()>;

    /// Runs a query: filter, then `order_by`, then `limit`.
    fn query(&self, t: TenantId, q: Query) -> Result<Vec<Record>>;

    /// Stage 2: builds an ordered secondary index on one field.
    fn create_index(&self, t: TenantId, object: &str, field: &str) -> Result<()>;

    /// Stage 3: returns the plan `query` would use for `q`, without running it.
    fn explain(&self, t: TenantId, q: &Query) -> Result<Plan>;
}
