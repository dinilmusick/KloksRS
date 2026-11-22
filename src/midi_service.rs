//! Host-supplied MIDI device discovery, independent port lifecycle and logical routing.
//! Native counterpart of Kloks audio-midi/src/midi/service.ts.
use alloc::collections::{BTreeMap, VecDeque};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use crate::midi_protocol::{self, MidiError, MidiMessage};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction { Input, Output, Duplex }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin { Physical, Logical }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiDevice {
    pub id: String,
    pub name: String,
    pub direction: Direction,
    pub origin: Origin,
    pub input_connected: bool,
    pub output_connected: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MidiCapabilities {
    pub physical_input: bool,
    pub physical_output: bool,
    pub system_exclusive: bool,
}
pub trait MidiHost {
    type Error;
    fn capabilities(&self) -> MidiCapabilities;
    fn discover(&mut self) -> Result<Vec<MidiDevice>, Self::Error>;
    fn connect_input(&mut self, id: &str) -> Result<(), Self::Error>;
    fn connect_output(&mut self, id: &str) -> Result<(), Self::Error>;
    fn disconnect(&mut self, id: &str) -> Result<(), Self::Error>;
    fn send(&mut self, id: &str, bytes: &[u8]) -> Result<(), Self::Error>;
    fn dispose(&mut self) -> Result<(), Self::Error>;
}
#[derive(Debug, PartialEq, Eq)]
pub enum MidiServiceError<E> {
    Disposed, InvalidDevice, DuplicateDevice, UnknownDevice, InputUnavailable,
    OutputUnavailable, NotConnected, SysExDenied, QueueFull, IdExhausted,
    Codec(MidiError), Host(E),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiReceived {
    pub device_id: String,
    pub message: MidiMessage,
}
pub struct MidiDeviceService<H: MidiHost> {
    host: H,
    devices: BTreeMap<String, MidiDevice>,
    queue: VecDeque<MidiReceived>,
    capacity: usize,
    next_logical: u64,
    disposed: bool,
}
impl<H: MidiHost> MidiDeviceService<H> {
    pub fn new(host: H, capacity: usize) -> Result<Self, MidiServiceError<H::Error>> {
        if capacity == 0 { return Err(MidiServiceError::InvalidDevice); }
        Ok(Self { host, devices: BTreeMap::new(), queue: VecDeque::new(),
            capacity, next_logical: 0, disposed: false })
    }
    fn active(&self) -> Result<(), MidiServiceError<H::Error>> {
        if self.disposed { Err(MidiServiceError::Disposed) } else { Ok(()) }
    }
    fn require(&self, id: &str) -> Result<MidiDevice, MidiServiceError<H::Error>> {
        self.devices.get(id).cloned().ok_or(MidiServiceError::UnknownDevice)
    }
    pub fn capabilities(&self) -> MidiCapabilities { self.host.capabilities() }
    pub fn devices(&self) -> Vec<MidiDevice> { self.devices.values().cloned().collect() }
    pub fn refresh(&mut self) -> Result<Vec<MidiDevice>, MidiServiceError<H::Error>> {
        self.active()?;
        let found = self.host.discover().map_err(MidiServiceError::Host)?;
        let mut physical = BTreeMap::new();
        for device in found {
            if device.origin != Origin::Physical || device.id.trim().is_empty()
                || device.name.trim().is_empty() || physical.contains_key(&device.id)
                || self.devices.get(&device.id).is_some_and(|d| d.origin == Origin::Logical) {
                return Err(MidiServiceError::InvalidDevice);
            }
            physical.insert(device.id.clone(), device);
        }
        self.devices.retain(|_, d| d.origin == Origin::Logical);
        self.devices.extend(physical);
        Ok(self.devices())
    }
    pub fn create_logical(
        &mut self, name: &str, direction: Direction,
    ) -> Result<MidiDevice, MidiServiceError<H::Error>> {
        self.active()?;
        if name.trim().is_empty() { return Err(MidiServiceError::InvalidDevice); }
        let next = self.next_logical.checked_add(1).ok_or(MidiServiceError::IdExhausted)?;
        let id = format!("logical-midi-{next}");
        if self.devices.contains_key(&id) { return Err(MidiServiceError::DuplicateDevice); }
        self.next_logical = next;
        let device = MidiDevice { id: id.clone(), name: name.trim().to_string(),
            direction, origin: Origin::Logical,
            input_connected: direction != Direction::Output,
            output_connected: direction != Direction::Input };
        self.devices.insert(id, device.clone());
        Ok(device)
    }
    pub fn connect_input(&mut self, id: &str) -> Result<(), MidiServiceError<H::Error>> {
        self.active()?;
        let d = self.require(id)?;
        if d.direction == Direction::Output { return Err(MidiServiceError::InputUnavailable); }
        if d.origin == Origin::Physical {
            if !self.host.capabilities().physical_input { return Err(MidiServiceError::InputUnavailable); }
            self.host.connect_input(id).map_err(MidiServiceError::Host)?;
        }
        if let Some(d) = self.devices.get_mut(id) { d.input_connected = true; }
        Ok(())
    }
    pub fn connect_output(&mut self, id: &str) -> Result<(), MidiServiceError<H::Error>> {
        self.active()?;
        let d = self.require(id)?;
        if d.direction == Direction::Input { return Err(MidiServiceError::OutputUnavailable); }
        if d.origin == Origin::Physical {
            if !self.host.capabilities().physical_output { return Err(MidiServiceError::OutputUnavailable); }
            self.host.connect_output(id).map_err(MidiServiceError::Host)?;
        }
        if let Some(d) = self.devices.get_mut(id) { d.output_connected = true; }
        Ok(())
    }
    fn enqueue(&mut self, id: &str, message: MidiMessage) -> Result<(), MidiServiceError<H::Error>> {
        if self.queue.len() >= self.capacity { return Err(MidiServiceError::QueueFull); }
        self.queue.push_back(MidiReceived { device_id: id.to_string(), message });
        Ok(())
    }
    pub fn send(&mut self, id: &str, message: MidiMessage) -> Result<(), MidiServiceError<H::Error>> {
        self.active()?;
        let d = self.require(id)?;
        if d.direction == Direction::Input { return Err(MidiServiceError::OutputUnavailable); }
        if !d.output_connected { return Err(MidiServiceError::NotConnected); }
        let bytes = midi_protocol::encode(&message).map_err(MidiServiceError::Codec)?;
        if d.origin == Origin::Physical {
            if matches!(&message, MidiMessage::System { status: 0xf0, .. })
                && !self.host.capabilities().system_exclusive {
                return Err(MidiServiceError::SysExDenied);
            }
            self.host.send(id, &bytes).map_err(MidiServiceError::Host)
        } else {
            self.enqueue(id, message)
        }
    }
    /// The host explicitly calls receive for a connected input; no listener is spawned.
    pub fn receive(&mut self, id: &str, bytes: &[u8]) -> Result<(), MidiServiceError<H::Error>> {
        self.active()?;
        let d = self.require(id)?;
        if d.direction == Direction::Output { return Err(MidiServiceError::InputUnavailable); }
        if !d.input_connected { return Err(MidiServiceError::NotConnected); }
        let message = midi_protocol::decode(bytes).map_err(MidiServiceError::Codec)?;
        self.enqueue(id, message)
    }
    pub fn next_event(&mut self) -> Option<MidiReceived> { self.queue.pop_front() }
    pub fn disconnect(&mut self, id: &str) -> Result<(), MidiServiceError<H::Error>> {
        self.active()?;
        let d = self.require(id)?;
        if d.origin == Origin::Physical { self.host.disconnect(id).map_err(MidiServiceError::Host)?; }
        if let Some(d) = self.devices.get_mut(id) {
            d.input_connected = false; d.output_connected = false;
        }
        Ok(())
    }
    pub fn dispose(&mut self) -> Result<(), MidiServiceError<H::Error>> {
        if self.disposed { return Ok(()); }
        let ids = self.devices.values().filter(|d| d.origin == Origin::Physical
            && (d.input_connected || d.output_connected)).map(|d| d.id.clone()).collect::<Vec<_>>();
        for id in ids { self.disconnect(&id)?; }
        self.host.dispose().map_err(MidiServiceError::Host)?;
        self.devices.clear(); self.queue.clear(); self.disposed = true;
        Ok(())
    }
}
impl<H: MidiHost> Drop for MidiDeviceService<H> {
    fn drop(&mut self) { let _ = self.dispose(); }
}
