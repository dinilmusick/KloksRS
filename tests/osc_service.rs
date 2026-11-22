use kloksrs::osc::{
    encode, OscArgument, OscBundle, OscEndpoint, OscMessage, OscPacket, OscTransport,
};
use kloksrs::osc_service::{matches_osc_address, OscHost, OscService, OscServiceError};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
struct State {
    listening: Vec<String>,
    stopped: Vec<String>,
    sent: Vec<Vec<u8>>,
    disposed: bool,
}
#[derive(Clone)]
struct FakeHost(Rc<RefCell<State>>);
impl OscTransport for FakeHost {
    type Error = &'static str;
    fn send(&mut self, _: &OscEndpoint, packet: &[u8]) -> Result<(), Self::Error> {
        self.0.borrow_mut().sent.push(packet.to_vec());
        Ok(())
    }
}
impl OscHost for FakeHost {
    fn listen(&mut self, endpoint: &OscEndpoint) -> Result<(), Self::Error> {
        self.0.borrow_mut().listening.push(endpoint.id.clone());
        Ok(())
    }
    fn stop(&mut self, endpoint: &OscEndpoint) -> Result<(), Self::Error> {
        self.0.borrow_mut().stopped.push(endpoint.id.clone());
        Ok(())
    }
    fn dispose(&mut self) -> Result<(), Self::Error> {
        self.0.borrow_mut().disposed = true;
        Ok(())
    }
}
fn endpoint() -> OscEndpoint {
    OscEndpoint {
        id: "osc".into(),
        host: "127.0.0.1".into(),
        port: 9000,
    }
}
fn packet(address: &str) -> OscPacket {
    OscPacket::Message(OscMessage {
        address: address.into(),
        args: vec![OscArgument::Int(42)],
    })
}
#[test]
fn host_listener_lifecycle_and_bundle_dispatch_are_deterministic() {
    let state = Rc::new(RefCell::new(State::default()));
    let mut service = OscService::new(FakeHost(state.clone()), 4).unwrap();
    service.add_client(endpoint()).unwrap();
    assert_eq!(service.start_server(endpoint()), Ok(true));
    assert_eq!(service.start_server(endpoint()), Ok(false));
    let sub = service.subscribe("/synth/*").unwrap();
    let bundle = OscPacket::Bundle(OscBundle {
        timetag: 1,
        elements: vec![packet("/synth/note"), packet("/other")],
    });
    service.send("osc", &bundle).unwrap();
    assert_eq!(service.receive("osc", &encode(&bundle).unwrap()), Ok(2));
    let first = service.next_event().unwrap();
    assert_eq!(first.message.address, "/synth/note");
    assert_eq!(first.subscription_ids, vec![sub]);
    assert!(service.next_event().unwrap().subscription_ids.is_empty());
    assert_eq!(state.borrow().sent.len(), 1);
    assert!(service.unsubscribe(sub));
    assert_eq!(service.stop_server("osc"), Ok(true));
    assert_eq!(service.stop_server("osc"), Ok(false));
    service.dispose().unwrap();
    assert_eq!(state.borrow().listening, vec!["osc"]);
    assert_eq!(state.borrow().stopped, vec!["osc"]);
    assert!(state.borrow().disposed);
    assert!(matches!(
        service.send("osc", &packet("/a")),
        Err(OscServiceError::Disposed)
    ));
}
#[test]
fn invalid_packets_and_backpressure_have_no_partial_delivery() {
    let mut server = OscService::new(FakeHost(Rc::new(RefCell::new(State::default()))), 1).unwrap();
    assert_eq!(
        server.subscribe("invalid"),
        Err(OscServiceError::InvalidPattern)
    );
    assert!(matches!(
        server.start_server(OscEndpoint {
            id: "bad".into(),
            host: "".into(),
            port: 0,
        }),
        Err(OscServiceError::InvalidEndpoint)
    ));
    server.start_server(endpoint()).unwrap();
    let bundle = OscPacket::Bundle(OscBundle {
        timetag: 0,
        elements: vec![packet("/a"), packet("/b")],
    });
    assert_eq!(
        server.receive("osc", &encode(&bundle).unwrap()),
        Err(OscServiceError::QueueFull)
    );
    assert_eq!(server.next_event(), None);
    assert!(matches!(
        server.receive("osc", b"bad"),
        Err(OscServiceError::Codec(_))
    ));
    assert_eq!(
        server.receive("missing", &encode(&packet("/a")).unwrap()),
        Err(OscServiceError::UnknownEndpoint)
    );
}
#[test]
fn patterns_match_unicode_wildcards_without_resident_subscribers() {
    assert!(matches_osc_address("/synth/a/b", "/synth/*"));
    assert!(matches_osc_address("/synth/é", "/synth/?"));
    assert!(!matches_osc_address("/synth/ab", "/synth/?"));
    assert!(!matches_osc_address("synth/a", "/synth/*"));
}
