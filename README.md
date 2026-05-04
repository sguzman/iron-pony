# Iron Pony

`iron-pony` is a Rust workspace for building a compatibility-focused `ponysay` implementation.
It is structured as a small set of crates that separate rendering, CLI behavior, parity testing,
and project automation so the port can stay deterministic and testable while it closes gaps with
the upstream tools.

The repository is not just a text balloon renderer. It is set up to answer a more specific
question: "does this Rust implementation behave like `ponysay` and `ponythink` in the cases we
care about?" The spec, parity harness, and fixture layout all support that goal.

## What The Project Does

At the top level, the workspace provides:

- A user-facing CLI binary named `iron-pony`.
- A core crate that loads pony templates, loads balloon styles, wraps and renders messages, and
  picks fortunes.
- A spec crate for requirement definitions and feature-to-requirement mapping.
- A parity crate that runs differential comparisons against a reference `ponysay` installation.
- An `xtask` binary for project automation, currently focused on parity runs.

Today the codebase already supports:

- `say` and `think` rendering modes.
- Pony selection by name or path.
- Balloon style selection by name.
- Message input from CLI args or `stdin`.
- Internal fortune selection from fortune databases.
- Seeded randomness for deterministic pony and fortune selection.
- Metadata parsing for pony files that use the `$$$` header format.
- Differential parity cases and weighted requirement scoring.

## Current Scope

This repository is a Rust implementation workspace, not a full asset pack.

- Pony templates are expected to come from a `ponysay` installation or explicit search paths.
- Balloon styles are expected to come from a `ponysay` installation or explicit search paths.
- The repo currently includes sample fortune data and parity cases, but not a bundled upstream pony
  asset tree.

That matters when running the binary locally: builds and unit tests pass in this repo as-is, but
real rendering usually depends on having pony and balloon assets available through standard install
locations or environment overrides.

## Workspace Layout

### Root

- `Cargo.toml`: workspace manifest and shared dependency declarations.
- `Cargo.lock`: locked dependency graph for reproducible builds.
- `README.md`: project documentation.
- `LICENSE`: project license.

### Crates

- `crates/iron-pony-cli/`: the `iron-pony` binary.
  - Parses CLI flags with `clap`.
  - Resolves message input from args, `stdin`, or `--fortune`.
  - Resolves pony, balloon, and fortune search paths from flags or environment.
  - Calls into `iron-pony-core` and writes the final rendered output.

- `crates/iron-pony-core/`: rendering and asset-loading logic.
  - `src/lib.rs`: public API and high-level orchestration.
  - `src/pony.rs`: pony asset discovery, metadata parsing, and balloon insertion.
  - `src/balloon.rs`: balloon style loading, wrap logic, and frame rendering.
  - `src/fortune.rs`: fortune database discovery, parsing, and random selection.

- `crates/iron-pony-spec/`: compatibility spec types.
  - Loads YAML requirement definitions.
  - Maps low-level features to requirement IDs so parity runs can score behavior in a stable way.

- `crates/iron-pony-parity/`: differential test harness.
  - Loads JSON parity cases.
  - Runs the reference program and candidate program with the same environment and input.
  - Compares exit status, `stdout`, and `stderr`.
  - Produces machine-readable report artifacts under `target/parity/`.

- `crates/xtask/`: automation entry point.
  - Wraps parity execution in a small task-oriented CLI.
  - Keeps project tooling out of the main binary.

### Spec, Tests, And Fixtures

- `spec/requirements.yaml`: weighted compatibility requirements and feature mappings.
- `tests/parity_cases/`: JSON parity cases used by the differential harness.
- `testdata/fortunes/`: sample fortune database used for local development and tests.
- `tmp/`: scratch area used by the repository.
- `target/`: Cargo build output and generated parity artifacts.

## Architecture

The main execution flow is:

1. `iron-pony` parses flags and environment.
2. The CLI resolves the message source.
3. The core crate selects a pony template.
4. The core crate loads a balloon style for `say` or `think` mode.
5. The message is wrapped to the configured width.
6. The balloon is rendered and inserted into the pony template.
7. The final ANSI-safe output is written to `stdout`.

The parity flow is separate:

1. The spec crate loads compatibility requirements from YAML.
2. The parity crate loads JSON cases.
3. Each case executes the reference command and the Rust candidate.
4. The outputs are diffed and scored.
5. Reports and failure diffs are written to `target/parity/`.

## CLI Usage

Build and run the binary:

```bash
cargo run -p iron-pony-cli -- --help
```

The current CLI surface is:

```text
Usage: iron-pony [OPTIONS] [MESSAGE]...

Options:
  -f, --pony <PONY>                   Pony template name or path
  -b, --balloon <BALLOON>             Balloon style name
      --think                         Render using think mode
      --verbose                       Enable tracing logs
      --wrap <WRAP>                   Balloon wrap width
      --ponydir <PONY_PATHS>          Pony search path override
      --balloondir <BALLOON_PATHS>    Balloon search path override
      --list                          List available ponies
      --fortune                       Use internal fortune selection
      --fortune-all                   Include offensive fortunes in internal fortune mode
      --fortune-equal                 Select fortune files uniformly in internal fortune mode
      --fortune-path <FORTUNE_PATHS>  Fortune database search path override
      --seed <SEED>                   Deterministic seed for random selection
```

