# M0 — foundation

[Back to index](index.md)

### 4.2 M0 delivered (evidence)

| Deliverable | Where | Proof |
| --- | --- | --- |
| Nine-crate workspace, one-way dependencies | `Cargo.toml`, `crates/` | `./scripts/check.sh` green: `fmt --check`, `clippy -D warnings`, 123 tests |
| Millimetre-exact units, locale-free formatting | `crates/cam-units` | 10 tests, 3 property tests |
| Arc-first-class 2D kernel with arc-preserving offset | `crates/cam-geom` | 25 tests, 7 property tests; a circle offset stays two arcs |
| Document model with `c2d/document@1` | `crates/cam-model` | 9 tests; invalid geometry cannot be deserialised |
| `.c2d` ZIP+JSON store, atomic writes | `crates/cam-store` | 8 tests; unknown archive entries survive a round trip |
| SQLite tool library with migrations | `crates/cam-store` | 7 tests, incl. a v1→v2 migration of a legacy file |
| `.pp` parser (13 directives, 16 blocks) | `crates/cam-post` | 18 tests, every rejection with its `line:column` |
| Data-driven emitter | `crates/cam-post` | 13 tests, incl. the golden program and the arc refusal |
| Profile strategy | `crates/cam-toolpath` | 20 tests: sides, allowance, passes, tabs, leads, origins |
| Headless CLI with the documented exit codes | `crates/cam-cli` | 12 tests, golden G-code through the real binary |

Known M0 limits, recorded so they are not discovered later: offsetting is not arrangement-based (a
shrinking offset that would pass through the shape is refused, not topologically resolved); tabs split
arcs into sub-arcs but the tab boundary itself is a straight Z move; `FEED_RATE_CHANGE` is parsed but
never emitted because no M0 strategy changes feed mid-segment.

