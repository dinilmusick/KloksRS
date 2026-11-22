use kloksrs::midi::{control_change, decode_channel_event, note_off, note_on, MidiEventKind};
use kloksrs::{ClockOptions, ClockService};

#[test]
fn externally_visible_clock_and_legacy_midi_work_together() {
    let mut clock = ClockService::new(ClockOptions::default()).unwrap();
    clock.start(0.0).unwrap();
    assert_eq!(clock.poll(500.0).unwrap().len(), 24);
    let raw = note_on(1, 60, 90);
    let note = decode_channel_event(&raw).unwrap();
    assert_eq!(note.kind, MidiEventKind::NoteOn);
    assert_eq!(note.channel, 1);
    assert_eq!(note.note_name, "C4");
    assert!(note.velocity > 0.0);
    let off = decode_channel_event(&note_off(1, 60)).unwrap();
    assert_eq!(off.kind, MidiEventKind::NoteOff);
    assert_eq!(control_change(2, 74, 100), [0xB1, 74, 100]);
    assert_eq!(clock.pause(500.0).unwrap().len(), 0);
}
