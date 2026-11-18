//! Pure MIDI event interpretation and outgoing 3-byte channel message encoding.
//! Semantics adapted from Kloks audio-midi MidiManager (commit
//! 67de26dd39161a0bc642038fdca75dcdfee8fae7).
//! Does not request hardware access, spawn listeners or emit to endpoints.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MidiEventKind {
    NoteOn,
    NoteOff,
    ControlChange,
    PitchBend,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MidiEvent {
    pub kind: MidiEventKind,
    /// MIDI channel, one-based, matching Kloks' browser MidiManager.
    pub channel: u8,
    pub note: u8,
    pub note_name: String,
    /// Note-on velocity is normalized 0.0..=1.0; all other received
    /// event kinds use zero like the Kloks event decoder.
    pub velocity: f64,
    pub controller: Option<u8>,
    pub value: Option<i16>,
}

/// Mirrors Kloks' MIDI byte decoder for supported channel-voice commands.
/// Unsupported commands and incomplete (<2 byte) data produce no event.
/// Note-on velocity zero becomes note-off.
pub fn decode_channel_event(data: &[u8]) -> Option<MidiEvent> {
    if data.len() < 2 {
        return None;
    }
    let status = data[0];
    let command = status >> 4;
    let channel = (status & 0x0f) + 1;
    let note = data[1];
    let raw_velocity = data.get(2).copied().unwrap_or(0);
    let (kind, decoded_note, velocity, controller, value) = match command {
        0x9 if raw_velocity > 0 => (
            MidiEventKind::NoteOn,
            note,
            f64::from(raw_velocity) / 127.0,
            None,
            None,
        ),
        0x8 | 0x9 => (MidiEventKind::NoteOff, note, 0.0, None, None),
        0xB => (
            MidiEventKind::ControlChange,
            0,
            0.0,
            Some(note),
            Some(i16::from(raw_velocity)),
        ),
        0xE => (
            MidiEventKind::PitchBend,
            0,
            0.0,
            None,
            Some(((i16::from(raw_velocity)) << 7) | i16::from(note)),
        ),
        _ => return None,
    };
    let value = if kind == MidiEventKind::PitchBend {
        value.map(|raw| raw - 8192)
    } else {
        value
    };
    Some(MidiEvent {
        kind,
        channel,
        note: decoded_note,
        note_name: if kind == MidiEventKind::NoteOn || kind == MidiEventKind::NoteOff {
            midi_note_name(note)
        } else {
            String::new()
        },
        velocity,
        controller,
        value,
    })
}

pub fn midi_note_name(note: u8) -> String {
    const NOTES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let pitch = NOTES[usize::from(note % 12)];
    let octave = i16::from(note / 12) - 1;
    format!("{pitch}{octave}")
}

fn clamp_data(value: i32) -> u8 {
    value.clamp(0, 127) as u8
}
fn channel_status(command: u8, channel: i32) -> u8 {
    command | ((channel - 1).clamp(0, 15) as u8)
}

/// Match the on-wire clamping used by Kloks MidiManager.
pub fn note_on(channel: i32, note: i32, velocity: i32) -> [u8; 3] {
    [
        channel_status(0x90, channel),
        clamp_data(note),
        clamp_data(velocity),
    ]
}
pub fn note_off(channel: i32, note: i32) -> [u8; 3] {
    [channel_status(0x80, channel), clamp_data(note), 0]
}
pub fn control_change(channel: i32, controller: i32, value: i32) -> [u8; 3] {
    [
        channel_status(0xB0, channel),
        clamp_data(controller),
        clamp_data(value),
    ]
}
