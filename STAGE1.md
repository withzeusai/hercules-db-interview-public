# Stage 1: Runtime schemas and tenant isolation

Tenants define objects and fields at runtime. The store validates every write
against that metadata, and answers queries with a full scan.

## Build

In `crates/hercules-db` (start in `lib.rs`, and split it up as you like):

- `define_object` and `add_field`
- `insert`, `get`, `update` and `delete`
- `query` with `Eq`, `Range`, `And`, `Or`, `order_by` and `limit`

`create_index` (Stage 2) and `explain` (Stage 3) can stay `todo!()` for now.

## Requirements

- Every write is validated against the object's schema: value types, picklist
  membership, text length, required fields, and unknown fields.
- `update` is a partial patch. `Value::Null` clears a field, and an update that
  fails validation changes nothing.
- Tenants are isolated. Two tenants can each define an `Invoice` with
  different fields, and neither can see the other's schema or records.
- Every `Store` method takes `&self` and the store must be `Send + Sync`, so
  choose an interior-mutability and locking strategy up front.

## Errors

| Situation | Error |
|---|---|
| The tenant has no object with that name | `UnknownObject` |
| A field name that isn't in the object's schema (in writes, filters, `order_by`, `create_index`) | `UnknownField(name)` |
| A value whose variant doesn't fit the field's type | `TypeMismatch { field }` |
| A required field that is missing or null | `Required(name)` |
| A record id that doesn't exist in this tenant's object | `NotFound` |
| Anything else invalid: a duplicate object or field name, text that is too long, a value outside its picklist, a decimal with too many places | `Invalid(reason)` |

## Tests

```sh
cargo test -p harness --test stage1
```

The provided tests are a subset of what we run at the end. Write your own,
especially for cases the spec leaves open. `proptest` is available.

## Gaps in the spec

They're deliberate. When you hit one, raise it with your interviewer and talk
it through before you decide. Note what you settled on in `NOTES.md`.
