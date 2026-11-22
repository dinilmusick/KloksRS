//! Strict modern MIDI message codec from Kloks audio-midi/src/midi/message.ts.
//! Uses 0-based channels; the legacy MidiManager compatibility API in midi.rs is 1-based.
extern crate alloc;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelKind {
    NoteOff,
    NoteOn,
    PolyPressure,
    ControlChange,
    ProgramChange,
    ChannelPressure,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MidiMessage {
    Channel {
        kind: ChannelKind,
        channel: u8,
        data: u8,
        value: u8,
    },
    PitchBend {
        channel: u8,
        value: i16,
    },
    System {
        status: u8,
        data: Vec<u8>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MidiError {
    MissingStatus,
    InvalidStatus,
    InvalidChannel,
    InvalidData,
    InvalidLength,
    InvalidPitchBend,
    SystemTooLarge,
}

const MAX_SYSTEM_BYTES: usize = 4096;

fn channel_status(channel: u8, family: u8) -> Result<u8, MidiError> {
    if channel > 15 {
        return Err(MidiError::InvalidChannel);
    }
    Ok(family | channel)
}

fn seven_bit(value: u8) -> Result<u8, MidiError> {
    if value > 127 {
        Err(MidiError::InvalidData)
    } else {
        Ok(value)
    }
}

pub fn encode(message: &MidiMessage) -> Result<Vec<u8>, MidiError> {
    match message {
        MidiMessage::System { status, data } => {
            if *status < 0xf0 {
                return Err(MidiError::InvalidStatus);
            }
            if data.len() > MAX_SYSTEM_BYTES {
                return Err(MidiError::SystemTooLarge);
            }
            let mut bytes = Vec::with_capacity(data.len() + 1);
            bytes.push(*status);
            bytes.extend_from_slice(data);
            Ok(bytes)
        }
        MidiMessage::PitchBend { channel, value } => {
            if !(-8192..=8191).contains(value) {
                return Err(MidiError::InvalidPitchBend);
            }
            let bend = i32::from(*value) + 8192;
            Ok(alloc::vec![
                channel_status(*channel, 0xe0)?,
                (bend & 0x7f) as u8,
                ((bend >> 7) & 0x7f) as u8
            ])
        }
        MidiMessage::Channel {
            kind,
            channel,
            data,
            value,
        } => {
            let family = match kind {
                ChannelKind::NoteOff => 0x80,
                ChannelKind::NoteOn => 0x90,
                ChannelKind::PolyPressure => 0xa0,
                ChannelKind::ControlChange => 0xb0,
                ChannelKind::ProgramChange => 0xc0,
                ChannelKind::ChannelPressure => 0xd0,
            };
            let mut bytes = alloc::vec![channel_status(*channel, family)?, seven_bit(*data)?];
            if !matches!(kind, ChannelKind::ProgramChange | ChannelKind::ChannelPressure) {
                bytes.push(seven_bit(*value)?);
            }
            Ok(bytes)
        }
    }
}

pub fn decode(input: &[u8]) -> Result<MidiMessage, MidiError> {
    let status = *input.first().ok_or(MidiError::MissingStatus)?;
    if status < 0x80 {
        return Err(MidiError::InvalidStatus);
    }
    if status >= 0xf0 {
        if input.len() - 1 > MAX_SYSTEM_BYTES {
            return Err(MidiError::SystemTooLarge);
        }
        return Ok(MidiMessage::System {
            status,
            data: input[1..].to_vec(),
        });
    }
    let family = status & 0xf0;
    let channel = status & 0x0f;
    let expected = if family == 0xc0 || family == 0xd0 { 2 } else { 3 };
    if input.len() != expected {
        return Err(MidiError::InvalidLength);
    }
    for value in &input[1..] {
        seven_bit(*value)?;
    }
    if family == 0xe0 {
        return Ok(MidiMessage::PitchBend {
            channel,
            value: (i16::from(input[2]) << 7 | i16::from(input[1])) - 8192,
        });
    }
    let kind = match family {
        0x80 => ChannelKind::NoteOff,
        0x90 => ChannelKind::NoteOn,
        0xa0 => ChannelKind::PolyPressure,
        0xb0 => ChannelKind::ControlChange,
        0xc0 => ChannelKind::ProgramChange,
        0xd0 => ChannelKind::ChannelPressure,
        _ => return Err(MidiError::InvalidStatus),
    };
    Ok(MidiMessage::Channel {
        kind,
        channel,
        data: input[1],
        value: if expected == 3 { input[2] } else { 0 },
    })
}

/// Explicit host interface. Actual physical devices are outside this crate.
pub trait MidiSink {
    type Error;
    fn send(&mut self, device_id: &str, bytes: &[u8]) -> Result<(), Self::Error>;
}

pub fn send_message<S: MidiSink>(
    sink: &mut S,
    device_id: &str,
    message: &MidiMessage,
) -> Result<(), SendError<S::Error>> {
    let bytes = encode(message).map_err(SendError::Codec)?;
    sink.send(device_id, &bytes).map_err(SendError::Host)
}

#[derive(Debug)]
pub enum SendError<E> {
    Codec(MidiError),
    Host(E),
}
