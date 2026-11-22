use std::collections::BTreeMap;
use kloksrs::host::{
    poll_with_host, FileSystemPort, HostPortError, MonotonicTime, NetworkClient,
    NetworkPort, ResourceClient,
};
use kloksrs::{ClockOptions, ClockService};

#[derive(Default)]
struct MemoryFs {
    files: BTreeMap<String, Vec<u8>>,
}
impl FileSystemPort for MemoryFs {
    type Error = &'static str;
    fn read(&mut self, name: &str, _: usize) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.files.get(name).cloned())
    }
    fn write(&mut self, name: &str, bytes: &[u8]) -> Result<(), Self::Error> {
        self.files.insert(name.to_string(), bytes.to_vec());
        Ok(())
    }
    fn delete(&mut self, name: &str) -> Result<bool, Self::Error> {
        Ok(self.files.remove(name).is_some())
    }
}

#[test]
fn resource_paths_are_bounded_and_host_owned() {
    let mut store = ResourceClient::new(MemoryFs::default(), 4).unwrap();
    store.write("clips/a.bin", &[1, 2, 3]).unwrap();
    assert_eq!(store.read("clips/a.bin").unwrap(), Some(vec![1, 2, 3]));
    assert_eq!(store.write("clips/b.bin", &[1; 5]), Err(HostPortError::TooLarge));
    assert_eq!(store.read("../escape"), Err(HostPortError::InvalidName));
    assert_eq!(store.delete("/absolute"), Err(HostPortError::InvalidName));
    assert_eq!(store.delete("clips/a.bin"), Ok(true));
    assert_eq!(store.read("clips/a.bin"), Ok(None));
    assert!(ResourceClient::new(MemoryFs::default(), 0).is_err());
}

#[derive(Default)]
struct MemoryNetwork { endpoint: String }
impl NetworkPort for MemoryNetwork {
    type Error = &'static str;
    fn exchange(&mut self, endpoint: &str, request: &[u8], _: usize)
        -> Result<Vec<u8>, Self::Error> {
        self.endpoint = endpoint.to_string();
        Ok(request.iter().rev().copied().collect())
    }
}
#[test]
fn network_port_is_explicit_and_bounded() {
    let mut client = NetworkClient::new(MemoryNetwork::default(), 3).unwrap();
    assert_eq!(client.exchange("loopback", &[1, 2, 3]), Ok(vec![3, 2, 1]));
    assert_eq!(client.exchange("", &[1]), Err(HostPortError::InvalidName));
    assert_eq!(client.exchange("loopback", &[0; 4]), Err(HostPortError::TooLarge));
}

struct FakeTime(f64);
impl MonotonicTime for FakeTime {
    fn now_millis(&self) -> f64 { self.0 }
}
#[test]
fn clock_consumes_host_time_without_owning_a_scheduler() {
    let mut clock = ClockService::new(ClockOptions::default()).unwrap();
    clock.start(100.0).unwrap();
    assert_eq!(poll_with_host(&mut clock, &FakeTime(600.0)).unwrap().len(), 24);
    assert_eq!(poll_with_host(&mut clock, &FakeTime(-1.0)).unwrap_err(),
        kloksrs::ClockError::InvalidTime);
}
