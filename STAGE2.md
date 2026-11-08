# Stage 2: Secondary indexes

`create_index(t, object, field)` builds an ordered secondary index on one field.

## Requirements

- One ordered index per tenant, object and field, able to serve both `Eq` and
  `Range`.
- Creating an index on an object that already has records backfills it.
- The index stays exact through `insert`, `update` and `delete`.
- `query` uses an index when the filter is a single `Eq` or `Range` on an
  indexed field. Anything more complex can still scan until Stage 3.

## Tests

```sh
cargo test -p harness --test stage2
```

As before, these are a subset of what we run at the end.

## Benchmark

```sh
cargo bench -p bench --bench stage2
```

The benchmark uses 1M records in one tenant, with 1,000 distinct values (0.1% selectivity).
`eq_indexed` queries the indexed field and `eq_scan` queries an identical
unindexed copy of it.

Target on a laptop: `eq_indexed` p50 under 1 ms. Expect `eq_scan` to take tens
of milliseconds or more. Set `HERCULES_DB_BENCH_ROWS=100000` for quicker runs
while you iterate.
