//! Portable OSC binary message/bundle codec and host-injected client adapter.
//! Pinned Kloks source: audio-midi/src/osc/{codec,service}.ts @ 286377a03a8c.
extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

const MAX_PACKET_BYTES: usize = 1024 * 1024;
const MAX_BUNDLE_DEPTH: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub enum OscArgument {
    Int(i32),
    Float(f32),
    Double(f64),
    Long(i64),
    TimeTag(u64),
    String(String),
    Blob(Vec<u8>),
    Bool(bool),
    Nil,
    Impulse,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OscMessage {
    pub address: String,
    pub args: Vec<OscArgument>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OscBundle {
    pub timetag: u64,
    pub elements: Vec<OscPacket>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OscPacket {
    Message(OscMessage),
    Bundle(OscBundle),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OscError {
    InvalidAddress,
    InvalidString,
    InvalidPadding,
    InvalidTypeTag,
    Truncated,
    TrailingBytes,
    InvalidBundle,
    InvalidBlob,
    TooLarge,
    TooDeep,
}

fn write_string(out: &mut Vec<u8>, text: &str) -> Result<(), OscError> {
    if text.as_bytes().contains(&0) {
        return Err(OscError::InvalidString);
    }
    out.extend_from_slice(text.as_bytes());
    out.push(0);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
    Ok(())
}

fn take<'a>(bytes: &'a [u8], pos: &mut usize, amount: usize) -> Result<&'a [u8], OscError> {
    let end = pos.checked_add(amount).ok_or(OscError::TooLarge)?;
    if end > bytes.len() {
        return Err(OscError::Truncated);
    }
    let part = &bytes[*pos..end];
    *pos = end;
    Ok(part)
}

fn read_string(bytes: &[u8], pos: &mut usize) -> Result<String, OscError> {
    let start = *pos;
    let width = bytes
        .get(start..)
        .ok_or(OscError::Truncated)?
        .iter()
        .position(|&b| b == 0)
        .ok_or(OscError::Truncated)?;
    let content =
        core::str::from_utf8(&bytes[start..start + width]).map_err(|_| OscError::InvalidString)?;
    let end = start.checked_add(width + 1).ok_or(OscError::TooLarge)?;
    let padded = end.checked_add(3).ok_or(OscError::TooLarge)? & !3;
    if padded > bytes.len() {
        return Err(OscError::Truncated);
    }
    if bytes[end..padded].iter().any(|&b| b != 0) {
        return Err(OscError::InvalidPadding);
    }
    *pos = padded;
    Ok(content.to_string())
}

fn read_i32(bytes: &[u8], pos: &mut usize) -> Result<i32, OscError> {
    let data: [u8; 4] = take(bytes, pos, 4)?
        .try_into()
        .map_err(|_| OscError::Truncated)?;
    Ok(i32::from_be_bytes(data))
}

fn read_i64(bytes: &[u8], pos: &mut usize) -> Result<i64, OscError> {
    let data: [u8; 8] = take(bytes, pos, 8)?
        .try_into()
        .map_err(|_| OscError::Truncated)?;
    Ok(i64::from_be_bytes(data))
}

fn read_u64(bytes: &[u8], pos: &mut usize) -> Result<u64, OscError> {
    let data: [u8; 8] = take(bytes, pos, 8)?
        .try_into()
        .map_err(|_| OscError::Truncated)?;
    Ok(u64::from_be_bytes(data))
}

fn arg_tag(argument: &OscArgument) -> char {
    match argument {
        OscArgument::Int(_) => 'i',
        OscArgument::Float(_) => 'f',
        OscArgument::Double(_) => 'd',
        OscArgument::Long(_) => 'h',
        OscArgument::TimeTag(_) => 't',
        OscArgument::String(_) => 's',
        OscArgument::Blob(_) => 'b',
        OscArgument::Bool(true) => 'T',
        OscArgument::Bool(false) => 'F',
        OscArgument::Nil => 'N',
        OscArgument::Impulse => 'I',
    }
}

fn encode_internal(packet: &OscPacket, depth: usize) -> Result<Vec<u8>, OscError> {
    if depth > MAX_BUNDLE_DEPTH {
        return Err(OscError::TooDeep);
    }
    let mut out = Vec::new();
    match packet {
        OscPacket::Message(message) => {
            if !message.address.starts_with('/') {
                return Err(OscError::InvalidAddress);
            }
            write_string(&mut out, &message.address)?;
            let mut types = String::from(",");
            for arg in &message.args {
                types.push(arg_tag(arg));
            }
            write_string(&mut out, &types)?;
            for arg in &message.args {
                match arg {
                    OscArgument::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
                    OscArgument::Float(v) => out.extend_from_slice(&v.to_bits().to_be_bytes()),
                    OscArgument::Double(v) => out.extend_from_slice(&v.to_bits().to_be_bytes()),
                    OscArgument::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
                    OscArgument::TimeTag(v) => out.extend_from_slice(&v.to_be_bytes()),
                    OscArgument::String(v) => write_string(&mut out, v)?,
                    OscArgument::Blob(v) => {
                        if v.len() > MAX_PACKET_BYTES {
                            return Err(OscError::TooLarge);
                        }
                        out.extend_from_slice(&(v.len() as i32).to_be_bytes());
                        out.extend_from_slice(v);
                        while !out.len().is_multiple_of(4) {
                            out.push(0);
                        }
                    }
                    OscArgument::Bool(_) | OscArgument::Nil | OscArgument::Impulse => {}
                }
            }
        }
        OscPacket::Bundle(bundle) => {
            out.extend_from_slice(b"#bundle\0");
            out.extend_from_slice(&bundle.timetag.to_be_bytes());
            for element in &bundle.elements {
                let data = encode_internal(element, depth + 1)?;
                if data.len() > MAX_PACKET_BYTES {
                    return Err(OscError::TooLarge);
                }
                out.extend_from_slice(&(data.len() as i32).to_be_bytes());
                out.extend_from_slice(&data);
            }
        }
    }
    if out.len() > MAX_PACKET_BYTES {
        return Err(OscError::TooLarge);
    }
    Ok(out)
}

pub fn encode(packet: &OscPacket) -> Result<Vec<u8>, OscError> {
    encode_internal(packet, 0)
}

fn decode_internal(bytes: &[u8], depth: usize) -> Result<OscPacket, OscError> {
    if depth > MAX_BUNDLE_DEPTH {
        return Err(OscError::TooDeep);
    }
    if bytes.len() > MAX_PACKET_BYTES {
        return Err(OscError::TooLarge);
    }
    if !bytes.len().is_multiple_of(4) {
        return Err(OscError::InvalidPadding);
    }
    let mut pos = 0;
    if bytes.starts_with(b"#bundle\0") {
        take(bytes, &mut pos, 8)?;
        let timetag = read_u64(bytes, &mut pos)?;
        let mut elements = Vec::new();
        while pos < bytes.len() {
            let length = read_i32(bytes, &mut pos)?;
            if length <= 0 || length % 4 != 0 {
                return Err(OscError::InvalidBundle);
            }
            let data = take(bytes, &mut pos, length as usize)?;
            elements.push(decode_internal(data, depth + 1)?);
        }
        return Ok(OscPacket::Bundle(OscBundle { timetag, elements }));
    }
    let address = read_string(bytes, &mut pos)?;
    if !address.starts_with('/') {
        return Err(OscError::InvalidAddress);
    }
    let tags = read_string(bytes, &mut pos)?;
    if !tags.starts_with(',') {
        return Err(OscError::InvalidTypeTag);
    }
    let mut args = Vec::new();
    for tag in tags.bytes().skip(1) {
        let arg = match tag {
            b'i' => OscArgument::Int(read_i32(bytes, &mut pos)?),
            b'f' => OscArgument::Float(f32::from_bits(read_i32(bytes, &mut pos)? as u32)),
            b'd' => OscArgument::Double(f64::from_bits(read_u64(bytes, &mut pos)?)),
            b'h' => OscArgument::Long(read_i64(bytes, &mut pos)?),
            b't' => OscArgument::TimeTag(read_u64(bytes, &mut pos)?),
            b's' => OscArgument::String(read_string(bytes, &mut pos)?),
            b'b' => {
                let n = read_i32(bytes, &mut pos)?;
                if n < 0 {
                    return Err(OscError::InvalidBlob);
                }
                let data = take(bytes, &mut pos, n as usize)?.to_vec();
                let padded = pos.checked_add(3).ok_or(OscError::TooLarge)? & !3;
                if padded > bytes.len() {
                    return Err(OscError::Truncated);
                }
                if bytes[pos..padded].iter().any(|&v| v != 0) {
                    return Err(OscError::InvalidPadding);
                }
                pos = padded;
                OscArgument::Blob(data)
            }
            b'T' => OscArgument::Bool(true),
            b'F' => OscArgument::Bool(false),
            b'N' => OscArgument::Nil,
            b'I' => OscArgument::Impulse,
            _ => return Err(OscError::InvalidTypeTag),
        };
        args.push(arg);
    }
    if pos != bytes.len() {
        return Err(OscError::TrailingBytes);
    }
    Ok(OscPacket::Message(OscMessage { address, args }))
}

pub fn decode(bytes: &[u8]) -> Result<OscPacket, OscError> {
    decode_internal(bytes, 0)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OscEndpoint {
    pub id: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug)]
pub enum OscSendError<E> {
    InvalidEndpoint,
    DuplicateEndpoint,
    UnknownEndpoint,
    Codec(OscError),
    Host(E),
}

/// Host must supply transport: no socket, listener thread or scheduler is created.
pub trait OscTransport {
    type Error;
    fn send(&mut self, endpoint: &OscEndpoint, packet: &[u8]) -> Result<(), Self::Error>;
}

pub struct OscClient<T: OscTransport> {
    transport: T,
    endpoints: BTreeMap<String, OscEndpoint>,
}

impl<T: OscTransport> OscClient<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            endpoints: BTreeMap::new(),
        }
    }

    pub fn add_endpoint(&mut self, endpoint: OscEndpoint) -> Result<(), OscSendError<T::Error>> {
        if endpoint.id.trim().is_empty() || endpoint.host.trim().is_empty() || endpoint.port == 0 {
            return Err(OscSendError::InvalidEndpoint);
        }
        if self.endpoints.contains_key(&endpoint.id) {
            return Err(OscSendError::DuplicateEndpoint);
        }
        self.endpoints.insert(endpoint.id.clone(), endpoint);
        Ok(())
    }

    pub fn remove_endpoint(&mut self, id: &str) -> Option<OscEndpoint> {
        self.endpoints.remove(id)
    }

    pub fn send_packet(
        &mut self,
        id: &str,
        packet: &OscPacket,
    ) -> Result<(), OscSendError<T::Error>> {
        let endpoint = self
            .endpoints
            .get(id)
            .ok_or(OscSendError::UnknownEndpoint)?;
        let bytes = encode(packet).map_err(OscSendError::Codec)?;
        self.transport
            .send(endpoint, &bytes)
            .map_err(OscSendError::Host)
    }

    pub fn accept(&self, bytes: &[u8]) -> Result<OscPacket, OscError> {
        decode(bytes)
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }
}
