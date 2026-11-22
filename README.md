# KloksRS

Renderer-neutral Rust services adapted from [Kloks](https://github.com/dinilmusick/Kloks). **Canonical TypeScript source:** `dinilmusick/Kloks@286377a03a8c5973c592b61ccaaa3864d060e8cc`. See [the pinned native parity matrix](docs/NATIVE_PARITY.md).

## Merged portable core

- `ClockService` — deterministic tempo, ticks, time signatures, pause/resume/stop, missed-tick policy and externally supplied monotonic time. There are no resident timers.
- `midi` — legacy event compatibility (1-based channel numbers); `midi_protocol` — strict 0-based MIDI wire codec, pitch bend, system packets and validation.
- `osc` — binary OSC messages and nested bundles, typed arguments and bounded decoding; a host-provided outbound transport trait.
- `registry` — versioned service definitions, capability discovery, deterministic dependencies and explicit lifecycle.
- `graph` — directed dependency graph, cycle prevention and orchestration plan validation. A host executes plans; KloksRS does not spawn work.

The above is merged on `main` at `9a6bc7f0f30491f829cff454eadff8a2ba3b55e9`, with [green main CI](https://github.com/dinilmusick/KloksRS/actions/runs/45008554204). A later `main` commit may incorporate additional work.

## Host adapters merged (PR #9)

The `host` module supplies explicit injected clock time, bounded filesystem and network ports. `midi_service` provides host-discovered physical devices, in-process logical endpoints, input/output connection, SysEx gating and bounded event delivery. `osc_service` provides injected OSC server lifecycle, client sends, wildcard subscriptions and bounded host-drained dispatch. **Actual hardware, UDP/network sockets and storage remain responsibilities of a platform host.**

These interfaces were merged in [PR #9](https://github.com/dinilmusick/KloksRS/pull/9), with exact-head [PR CI 45018013054](https://github.com/dinilmusick/KloksRS/actions/runs/45018013054) and merge commit `891e227cd6f857e72005dd15ff9a0a3c74250ab1`. The separate `consumer-smoke` package exercises these public host APIs in addition to the clock and MIDI/OSC codecs.

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
