//! Host-injected OSC client/server lifecycle, inbound bundles and subscriptions.
//! Native counterpart of Kloks audio-midi/src/osc/service.ts.
use crate::osc::{self, OscEndpoint, OscError, OscMessage, OscPacket, OscTransport};
use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub trait OscHost: OscTransport {
    fn listen(&mut self, endpoint: &OscEndpoint) -> Result<(), Self::Error>;
    fn stop(&mut self, endpoint: &OscEndpoint) -> Result<(), Self::Error>;
    fn dispose(&mut self) -> Result<(), Self::Error>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum OscServiceError<E> {
    InvalidEndpoint,
    InvalidPattern,
    DuplicateEndpoint,
    UnknownEndpoint,
    Disposed,
    QueueFull,
    IdExhausted,
    Codec(OscError),
    Host(E),
}
#[derive(Clone, Debug, PartialEq)]
pub struct OscDispatch {
    pub endpoint_id: String,
    pub message: OscMessage,
    pub subscription_ids: Vec<u64>,
}
pub struct OscService<H: OscHost> {
    host: H,
    clients: BTreeMap<String, OscEndpoint>,
    servers: BTreeMap<String, OscEndpoint>,
    patterns: BTreeMap<u64, String>,
    next_subscription: u64,
    events: VecDeque<OscDispatch>,
    max_events: usize,
    disposed: bool,
}
fn valid_endpoint(ep: &OscEndpoint) -> bool {
    !ep.id.trim().is_empty() && !ep.host.trim().is_empty() && ep.port > 0
}

/// Supports OSC wildcard '*' and '?'. Host adapters can supply richer matchers.
/// '*' is multi-character, '?' exactly one Unicode scalar.
pub fn matches_osc_address(address: &str, pattern: &str) -> bool {
    if !address.starts_with('/') || !pattern.starts_with('/') {
        return false;
    }
    let chars = address.chars().collect::<Vec<_>>();
    let mut matched = alloc::vec![false; chars.len() + 1];
    matched[0] = true;
    for token in pattern.chars() {
        if token == '*' {
            for i in 1..matched.len() {
                matched[i] = matched[i] || matched[i - 1];
            }
        } else {
            for i in (1..matched.len()).rev() {
                matched[i] = matched[i - 1] && (token == '?' || token == chars[i - 1]);
            }
            matched[0] = false;
        }
    }
    matched[chars.len()]
}
fn flatten(packet: &OscPacket, output: &mut Vec<OscMessage>) {
    match packet {
        OscPacket::Message(msg) => output.push(msg.clone()),
        OscPacket::Bundle(bundle) => {
            for element in &bundle.elements {
                flatten(element, output);
            }
        }
    }
}
impl<H: OscHost> OscService<H> {
    pub fn new(host: H, max_events: usize) -> Result<Self, OscServiceError<H::Error>> {
        if max_events == 0 {
            return Err(OscServiceError::QueueFull);
        }
        Ok(Self {
            host,
            clients: BTreeMap::new(),
            servers: BTreeMap::new(),
            patterns: BTreeMap::new(),
            next_subscription: 0,
            events: VecDeque::new(),
            max_events,
            disposed: false,
        })
    }
    fn active(&self) -> Result<(), OscServiceError<H::Error>> {
        if self.disposed {
            Err(OscServiceError::Disposed)
        } else {
            Ok(())
        }
    }
    pub fn add_client(&mut self, ep: OscEndpoint) -> Result<(), OscServiceError<H::Error>> {
        self.active()?;
        if !valid_endpoint(&ep) {
            return Err(OscServiceError::InvalidEndpoint);
        }
        if self.clients.contains_key(&ep.id) {
            return Err(OscServiceError::DuplicateEndpoint);
        }
        self.clients.insert(ep.id.clone(), ep);
        Ok(())
    }
    pub fn start_server(&mut self, ep: OscEndpoint) -> Result<bool, OscServiceError<H::Error>> {
        self.active()?;
        if !valid_endpoint(&ep) {
            return Err(OscServiceError::InvalidEndpoint);
        }
        if self.servers.contains_key(&ep.id) {
            return Ok(false);
        }
        self.host.listen(&ep).map_err(OscServiceError::Host)?;
        self.servers.insert(ep.id.clone(), ep);
        Ok(true)
    }
    pub fn stop_server(&mut self, id: &str) -> Result<bool, OscServiceError<H::Error>> {
        self.active()?;
        let Some(ep) = self.servers.get(id).cloned() else {
            return Ok(false);
        };
        self.host.stop(&ep).map_err(OscServiceError::Host)?;
        self.servers.remove(id);
        Ok(true)
    }
    pub fn send(&mut self, id: &str, packet: &OscPacket) -> Result<(), OscServiceError<H::Error>> {
        self.active()?;
        let ep = self
            .clients
            .get(id)
            .ok_or(OscServiceError::UnknownEndpoint)?
            .clone();
        let wire = osc::encode(packet).map_err(OscServiceError::Codec)?;
        self.host.send(&ep, &wire).map_err(OscServiceError::Host)
    }
    pub fn subscribe(&mut self, pattern: &str) -> Result<u64, OscServiceError<H::Error>> {
        self.active()?;
        if !pattern.starts_with('/') {
            return Err(OscServiceError::InvalidPattern);
        }
        let next = self
            .next_subscription
            .checked_add(1)
            .ok_or(OscServiceError::IdExhausted)?;
        self.next_subscription = next;
        self.patterns.insert(next, pattern.to_string());
        Ok(next)
    }
    pub fn unsubscribe(&mut self, id: u64) -> bool {
        self.patterns.remove(&id).is_some()
    }
    /// The host passes incoming datagrams to this method; dispatch is host-drained.
    pub fn receive(&mut self, id: &str, bytes: &[u8]) -> Result<usize, OscServiceError<H::Error>> {
        self.active()?;
        if !self.servers.contains_key(id) {
            return Err(OscServiceError::UnknownEndpoint);
        }
        let packet = osc::decode(bytes).map_err(OscServiceError::Codec)?;
        let mut messages = Vec::new();
        flatten(&packet, &mut messages);
        if self
            .events
            .len()
            .checked_add(messages.len())
            .is_none_or(|n| n > self.max_events)
        {
            return Err(OscServiceError::QueueFull);
        }
        let count = messages.len();
        for message in messages {
            let matched = self
                .patterns
                .iter()
                .filter(|(_, pattern)| matches_osc_address(&message.address, pattern))
                .map(|(key, _)| *key)
                .collect();
            self.events.push_back(OscDispatch {
                endpoint_id: id.to_string(),
                message,
                subscription_ids: matched,
            });
        }
        Ok(count)
    }
    pub fn next_event(&mut self) -> Option<OscDispatch> {
        self.events.pop_front()
    }
    pub fn active_servers(&self) -> Vec<OscEndpoint> {
        self.servers.values().cloned().collect()
    }
    pub fn dispose(&mut self) -> Result<(), OscServiceError<H::Error>> {
        if self.disposed {
            return Ok(());
        }
        let ids = self.servers.keys().cloned().collect::<Vec<_>>();
        for id in ids {
            self.stop_server(&id)?;
        }
        self.host.dispose().map_err(OscServiceError::Host)?;
        self.clients.clear();
        self.patterns.clear();
        self.events.clear();
        self.disposed = true;
        Ok(())
    }
}
impl<H: OscHost> Drop for OscService<H> {
    fn drop(&mut self) {
        let _ = self.dispose();
    }
}
