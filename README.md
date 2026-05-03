# Iron Pony

`iron-pony` is a Rust port baseline for `ponysay`, built around deterministic behavior, tracing, and parity-driven development.

## Intent

Recreate the upstream experience in Rust while improving internal structure, safety, and testability.

## Ambition

The parity harness, spec crate, and workspace split indicate a deliberate ambition to become a trustworthy upstream-compatible implementation rather than just a themed text renderer.

## Current Status

The workspace already contains core logic, CLI, spec, parity tooling, tests, and test data. It appears well underway as a compatibility-focused port.

## Core Capabilities Or Focus Areas

- Rust port of the core `ponysay` behavior.
- Dedicated crates for core logic, CLI, spec modeling, parity checks, and workspace tooling.
- Tracing and deterministic behavior for debugging and testing.
- Parity-oriented validation against upstream behavior.
- Test data and fixtures for compatibility work.

## Project Layout

- `crates/iron-pony-core/`: core rendering and parsing behavior for the Rust port.
- `crates/iron-pony-cli/`: CLI binary surface matching the user-facing command behavior.
- `crates/iron-pony-spec/`: shared spec types and compatibility-focused models.
- `crates/iron-pony-parity/`: parity harnesses for comparing against upstream behavior.
- `crates/xtask/`: workspace automation and developer tooling tasks.
- `crates/`: workspace member crates grouped by subsystem.
- `tests/`: automated tests, fixtures, or parity scenarios.
- `Cargo.toml`: crate or workspace manifest and the first place to check for package structure.

## Setup And Requirements

- Rust toolchain.
- Any upstream reference binaries or assets needed for parity work.
- Terminal environment suitable for the rendered output.

## Build / Run / Test Commands

```bash
cargo build --workspace
cargo test --workspace
cargo run -p iron-pony-cli -- --help
```

## Notes, Limitations, Or Known Gaps

- Compatibility and determinism matter more here than adding unrelated new features.
- Parity behavior should be treated as a first-class contract.

## Next Steps Or Roadmap Hints

- Keep broadening parity coverage as edge cases are discovered.
- Use the spec/parity crates to prevent regression as internals evolve.
