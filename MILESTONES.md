# Milestones

Three stages, each building on the last. Run a stage's provided tests with
`cargo test -p harness --test stageN`. They are a subset of what we run at the
end, so write your own too.

The spec has gaps on purpose. When you hit one, discuss it with your
interviewer before you decide, and note the outcome in `NOTES.md`.

## Stage 1: Runtime schemas and tenant isolation

Implement everything except `create_index` and `explain`. Queries scan.

- Validate every write against the schema: value types, picklist membership,
  text length, required fields, and unknown fields.
- Tenants are isolated. Two tenants can each define an `Invoice` with
  different fields, and neither can see the other's schema or records.
- A value of the wrong variant is `TypeMismatch`. Any other invalid input
  (text too long, value outside the picklist, too many decimal places,
  duplicate object or field name) is `Invalid`. The remaining errors mean what
  their doc comments in `api.rs` say.

## Stage 2: Secondary indexes

`create_index` builds an ordered index on one field.

- One index per tenant, object and field, serving both `Eq` and `Range`.
- Creating an index on existing records backfills it.
- The index stays exact through `insert`, `update` and `delete`.
- `query` uses an index when the filter is a single `Eq` or `Range` on an
  indexed field. Anything more complex can scan until Stage 3.

**Benchmark:** `cargo bench -p bench --bench stage2` uses 1M records at 0.1%
selectivity. It compares an indexed field (`eq_indexed`) with an identical
unindexed copy (`eq_scan`). Target: `eq_indexed` p50 under 1 ms.

## Stage 3: Query planner

`query` chooses a plan, and `explain` returns the plan `query` would use
without running it.

- Choose between a full scan, one index (`Eq` or `Range`) with the rest of the
  filter applied per row, and a union for `Or`. Index intersection is not
  required.
- Serve `order_by` + `limit` by walking the index on the `order_by` field, not
  by sorting every row.
- Any planning strategy is fine if the benchmarks come out right (for example,
  costs from cheap statistics, or racing plans and caching the winner).
- Extend `Plan` so `explain` output would help someone debug a slow query.
  Keep `Plan::index_fields` accurate, because the graders use it.

**Benchmark:** `cargo bench -p bench --bench stage3` uses 1M `Ticket` records.
`status`, `customer_id` and `created_at` are indexed, in that order, and
`notes` is not.

| Benchmark | Query | Target |
|---|---|---|
| `scan_baseline` | `notes` equals a value no row has (a full scan) | the comparison point |
| `q1_open_for_customer` | `And(status = Open, customer_id = X)`. 90% of rows are Open; X matches 0.01%. | at least 50x faster than `scan_baseline` |
| `q2_latest_10` | `order_by created_at desc`, `limit 10` | at least 50x faster |
| `q3_customer_or_urgent` | `Or(customer_id = X, notes = "urgent")`. "urgent" is on 0.1% of rows. | at most 1.5x `scan_baseline` |