### Common Examples

Render a message with a specific pony and balloon:

```bash
cargo run -p iron-pony-cli -- \
  --pony pinacolada \
  --balloon ascii \
  "Hello from iron pony"
```

Render in think mode with a narrower wrap width:

```bash
cargo run -p iron-pony-cli -- \
  --think \
  --wrap 18 \
  --pony pinacolada \
  --balloon ascii \
  "Thinking in Rust"
```

Read the message from `stdin`:

```bash
printf 'Message from stdin\n' | cargo run -p iron-pony-cli -- \
  --pony pinacolada \
  --balloon ascii
```

List discoverable ponies:

```bash
cargo run -p iron-pony-cli -- --list
```

Use the internal fortune picker:

```bash
cargo run -p iron-pony-cli -- \
  --fortune \
  --pony pinacolada \
  --balloon ascii
```

## Asset And Search Path Behavior

When explicit path flags are not provided, the CLI uses environment variables first and then falls
back to built-in defaults.

### Pony Search

Environment variable:

- `PONYSAY_PONY_PATH`

Default search roots:

- `/usr/share/ponysay/ponies`
- `/usr/share/ponysay/extraponies`
- `/usr/share/ponysay/ttyponies`
- `/usr/local/share/ponysay/ponies`
- `/usr/local/share/ponysay/extraponies`
- `/usr/local/share/ponysay/ttyponies`

### Balloon Search

Environment variable:

- `PONYSAY_BALLOON_PATH`

Default search roots:

- `/usr/share/ponysay/balloons`
- `/usr/local/share/ponysay/balloons`

### Fortune Search

Environment variable:

- `FORTUNE_PATH`

Default search roots used by `iron-pony-core`:

- `testdata/fortunes`
- `/usr/share/games/fortunes`
- `/usr/share/fortune`

## Determinism And Selection Rules

The project intentionally exposes deterministic behavior where it matters for testing.

- `--seed` makes random pony selection deterministic when no pony is explicitly requested.
- `--seed` also makes internal fortune selection deterministic.
- If no pony is specified, the core crate first looks for `best.pony` in the configured pony
  search roots.
- If `best.pony` is not found, a random installed pony is selected.
- `--fortune-equal` changes fortune selection so databases are chosen uniformly rather than weighted
  by the number of fortunes in each file.

## Pony Template And Balloon Style Notes

### Pony Files

Pony assets may contain a metadata header delimited by `$$$`.

- Uppercase `KEY: value` lines are parsed into structured metadata tags.
- Non-tag lines in the header are preserved as metadata comments.
- The rest of the file is treated as the pony body.
- `$balloon$` marks the insertion point for the rendered balloon.
- If no `$balloon$` anchor exists, the balloon is prepended above the pony body.

### Balloon Styles

Balloon styles can be loaded by name from balloon search roots or by explicit path. The loader
checks a few candidate names, including mode-specific suffixes such as `.say` and `.think`, and
also `.balloon`.

If no balloon is requested, the renderer falls back to built-in default styles for:

- `say` mode
- `think` mode

## Compatibility And Parity Tooling

The repository treats compatibility as a tracked engineering target rather than an informal goal.

- `spec/requirements.yaml` defines weighted requirements such as mode support, wrap behavior,
  balloon anchoring, metadata parsing, and parity execution.
- `tests/parity_cases/*.json` define concrete differential cases.
- `iron-pony-parity` compares exit codes, `stdout`, and `stderr` between the reference program and
  the Rust candidate.
- Failed parity cases write diff artifacts under `target/parity/failures/`.

Run the parity task with:

```bash
cargo run -p xtask -- parity
```

Useful overrides:

```bash
cargo run -p xtask -- parity \
  --reference ponysay \
  --candidate ./target/debug/iron-pony \
  --cases tests/parity_cases \
  --spec spec/requirements.yaml \
  --out target/parity
```

The parity harness also looks at these environment variables:

- `PONYSAY_REF`: default reference program for parity runs.
- `IRON_PONY_BIN`: default candidate binary path for parity runs.

## Development

### Requirements

- A recent Rust toolchain with Cargo.
- A local `ponysay`/`ponythink` installation if you want meaningful parity runs against upstream.
- Pony and balloon assets available in standard paths or via overrides if you want to render real
  output from the CLI.

### Common Commands

Build the whole workspace:

```bash
cargo build --workspace
```

Run tests:

```bash
cargo test --workspace
```

Run the main binary:

```bash
cargo run -p iron-pony-cli -- --help
```

Run parity automation:

```bash
cargo run -p xtask -- parity
```

### Verified In This Repository

The current workspace test suite passes with:

```bash
cargo test --workspace
```

That validates the internal crates and unit coverage in this repo. It does not guarantee parity
with upstream unless the differential parity workflow is run in an environment that also has the
reference tools and assets installed.

## Project Status

The repo is already beyond a toy prototype. The crate boundaries, parity harness, requirement spec,
and deterministic selection paths show a deliberate structure for an upstream-compatible port.

The main gaps are the normal ones for a compatibility project:

- Broadening parity case coverage.
- Validating behavior against more real-world asset sets.
- Closing any remaining differences from `ponysay` and `ponythink`.

## License

MIT. See `LICENSE`.
