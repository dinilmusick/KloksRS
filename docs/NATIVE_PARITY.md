# KloksRS native parity — pinned authoritative source

Canonical: `dinilmusick/Kloks@286377a03a8c5973c592b61ccaaa3864d060e8cc`.
Native foundations examined: `GutRS@3e4ad1823166b37ddc437aab04277b42c124ce36`, `KrnlRS@e93cd570a41e58157bd4b55362c18124167622a4`, `FrnlRS@4325cf84fdad2a51ea41c10bf049342fee2a7327`.

| TypeScript source under packages/ | Rust API | Status and boundary |
| --- | --- | --- |
| `audio-midi/src/clock/{clock,scheduler}.ts` | `ClockService` | **Merged** deterministic polling and catch-up/skip ticks; scheduler belongs to host |
| `audio-midi/src/engine/midi/midiManager/*` | `midi` | **Merged subset** legacy one-based note/CC/pitch event compatibility |
| `audio-midi/src/midi/message.ts` | `midi_protocol` | **Merged** strict zero-based channel wire codec, message validation and pitch bend |
| `audio-midi/src/midi/service.ts` | `midi_service` | **PR #9 candidate** host-injected discovered physical and logical devices, explicit connect/disconnect, SysEx gate |
| `audio-midi/src/osc/codec.ts` | `osc` | **Merged** typed OSC messages, nested bundles, bounded decoder |
| `audio-midi/src/osc/service.ts` | `osc_service` | **PR #9 candidate** host-driven server lifecycle, message routing, basic wildcard `*`/`?` matching |
| `audio-midi/src/osc/node-udp.ts` | `OscHost`/`OscTransport` | **Interface only**; no UDP socket or WebSocket implementation supplied |
| `services/src/index.ts` | `registry::ServiceRegistry` | **Merged subset** typed definitions, deterministic dependency resolution, lifecycle and capability discovery |
| `state-graph/src/{domain,engine,rules,spatial}` | `graph::DirectedGraph` | **Merged subset** graph edges, cycles and ordering; visual/spatial nodes remain outstanding |
| `orchestrations/src/*` | `graph::compile_plan` | **Merged subset** plan validation; execution delegated to host/KrnlRS |
| `backend/src/{api,storage,connectivity,provider-services}` | `host::{FileSystemPort,NetworkPort,MonotonicTime}` | **PR #9 candidate** narrow host ports, not full providers or credentials |
| `audio-midi/src/engine/audio/*` | — | **Outstanding** audio DSP, real-time processing, host streams and synthesis |

## Verified boundaries

Merged core PRs [#1](https://github.com/dinilmusick/KloksRS/pull/1), [#2](https://github.com/dinilmusick/KloksRS/pull/2), [#5](https://github.com/dinilmusick/KloksRS/pull/5), [#7](https://github.com/dinilmusick/KloksRS/pull/7) were tested at exact heads. Core `main` commit `9a6bc7f0f30491f829cff454eadff8a2ba3b55e9` passed [GitHub Actions 45008554204](https://github.com/dinilmusick/KloksRS/actions/runs/45008554204): rustfmt, all-target tests, Clippy, no-default-features library check/Clippy and external path consumer.

**PR #9 is not yet verified by those historical green runs.** It needs its own exact-head checks and independent readback before any merge.

### Remaining bounded work

1. Verify and safely merge PR #9 (host traits, MIDI discovery/connect/disconnect, OSC listener lifecycle, failure cleanup and external consumer); update its status only after evidence.
2. Complete canonical OSC address-pattern expressions (`[]`, `{a,b}`, escapes), subscription/error callback parity.
3. Add real host-specific MIDI hardware, network and device hot-plug adapters where a concrete platform is chosen.
4. Implement remaining backend/cloud/provider facades, audio DSP, visual/spatial graph and service runtime exports as distinct parity goals with source pins and tests.

**Native adaptation:** all external I/O is injected; there are no hidden threads, sockets, timers or resident agents. `no_std` + `alloc` validates portable semantics, not hardware drivers.
