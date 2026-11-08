# Hercules DB interview

In this interview you'll build a small in-memory, multi-tenant,
metadata-driven record store in Rust. Tenants define their own objects and
fields at runtime, and queries must stay fast and correct as schemas and data
change underneath them.

The public API is fixed in [`crates/hercules-db/src/api.rs`](crates/hercules-db/src/api.rs).
You implement the `Store` trait for `HerculesDb` in
[`crates/hercules-db/src/lib.rs`](crates/hercules-db/src/lib.rs). Add modules as you like.

## Before the interview

```sh
cargo build --all-targets   # fetches and compiles every allowed crate
cargo test --no-fail-fast   # builds the tests; they fail with `not yet implemented`, which is expected
```

If both commands get as far as running the tests, you're set up. The toolchain is
pinned in `rust-toolchain.toml`, and rustup installs it on first use.

## Layout

```
crates/
  hercules-db/ the store: api.rs (fixed contract) and lib.rs (yours)
  harness/     shared test helpers (src/lib.rs) and the provided tests (tests/stage*.rs)
  bench/       criterion benchmarks for Stages 2 and 3
STAGE1.md      what to build first; later stages are handed over during the interview
NOTES.md       a place to record the decisions you make
```

## Working

```sh
cargo test -p harness --test stage1           # one stage's provided tests
cargo test                                    # everything, including your own tests
cargo bench -p bench --bench stage2           # Stage 2 benchmark (1M rows)
HERCULES_DB_BENCH_ROWS=100000 cargo bench -p bench --bench stage3   # smaller, while iterating
```

The provided tests are a subset of what runs at the end, so write your own as
well. Put them in `crates/hercules-db` (unit tests) or as new files under
`crates/harness/tests/`. Please don't edit the provided test files or the
`Store` trait.

## Rules

- Allowed crates: `std`, `parking_lot`, `dashmap`, `rust_decimal`, `uuid`,
  `thiserror`, `proptest`, `criterion`, `crossbeam`. They are already in the
  workspace, so you won't need the network. Embedded databases (sled,
  rocksdb, rusqlite, redb, ...) are not allowed.
- Out of scope: access control, persistence, nested or array fields, joins,
  and SQL parsing. The store must be `Send + Sync`, but you don't need to
  optimise for concurrent throughput.
- Use whatever AI tools you normally use, and narrate as you go.
- The spec has gaps on purpose. When you hit one, make a call, tell us, and
  note it in `NOTES.md`.
