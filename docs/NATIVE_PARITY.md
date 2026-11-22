# KloksRS native parity — pinned source

Reference: `dinilmusick/Kloks@286377a03a8c5973c592b61ccaaa3864d060e8cc`.
Native references: `GutRS@3e4ad1823166b37ddc437aab04277b42c124ce36`, `KrnlRS@e93cd570a41e58157bd4b55362c18124167622a4`, `FrnlRS@4325cf84fdad2a51ea41c10bf049342fee2a7327`.

| TypeScript source | Rust surface | Status / adaptation |
| --- | --- | --- |
| `audio-midi/src/clock/*` | `ClockService` | Implemented (clock PR #1); host calls `poll`; no resident timer |
| `audio-midi/src/engine/midi/midiManager/*` | `midi` | PR #2; legacy 1-based channels; physical I/O intentionally absent |
| `services/src/index.ts`, manifests | `registry::ServiceRegistry` | Typed definitions, deterministic dependencies, lifecycle and discovery; no duplicated KrnlRS executor |
| `state-graph/src/*` | `graph::DirectedGraph` | Native graph dependency ordering, cycle prevention; browser rendering intentionally excluded |
| `orchestrations/src/*` | `graph::compile_plan` | Serializable-style plan validation; execution delegated to host/KrnlRS |
| `audio-midi/src/osc/codec.ts` | pending OSC module | Binary codec, message/bundle and transport implementation remains required |
| `audio-midi/src/midi/message.ts` | pending strict MIDI module | Separate modern 0-based wire codec required, distinct from browser legacy 1-based events |
| `backend/src/*` | pending adapters | Broader provider facades/storage/credentials intentionally not represented as implemented |
| Audio DSP, graphical nodes | pending | Not part of renderer-neutral portable core yet |

No physical MIDI, OSC UDP socket or autonomous scheduler is supplied by this crate. All host lifecycle operations are explicit. The allocation-only graph and registry modules avoid networking/threads; portability should be judged by CI feature-disabled checks, not just this table.
