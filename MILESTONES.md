# Milestones

The work comes in three stages, and each builds on the last: runtime schemas,
then secondary indexes, then a query planner that chooses between them. Work
through them in order. Nobody is expected to finish everything, and your
interviewer will move you on at each time box.

**Tests.** Each stage has provided tests in `crates/harness/tests/`. They are
a subset of what we run at the end, so write your own as well, especially for
cases the spec leaves open. `proptest` is available.

**Gaps in the spec.** They're deliberate. When you hit one, raise it with your
interviewer and talk it through before you decide. Note what you settled on in
`NOTES.md`.

## Stage 1: Runtime schemas and tenant isolation

Tenants define objects and fields at runtime. The store validates every write
against that metadata, and answers queries with a full scan.

### Build

In `crates/hercules-db` (start in `lib.rs`, and split it up as you like):

- `define_object` and `add_field`
- `insert`, `get`, `update` and `delete`
- `query` with `Eq`, `Range`, `And`, `Or`, `order_by` and `limit`

`create_index` (Stage 2) and `explain` (Stage 3) can stay `todo!()` for now.

### Requirements

- Every write is validated against the object's schema: value types, picklist
  membership, text length, required fields, and unknown fields.
- `update` is a partial patch. `Value::Null` clears a field, and an update that
  fails validation changes nothing.
- Tenants are isolated. Two tenants can each define an `Invoice` with
  different fields, and neither can see the other's schema or records.
- Every `Store` method takes `&self` and the store must be `Send + Sync`, so
  choose an interior-mutability and locking strategy up front.

### Errors

| Situation | Error |
|---|---|
| The tenant has no object with that name | `UnknownObject` |
| A field name that isn't in the object's schema (in writes, filters, `order_by`, `create_index`) | `UnknownField(name)` |
| A value whose variant doesn't fit the field's type | `TypeMismatch { field }` |
| A required field that is missing or null | `Required(name)` |
| A record id that doesn't exist in this tenant's object | `NotFound` |
| Anything else invalid: a duplicate object or field name, text that is too long, a value outside its picklist, a decimal with too many places | `Invalid(reason)` |

### Tests

```sh
cargo test -p harness --test stage1
```

## Stage 2: Secondary indexes

`create_index(t, object, field)` builds an ordered secondary index on one field.

### Requirements

- One ordered index per tenant, object and field, able to serve both `Eq` and
  `Range`.
- Creating an index on an object that already has records backfills it.
- The index stays exact through `insert`, `update` and `delete`.
- `query` uses an index when the filter is a single `Eq` or `Range` on an
  indexed field. Anything more complex can still scan until Stage 3.

### Tests

```sh
cargo test -p harness --test stage2
```

### Benchmark

```sh
cargo bench -p bench --bench stage2
```

The benchmark uses 1M records in one tenant, with 1,000 distinct values (0.1%
selectivity). `eq_indexed` queries the indexed field and `eq_scan` queries an
identical unindexed copy of it.

Target on a laptop: `eq_indexed` p50 under 1 ms. Expect `eq_scan` to take tens
of milliseconds or more. Set `HERCULES_DB_BENCH_ROWS=100000` for quicker runs
while you iterate.

## Stage 3: Query planner

Objects now have several indexes. `query` must choose a good plan, and
`explain` returns the plan `query` would use, without running it.

### Requirements

- Choose between a full scan, a single index (`Eq` or `Range`) with the rest
  of the filter applied per row, and a union for `Or`. Index intersection is
  not required.
- Use an index on the `order_by` field to serve `order_by` + `limit` without
  sorting every row.
- Any planning strategy is fine if the benchmarks come out right. For example,
  you could use cost estimates from cheap statistics (the Postgres approach),
  or race candidate plans briefly and cache the winner (the MongoDB approach).
- `Plan` is an open enum. Extend it so `explain` output would help someone
  debug a slow query. Keep `Plan::index_fields` accurate, because the graders
  use it.

### Tests

```sh
cargo test -p harness --test stage3
```

### Benchmark

```sh
cargo bench -p bench --bench stage3
```

The benchmark uses 1M `Ticket` records. `status`, `customer_id` and
`created_at` are indexed, in that order. `notes` is not indexed. Before timing,
the bench prints each query's row count and `explain` output.

| Benchmark | Query | Data | Target vs `scan_baseline` |
|---|---|---|---|
| `q1_open_for_customer` | `And(status = Open, customer_id = X)` | 90% of rows are Open; X matches 0.01% | at least 50x faster |
| `q2_latest_10` | `order_by created_at desc`, `limit 10` | `created_at` is unique | at least 50x faster |
| `q3_customer_or_urgent` | `Or(customer_id = X, notes = "urgent")` | "urgent" is on 0.1% of rows | at most 1.5x slower |

`scan_baseline` is an `Eq` on the unindexed `notes` field that matches nothing.
