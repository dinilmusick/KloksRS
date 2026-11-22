use kloksrs::host::{FileSystemPort, MonotonicTime, NetworkClient, NetworkPort, ResourceClient, poll_with_host};
use kloksrs::midi_protocol::{ChannelKind, MidiMessage};
use kloksrs::midi_service::{Direction, MidiCapabilities, MidiDevice, MidiDeviceService, MidiHost};
use kloksrs::osc::{self, OscEndpoint, OscPacket};
use kloksrs::osc_service::{OscHost, OscService};
use kloksrs::{ClockOptions, ClockService};

struct NoPhysicalDevices;
impl MidiHost for NoPhysicalDevices {
    type Error = ();
    fn capabilities(&self) -> MidiCapabilities {
        MidiCapabilities { physical_input: false, physical_output: false, system_exclusive: false }
    }
    fn discover(&mut self) -> Result<Vec<MidiDevice>, Self::Error> { Ok(vec![]) }
    fn connect_input(&mut self, _: &str) -> Result<(), Self::Error> { Ok(()) }
    fn connect_output(&mut self, _: &str) -> Result<(), Self::Error> { Ok(()) }
    fn disconnect(&mut self, _: &str) -> Result<(), Self::Error> { Ok(()) }
    fn send(&mut self, _: &str, _: &[u8]) -> Result<(), Self::Error> { Ok(()) }
    fn dispose(&mut self) -> Result<(), Self::Error> { Ok(()) }
}
struct NoSocket;
impl osc::OscTransport for NoSocket {
    type Error = ();
    fn send(&mut self, _: &OscEndpoint, _: &[u8]) -> Result<(), Self::Error> { Ok(()) }
}
impl OscHost for NoSocket {
    fn listen(&mut self, _: &OscEndpoint) -> Result<(), Self::Error> { Ok(()) }
    fn stop(&mut self, _: &OscEndpoint) -> Result<(), Self::Error> { Ok(()) }
    fn dispose(&mut self) -> Result<(), Self::Error> { Ok(()) }
}
#[derive(Default)]
struct MemoryFile(Option<Vec<u8>>);
impl FileSystemPort for MemoryFile {
    type Error = ();
    fn read(&mut self, _: &str, _: usize) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.0.clone())
    }
    fn write(&mut self, _: &str, data: &[u8]) -> Result<(), Self::Error> {
        self.0 = Some(data.to_vec());
        Ok(())
    }
    fn delete(&mut self, _: &str) -> Result<bool, Self::Error> {
        Ok(self.0.take().is_some())
    }
}
struct Loopback;
impl NetworkPort for Loopback {
    type Error = ();
    fn exchange(&mut self, _: &str, request: &[u8], _: usize) -> Result<Vec<u8>, Self::Error> {
        Ok(request.to_vec())
    }
}
struct Time(f64);
impl MonotonicTime for Time {
    fn now_millis(&self) -> f64 { self.0 }
}
pub fn run() {
    let mut midi = MidiDeviceService::new(NoPhysicalDevices, 2).unwrap();
    assert!(midi.refresh().unwrap().is_empty());
    let logical = midi.create_logical("consumer", Direction::Duplex).unwrap();
    let message = MidiMessage::Channel {
        kind: ChannelKind::NoteOn, channel: 0, data: 60, value: 100,
    };
    midi.send(&logical.id, message.clone()).unwrap();
    assert_eq!(midi.next_event().unwrap().message, message);
    midi.dispose().unwrap();

    let mut osc_service = OscService::new(NoSocket, 2).unwrap();
    let server = OscEndpoint { id: "mock".into(), host: "host".into(), port: 1234 };
    osc_service.start_server(server).unwrap();
    let id = osc_service.subscribe("/native/*").unwrap();
    let packet = OscPacket::Message(osc::OscMessage { address: "/native/test".into(), args: vec![] });
    assert_eq!(osc_service.receive("mock", &osc::encode(&packet).unwrap()).unwrap(), 1);
    assert_eq!(osc_service.next_event().unwrap().subscription_ids, vec![id]);
    osc_service.dispose().unwrap();

    let mut files = ResourceClient::new(MemoryFile::default(), 64).unwrap();
    files.write("test/data", &[1, 2, 3]).unwrap();
    assert_eq!(files.read("test/data").unwrap(), Some(vec![1, 2, 3]));
    assert!(files.delete("test/data").unwrap());

    let mut network = NetworkClient::new(Loopback, 16).unwrap();
    assert_eq!(network.exchange("loopback", &[5, 4]).unwrap(), vec![5, 4]);

    let mut clock = ClockService::new(ClockOptions::default()).unwrap();
    clock.start(0.0).unwrap();
    assert_eq!(poll_with_host(&mut clock, &Time(500.0)).unwrap().len(), 24);
}
