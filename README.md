# KloksRS

Renderer-neutral Rust services adapted from [Kloks](https://github.com/dinilmusick/Kloks). **Canonical TypeScript source:** `dinilmusick/Kloks@286377a03a8c5973c592b61ccaaa3864d060e8cc`. See [the pinned native parity matrix](docs/NATIVE_PARITY.md).

## Merged portable core

- `ClockService` — deterministic tempo, ticks, time signatures, pause/resume/stop, missed-tick policy and externally supplied monotonic time. There are no resident timers.
- `midi` — legacy event compatibility (1-based channel numbers); `midi_protocol` — strict 0-based MIDI wire codec, pitch bend, system packets and validation.
- `osc` — binary OSC messages and nested bundles, typed arguments and bounded decoding; a host-provided outbound transport trait.
- `registry` — versioned service definitions, capability discovery, deterministic dependencies and explicit lifecycle.
- `graph` — directed dependency graph, cycle prevention and orchestration plan validation. A host executes plans; KloksRS does not spawn work.

The above is merged on `main` at `9a6bc7f0f30491f829cff454eadff8a2ba3b55e9`, with [green main CI](https://github.com/dinilmusick/KloksRS/actions/runs/45008554204). A later `main` commit may incorporate additional work.

## Host adapter candidate (PR #9)

The `feat/kloksrs-wave3-host-adapters-20261122` branch proposes: `host` traits for clock time, bounded filesystem and network exchange; `midi_service` for injected physical device discovery and logical MIDI endpoints; `osc_service` for injected OSC listener start/stop, subscription matching and bounded, host-drained dispatch.

**These candidate interfaces are not certified as merged until [PR #9](https://github.com/dinilmusick/KloksRS/pull/9) passes exact-head CI and is safely merged.** The crate never provides real MIDI hardware, filesystem access, UDP sockets or resident schedulers unless a platform-specific host implements the corresponding port.

## Build and consumer checks

```sh
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo check --no-default-features --lib
cargo clippy --no-default-features --lib -- -D warnings
cargo run --manifest-path consumer-smoke/Cargo.toml --offline
cargo clippy --manifest-path consumer-smoke/Cargo.toml --all-targets --offline -- -D warnings
```

The `consumer-smoke` crate is a separate path-dependent Rust consumer. `no_std` with `alloc` checks the portable library code; it does **not** imply bare-metal hardware support.

## Explicit non-parity

KloksRS is **not** a complete Rust reimplementation of the Kloks monorepo. Browser Web MIDI integration, hardware MIDI/OSC transport providers, audio DSP and synthesizer engines, full spatial/visual graph semantics, backend storage/cloud/credentials providers and some OSC address-pattern semantics require separate implementation and verification. Consult the [parity matrix](docs/NATIVE_PARITY.md) before claiming feature support.
