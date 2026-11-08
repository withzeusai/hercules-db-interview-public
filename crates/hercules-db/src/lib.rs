//! hercules-db: an in-memory, multi-tenant, metadata-driven record store.
//!
//! The public contract is in `api.rs` and is re-exported here. Implement
//! [`Store`] for [`HerculesDb`] below, and split it into modules as you go.

mod api;

pub use api::*;

/// Your store. Add whatever state you need.
#[derive(Default)]
pub struct HerculesDb {}

impl HerculesDb {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Store for HerculesDb {
    fn define_object(&self, _t: TenantId, _def: ObjectDef) -> Result<()> {
        todo!()
    }

    fn add_field(&self, _t: TenantId, _object: &str, _field: FieldDef) -> Result<()> {
        todo!()
    }

    fn insert(&self, _t: TenantId, _object: &str, _values: Fields) -> Result<RecordId> {
        todo!()
    }

    fn get(&self, _t: TenantId, _object: &str, _id: RecordId) -> Result<Record> {
        todo!()
    }

    fn update(&self, _t: TenantId, _object: &str, _id: RecordId, _patch: Fields) -> Result<()> {
        todo!()
    }

    fn delete(&self, _t: TenantId, _object: &str, _id: RecordId) -> Result<()> {
        todo!()
    }

    fn query(&self, _t: TenantId, _q: Query) -> Result<Vec<Record>> {
        todo!()
    }

    fn create_index(&self, _t: TenantId, _object: &str, _field: &str) -> Result<()> {
        todo!("Stage 2")
    }

    fn explain(&self, _t: TenantId, _q: &Query) -> Result<Plan> {
        todo!("Stage 3")
    }
}
