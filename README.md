# KloksRS

ULTRST canonical Rust backend and transport counterpart.

## Deterministic musical clock — first parity increment

Implements Kloks `ClockService` timing, state transitions, 1–960 tick resolution,
tempo changes, bar/beat/subdivision positioning, catch-up/skip missed-tick policy,
capped catch-up, and errors with native tests.

Pinned reference: `dinilmusick/Kloks@67de26dd39161a0bc642038fdca75dcdfee8fae7`,
`packages/audio-midi/src/clock/{clock,kernel,scheduler}.ts`.

The native API is **externally advanced**: a host runtime supplies monotonic milliseconds
to `start`, `poll`, `pause`, `resume`, `stop`, etc. KloksRS does not
spawn threads, timers or resident agents. A host may call `poll` whenever
its scheduler wakes; the returned ticks carry scheduled positions and missed tick counts.
This is intentional idiomatic Rust adaptation to native/event-loop environments.

**Not yet ported:** clock callback subscriptions, full MIDI/OSC managers, network
transport, DSP engines, Kloks backend facades, credentials, runtime consumer integration.
This PR is not a claim of full Kloks Rust functional parity.

## MIDI event compatibility (stacked change)

Native `midi` module decodes note-on, note-off, control change and pitch bend
channel events and builds clamped three-byte outgoing MIDI messages. It tracks
Kloks' 1-based channel and zero-velocity note-off semantics.

Upstream pin: `packages/audio-midi/src/engine/midi/midiManager/midiManager.ts`
under `dinilmusick/Kloks@67de26dd39161a0bc642038fdca75dcdfee8fae7`.
This is a pure packet API; it does not replace Kloks' physical MIDI manager,
Web MIDI device discovery, network transports or canonical service facades.
