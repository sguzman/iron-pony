# Iron Pony

`iron-pony` is a Rust port baseline for `ponysay` focused on deterministic behavior, extensive tracing, and parity-driven development.

This repository implements a modular workspace with:
- pony template loading and metadata parsing (`$$$` headers)
- upstream-compatible balloon style parsing (`.say` / `.think`) and rendering
- an internal fortune fast path (`--fortune`) for single-process startup workflows
- a differential parity harness that compares `iron-pony` against upstream `ponysay`/`ponythink`

## Workspace Layout

- `crates/iron-pony-core`: pony/balloon/fortune core logic
- `crates/iron-pony-cli`: `iron-pony` binary and CLI plumbing
- `crates/iron-pony-spec`: parity requirement/spec loading
- `crates/iron-pony-parity`: differential runner + report generation
- `crates/xtask`: automation commands (`xtask parity`)
- `spec/requirements.yaml`: weighted requirement definitions
- `tests/parity_cases/*.json`: parity case corpus
- `testdata/`: local pony/balloon/fortune fixtures

## Current Scope

This is a comprehensive **port scaffold and baseline implementation** designed for iterative parity work.
The included parity corpus currently reports 100% case and weighted requirement parity.

The parity harness reports exactly how far away the port is by:
- case parity (`passed_cases / total_cases`)
- weighted requirement parity
- per-requirement status (`done`, `failing`, `untested`)

## Build and Test

```bash
cargo build --workspace
cargo test --workspace
```

## Run

```bash
cargo run -p iron-pony-cli -- -f twilight -b say "Hello from Iron Pony"
```

Think mode:

```bash
cargo run -p iron-pony-cli -- --think --wrap 22 -f twilight "Thinking in Rust"
```

Internal fortune mode:

```bash
cargo run -p iron-pony-cli -- --fortune --fortune-all --fortune-equal --seed 7
```

## Tracing / Logging

Logging is built with `tracing` + `tracing-subscriber` across CLI/core/parity tooling.

Default filter:
- `info` globally
- `debug` for `iron_pony_core`, `iron_pony_parity`, and `xtask`

Override with `RUST_LOG`:

```bash
RUST_LOG=debug cargo run -p iron-pony-cli -- -f twilight "trace me"
```

## Parity Harness

Run:

```bash
cargo run -p xtask -- parity
```

Environment overrides:
- `PONYSAY_REF`: reference program (default: `ponysay`)
- `IRON_PONY_BIN`: candidate binary path (otherwise harness uses `cargo run -p iron-pony-cli`)

Deterministic fixture set used by current parity cases:
- `testdata/ponies/simple_say.pony`
- `testdata/ponies/simple_think.pony`
- `testdata/balloons/ascii.say`
- `testdata/balloons/ascii.think`

Outputs:
- `target/parity/parity-report.json`
- `target/parity/parity-report.md`
- `target/parity/failures/<case_id>.diff`

Case format supports:
- `argv`
- optional `reference_program`
- optional `reference_argv`
- optional `candidate_program`
- optional `candidate_argv`
- `stdin`
- `env`
- `features` (mapped to weighted requirements)

## Notes on Upstream Compatibility

Upstream `ponysay` behavior depends on installed pony assets, balloon styles, terminal mode, and environment.
The parity harness is built to expose these mismatches quickly and make remaining work measurable.

## License

MIT (project code in this repo).
