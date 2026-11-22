use kloksrs::midi::*;

#[test]
fn note_on_and_zero_velocity_note_off_follow_kloks_decoder() {
    let on = decode_channel_event(&[0x90, 60, 127]).unwrap();
    assert_eq!(on.kind, MidiEventKind::NoteOn);
    assert_eq!(on.channel, 1);
    assert_eq!(on.note, 60);
    assert_eq!(on.note_name, "C4");
    assert_eq!(on.velocity, 1.0);
    let off = decode_channel_event(&[0x9F, 60, 0]).unwrap();
    assert_eq!(off.kind, MidiEventKind::NoteOff);
    assert_eq!(off.channel, 16);
    assert_eq!(off.velocity, 0.0);
    assert_eq!(
        decode_channel_event(&[0x80, 36, 100]).unwrap().note_name,
        "C2"
    );
}

#[test]
fn cc_pitch_bend_and_unsupported_messages() {
    let cc = decode_channel_event(&[0xB2, 74, 100]).unwrap();
    assert_eq!(cc.kind, MidiEventKind::ControlChange);
    assert_eq!(cc.channel, 3);
    assert_eq!(cc.controller, Some(74));
    assert_eq!(cc.value, Some(100));
    assert_eq!(cc.note, 0);
    assert_eq!(cc.note_name, "");
    let center = decode_channel_event(&[0xE0, 0, 64]).unwrap();
    assert_eq!(center.kind, MidiEventKind::PitchBend);
    assert_eq!(center.value, Some(0));
    assert_eq!(
        decode_channel_event(&[0xE0, 0, 0]).unwrap().value,
        Some(-8192)
    );
    assert_eq!(
        decode_channel_event(&[0xEF, 127, 127]).unwrap().value,
        Some(8191)
    );
    assert_eq!(decode_channel_event(&[0xC0, 10]), None);
    assert_eq!(decode_channel_event(&[]), None);
    assert_eq!(decode_channel_event(&[0x90]), None);
}

#[test]
fn note_and_control_packets_are_bounded_and_channel_one_based() {
    assert_eq!(note_on(1, 60, 127), [0x90, 60, 127]);
    assert_eq!(note_on(16, 61, 0), [0x9F, 61, 0]);
    assert_eq!(note_on(0, -1, 300), [0x90, 0, 127]);
    assert_eq!(note_off(22, 256), [0x8F, 127, 0]);
    assert_eq!(control_change(2, 74, 200), [0xB1, 74, 127]);
    assert_eq!(control_change(-99, -5, -1), [0xB0, 0, 0]);
    assert_eq!(midi_note_name(0), "C-1");
    assert_eq!(midi_note_name(127), "G9");
}
