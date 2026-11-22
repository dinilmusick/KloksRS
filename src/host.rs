//! Host-injected time, filesystem and network capabilities; allocation-only.
//! No filesystem, socket, process or resident scheduler is created by KloksRS.
use crate::{ClockError, ClockService, ClockTick};
use alloc::vec::Vec;

pub trait MonotonicTime {
    fn now_millis(&self) -> f64;
}

pub fn poll_with_host<T: MonotonicTime>(
    service: &mut ClockService,
    time: &T,
) -> Result<Vec<ClockTick>, ClockError> {
    service.poll(time.now_millis())
}

#[derive(Debug, PartialEq, Eq)]
pub enum HostPortError<E> {
    InvalidName,
    InvalidLimit,
    TooLarge,
    Backend(E),
}

pub trait FileSystemPort {
    type Error;
    /// Host enforces max_bytes before allocating a response.
    fn read(&mut self, name: &str, max_bytes: usize) -> Result<Option<Vec<u8>>, Self::Error>;
    fn write(&mut self, name: &str, bytes: &[u8]) -> Result<(), Self::Error>;
    fn delete(&mut self, name: &str) -> Result<bool, Self::Error>;
}

fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name.contains('\0')
        && name
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

pub struct ResourceClient<F: FileSystemPort> {
    backend: F,
    max_bytes: usize,
}

impl<F: FileSystemPort> ResourceClient<F> {
    pub fn new(backend: F, max_bytes: usize) -> Result<Self, HostPortError<F::Error>> {
        if max_bytes == 0 {
            return Err(HostPortError::InvalidLimit);
        }
        Ok(Self { backend, max_bytes })
    }

    pub fn read(&mut self, name: &str) -> Result<Option<Vec<u8>>, HostPortError<F::Error>> {
        if !valid_name(name) {
            return Err(HostPortError::InvalidName);
        }
        let data = self
            .backend
            .read(name, self.max_bytes)
            .map_err(HostPortError::Backend)?;
        if data
            .as_ref()
            .is_some_and(|value| value.len() > self.max_bytes)
        {
            return Err(HostPortError::TooLarge);
        }
        Ok(data)
    }

    pub fn write(&mut self, name: &str, bytes: &[u8]) -> Result<(), HostPortError<F::Error>> {
        if !valid_name(name) {
            return Err(HostPortError::InvalidName);
        }
        if bytes.len() > self.max_bytes {
            return Err(HostPortError::TooLarge);
        }
        self.backend
            .write(name, bytes)
            .map_err(HostPortError::Backend)
    }

    pub fn delete(&mut self, name: &str) -> Result<bool, HostPortError<F::Error>> {
        if !valid_name(name) {
            return Err(HostPortError::InvalidName);
        }
        self.backend.delete(name).map_err(HostPortError::Backend)
    }

    pub fn backend_mut(&mut self) -> &mut F {
        &mut self.backend
    }
}

/// A host-owned request/response interface. The host owns retries and connections.
pub trait NetworkPort {
    type Error;
    fn exchange(
        &mut self,
        endpoint: &str,
        request: &[u8],
        max_response_bytes: usize,
    ) -> Result<Vec<u8>, Self::Error>;
}

pub struct NetworkClient<N: NetworkPort> {
    backend: N,
    max_bytes: usize,
}

impl<N: NetworkPort> NetworkClient<N> {
    pub fn new(backend: N, max_bytes: usize) -> Result<Self, HostPortError<N::Error>> {
        if max_bytes == 0 {
            return Err(HostPortError::InvalidLimit);
        }
        Ok(Self { backend, max_bytes })
    }

    pub fn exchange(
        &mut self,
        endpoint: &str,
        bytes: &[u8],
    ) -> Result<Vec<u8>, HostPortError<N::Error>> {
        if endpoint.trim().is_empty() {
            return Err(HostPortError::InvalidName);
        }
        if bytes.len() > self.max_bytes {
            return Err(HostPortError::TooLarge);
        }
        let response = self
            .backend
            .exchange(endpoint, bytes, self.max_bytes)
            .map_err(HostPortError::Backend)?;
        if response.len() > self.max_bytes {
            return Err(HostPortError::TooLarge);
        }
        Ok(response)
    }
}
