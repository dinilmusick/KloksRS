use kloksrs::midi_protocol::{
    decode, encode, send_message, ChannelKind, MidiError, MidiMessage, MidiSink,
};

#[test]
fn modern_codec_uses_strict_zero_based_channel_semantics() {
    let msg = MidiMessage::Channel {
        kind: ChannelKind::NoteOn,
        channel: 15,
        data: 60,
        value: 0,
    };
    assert_eq!(encode(&msg).unwrap(), vec![0x9f, 60, 0]);
    assert_eq!(decode(&[0x9f, 60, 0]).unwrap(), msg);
    assert_eq!(
        decode(&[0xc2, 23]).unwrap(),
        MidiMessage::Channel {
            kind: ChannelKind::ProgramChange,
            channel: 2,
            data: 23,
            value: 0
        }
    );
    assert_eq!(
        decode(&[0xd0, 127]).unwrap(),
        MidiMessage::Channel {
            kind: ChannelKind::ChannelPressure,
            channel: 0,
            data: 127,
            value: 0
        }
    );
}

#[test]
fn pitch_bend_and_system_status_round_trip() {
    for value in [-8192, -1, 0, 8191] {
        let msg = MidiMessage::PitchBend { channel: 0, value };
        assert_eq!(decode(&encode(&msg).unwrap()).unwrap(), msg);
    }
    let sysex = MidiMessage::System {
        status: 0xf0,
        data: vec![0, 127, 255, 0xf7],
    };
    assert_eq!(decode(&encode(&sysex).unwrap()).unwrap(), sysex);
}

#[test]
fn malformed_messages_fail_closed() {
    assert_eq!(decode(&[]), Err(MidiError::MissingStatus));
    assert_eq!(decode(&[0x70, 10, 20]), Err(MidiError::InvalidStatus));
    assert_eq!(decode(&[0x90, 10]), Err(MidiError::InvalidLength));
    assert_eq!(decode(&[0x90, 10, 20, 30]), Err(MidiError::InvalidLength));
    assert_eq!(decode(&[0x90, 10, 200]), Err(MidiError::InvalidData));
    assert_eq!(
        encode(&MidiMessage::PitchBend {
            channel: 0,
            value: 8192
        }),
        Err(MidiError::InvalidPitchBend)
    );
    assert_eq!(
        encode(&MidiMessage::Channel {
            kind: ChannelKind::NoteOn,
            channel: 16,
            data: 20,
            value: 1
        }),
        Err(MidiError::InvalidChannel)
    );
}

#[derive(Default)]
struct MemorySink(Vec<u8>);
impl MidiSink for MemorySink {
    type Error = ();
    fn send(&mut self, _device_id: &str, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}
#[test]
fn transport_is_host_injected_and_no_hardware_claim_is_made() {
    let mut sink = MemorySink::default();
    let msg = MidiMessage::Channel {
        kind: ChannelKind::ControlChange,
        channel: 1,
        data: 74,
        value: 90,
    };
    send_message(&mut sink, "logical-device", &msg).unwrap();
    assert_eq!(sink.0, [0xb1, 74, 90]);
}
