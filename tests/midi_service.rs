use kloksrs::midi_protocol::{ChannelKind, MidiMessage};
use kloksrs::midi_service::{
    Direction, MidiCapabilities, MidiDevice, MidiDeviceService, MidiHost, MidiServiceError, Origin,
};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
struct State {
    sent: Vec<Vec<u8>>,
    disconnected: Vec<String>,
    disposed: bool,
}
#[derive(Clone)]
struct FakeHost {
    shared: Rc<RefCell<State>>,
    devices: Vec<MidiDevice>,
}
impl MidiHost for FakeHost {
    type Error = &'static str;
    fn capabilities(&self) -> MidiCapabilities {
        MidiCapabilities {
            physical_input: true,
            physical_output: true,
            system_exclusive: false,
        }
    }
    fn discover(&mut self) -> Result<Vec<MidiDevice>, Self::Error> {
        Ok(self.devices.clone())
    }
    fn connect_input(&mut self, _: &str) -> Result<(), Self::Error> {
        Ok(())
    }
    fn connect_output(&mut self, _: &str) -> Result<(), Self::Error> {
        Ok(())
    }
    fn disconnect(&mut self, id: &str) -> Result<(), Self::Error> {
        self.shared.borrow_mut().disconnected.push(id.to_string());
        Ok(())
    }
    fn send(&mut self, _: &str, bytes: &[u8]) -> Result<(), Self::Error> {
        self.shared.borrow_mut().sent.push(bytes.to_vec());
        Ok(())
    }
    fn dispose(&mut self) -> Result<(), Self::Error> {
        self.shared.borrow_mut().disposed = true;
        Ok(())
    }
}
fn note() -> MidiMessage {
    MidiMessage::Channel {
        kind: ChannelKind::NoteOn,
        channel: 0,
        data: 60,
        value: 100,
    }
}
fn physical() -> MidiDevice {
    MidiDevice {
        id: "device".into(),
        name: "Injected".into(),
        direction: Direction::Duplex,
        origin: Origin::Physical,
        input_connected: false,
        output_connected: false,
    }
}
#[test]
fn physical_connect_send_receive_and_dispose_are_host_owned() {
    let shared = Rc::new(RefCell::new(State::default()));
    let backend = FakeHost {
        shared: shared.clone(),
        devices: vec![physical()],
    };
    let mut svc = MidiDeviceService::new(backend, 4).unwrap();
    assert_eq!(svc.refresh().unwrap().len(), 1);
    assert_eq!(
        svc.send("device", note()),
        Err(MidiServiceError::NotConnected)
    );
    svc.connect_input("device").unwrap();
    svc.connect_output("device").unwrap();
    svc.send("device", note()).unwrap();
    assert_eq!(shared.borrow().sent, vec![vec![0x90, 60, 100]]);
    svc.receive("device", &[0x90, 60, 100]).unwrap();
    assert_eq!(svc.next_event().unwrap().message, note());
    assert_eq!(
        svc.send(
            "device",
            MidiMessage::System {
                status: 0xf0,
                data: vec![1, 2, 0xf7],
            }
        ),
        Err(MidiServiceError::SysExDenied)
    );
    svc.dispose().unwrap();
    assert!(shared.borrow().disposed);
    assert_eq!(shared.borrow().disconnected, vec!["device"]);
    assert_eq!(svc.refresh(), Err(MidiServiceError::Disposed));
}
#[test]
fn logical_queue_is_bounded_and_no_physical_host_is_required() {
    let shared = Rc::new(RefCell::new(State::default()));
    let mut svc = MidiDeviceService::new(
        FakeHost {
            shared: shared.clone(),
            devices: vec![],
        },
        1,
    )
    .unwrap();
    let logical = svc.create_logical("Internal", Direction::Duplex).unwrap();
    assert_eq!(logical.id, "logical-midi-1");
    svc.send(&logical.id, note()).unwrap();
    assert_eq!(
        svc.send(&logical.id, note()),
        Err(MidiServiceError::QueueFull)
    );
    assert_eq!(svc.next_event().unwrap().device_id, logical.id);
    assert_eq!(svc.next_event(), None);
    svc.disconnect(&logical.id).unwrap();
    assert_eq!(
        svc.send(&logical.id, note()),
        Err(MidiServiceError::NotConnected)
    );
    assert!(shared.borrow().sent.is_empty());
}
#[test]
fn duplicate_discovery_fails_without_mutating_registry() {
    let dev = physical();
    let mut svc = MidiDeviceService::new(
        FakeHost {
            shared: Rc::new(RefCell::new(State::default())),
            devices: vec![dev.clone(), dev],
        },
        2,
    )
    .unwrap();
    assert_eq!(svc.refresh(), Err(MidiServiceError::InvalidDevice));
    assert!(svc.devices().is_empty());
}
