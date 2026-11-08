# Stage 3: Query planner

Objects now have several indexes. `query` must choose a good plan, and
`explain` returns the plan `query` would use, without running it.

## Requirements

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

## Tests

```sh
cargo test -p harness --test stage3
```

## Benchmark

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
