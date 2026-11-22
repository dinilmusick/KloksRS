use kloksrs::osc::{
    decode, encode, OscArgument, OscBundle, OscClient, OscEndpoint, OscError, OscMessage,
    OscPacket, OscSendError, OscTransport,
};

#[test]
fn osc_message_round_trips_all_supported_tags() {
    let packet = OscPacket::Message(OscMessage {
        address: "/instrument/note".into(),
        args: vec![
            OscArgument::Int(-4),
            OscArgument::Float(2.5),
            OscArgument::Double(-0.75),
            OscArgument::Long(-1234567890123),
            OscArgument::TimeTag(0x1122334455667788),
            OscArgument::String("hello".into()),
            OscArgument::Blob(vec![0, 1, 255]),
            OscArgument::Bool(true),
            OscArgument::Bool(false),
            OscArgument::Nil,
            OscArgument::Impulse,
        ],
    });
    let data = encode(&packet).unwrap();
    assert_eq!(data.len() % 4, 0);
    assert_eq!(decode(&data).unwrap(), packet);
}

#[test]
fn nested_bundles_are_preserved_in_wire_order() {
    let inner = OscPacket::Message(OscMessage {
        address: "/a".into(),
        args: vec![],
    });
    let packet = OscPacket::Bundle(OscBundle {
        timetag: 1234,
        elements: vec![
            inner.clone(),
            OscPacket::Bundle(OscBundle {
                timetag: 5678,
                elements: vec![inner],
            }),
        ],
    });
    assert_eq!(decode(&encode(&packet).unwrap()).unwrap(), packet);
}

#[test]
fn malformed_and_truncated_packets_fail() {
    assert_eq!(
        encode(&OscPacket::Message(OscMessage {
            address: "bad".into(),
            args: vec![],
        })),
        Err(OscError::InvalidAddress)
    );
    assert!(decode(b"/abc").is_err());
    assert_eq!(decode(b"#bundle\0"), Err(OscError::Truncated));
    assert_eq!(
        decode(&[b'/', b'a', 0, 1, b',', b'i', 0, 0, 0, 0, 0, 1]),
        Err(OscError::InvalidPadding)
    );
    assert_eq!(
        decode(&[b'/', b'a', 0, 0, b',', b'z', 0, 0]),
        Err(OscError::InvalidTypeTag)
    );
    let mut data = encode(&OscPacket::Message(OscMessage {
        address: "/a".into(),
        args: vec![OscArgument::Blob(vec![1, 2, 3])],
    }))
    .unwrap();
    data.truncate(data.len() - 1);
    assert!(decode(&data).is_err());
}

#[derive(Default)]
struct MemoryTransport(Vec<u8>);
impl OscTransport for MemoryTransport {
    type Error = ();
    fn send(&mut self, _endpoint: &OscEndpoint, packet: &[u8]) -> Result<(), Self::Error> {
        self.0.extend_from_slice(packet);
        Ok(())
    }
}

#[test]
fn host_injection_has_explicit_endpoint_configuration() {
    let mut client = OscClient::new(MemoryTransport::default());
    assert!(matches!(
        client.add_endpoint(OscEndpoint {
            id: "bad".into(),
            host: "".into(),
            port: 8000,
        }),
        Err(OscSendError::InvalidEndpoint)
    ));
    client
        .add_endpoint(OscEndpoint {
            id: "local".into(),
            host: "127.0.0.1".into(),
            port: 9000,
        })
        .unwrap();
    let packet = OscPacket::Message(OscMessage {
        address: "/test".into(),
        args: vec![],
    });
    client.send_packet("local", &packet).unwrap();
    assert_eq!(client.accept(&client.transport().0).unwrap(), packet);
    assert!(matches!(
        client.send_packet("missing", &packet),
        Err(OscSendError::UnknownEndpoint)
    ));
    assert!(client.remove_endpoint("local").is_some());
}
