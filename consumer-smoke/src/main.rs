use kloksrs::{ClockOptions, ClockRunState, ClockService};

fn main() {
    let mut clock = ClockService::new(ClockOptions::default()).expect("valid options");
    assert_eq!(clock.snapshot(0.0).unwrap().state, ClockRunState::Stopped);
    clock.start(10.0).unwrap();
    let ticks = clock.poll(510.0).unwrap();
    assert_eq!(ticks.len(), 24);
    assert_eq!(ticks.last().unwrap().position.tick, 24);
    assert_eq!(clock.pause(510.0).unwrap().len(), 0);
    assert_eq!(clock.snapshot(1010.0).unwrap().tick, 24);
    clock.resume(1010.0).unwrap();
    assert_eq!(clock.poll(1510.0).unwrap().len(), 24);
    assert_eq!(clock.stop(1510.0).unwrap().len(), 0);
    clock.dispose();
    assert!(clock.start(1600.0).is_err());

    let bend = kloksrs::midi_protocol::MidiMessage::PitchBend {
        channel: 0,
        value: 8191,
    };
    let bend_bytes = kloksrs::midi_protocol::encode(&bend).expect("MIDI encode");
    assert_eq!(
        kloksrs::midi_protocol::decode(&bend_bytes).expect("MIDI decode"),
        bend
    );

    let packet = kloksrs::osc::OscPacket::Message(kloksrs::osc::OscMessage {
        address: "/native/clock".into(),
        args: vec![kloksrs::osc::OscArgument::Int(24)],
    });
    let packet_bytes = kloksrs::osc::encode(&packet).expect("OSC encode");
    assert_eq!(
        kloksrs::osc::decode(&packet_bytes).expect("OSC decode"),
        packet
    );
}
