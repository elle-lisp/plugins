//! Elle MQTT plugin — MQTT packet codec via the `mqttbytes` crate.
//!
//! State-machine pattern: this plugin handles MQTT packet encode/decode only.
//! All TCP I/O happens in Elle code via `port/read`/`port/write`.

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_OK, SIG_ERROR};
use mqttbytes::v4::{self, Packet};
use mqttbytes::QoS;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// State struct
// ---------------------------------------------------------------------------

/// MQTT protocol state machine. Type name: `"mqtt-state"`.
///
/// Holds the protocol version, packet ID counter, incoming byte buffer,
/// and parsed packet queue. No I/O happens here.
pub struct MqttState {
    /// Protocol version: 4 = MQTT 3.1.1, 5 = MQTT 5.0 (only 4 supported currently)
    #[allow(dead_code)]
    protocol: Cell<u8>,
    /// Keep-alive interval in seconds
    keep_alive: Cell<u16>,
    /// Monotonically increasing packet ID counter
    next_packet_id: Cell<u16>,
    /// Incoming raw TCP bytes not yet parsed
    incoming: RefCell<bytes::BytesMut>,
    /// Parsed packets waiting to be consumed
    packets: RefCell<VecDeque<Packet>>,
    /// True after a successful CONNACK is received
    connected: Cell<bool>,
}

// ---------------------------------------------------------------------------
// Plugin entry point
// ---------------------------------------------------------------------------
elle_plugin::define_plugin!("mqtt/", &PRIMITIVES);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_state<'a>(
    args: *const ElleValue,
    nargs: usize,
    idx: usize,
    name: &str,
) -> Result<&'a MqttState, ElleResult> {
    let a = api();
    let val = unsafe { a.arg(args, nargs, idx) };
    a.get_external::<MqttState>(val, "mqtt-state").ok_or_else(|| {
        a.err(
            "type-error",
            &format!(
                "{}: expected mqtt-state, got {}",
                name,
                a.type_name(val),
            ),
        )
    })
}

fn mqtt_err(name: &str, msg: impl std::fmt::Display) -> ElleResult {
    let a = api();
    a.err("mqtt-error", &format!("{}: {}", name, msg))
}

fn qos_from_int(n: i64) -> Option<QoS> {
    match n {
        0 => Some(QoS::AtMostOnce),
        1 => Some(QoS::AtLeastOnce),
        2 => Some(QoS::ExactlyOnce),
        _ => None,
    }
}

fn qos_to_int(q: QoS) -> i64 {
    match q {
        QoS::AtMostOnce => 0,
        QoS::AtLeastOnce => 1,
        QoS::ExactlyOnce => 2,
    }
}

/// Encode a packet into bytes using a temporary buffer.
fn encode_packet(packet: &Packet) -> Result<Vec<u8>, String> {
    let mut buf = Vec::with_capacity(256);
    match packet {
        Packet::Connect(p) => {
            let mut b = bytes::BytesMut::new();
            p.write(&mut b).map_err(|e| e.to_string())?;
            buf.extend_from_slice(&b);
        }
        Packet::Publish(p) => {
            let mut b = bytes::BytesMut::new();
            p.write(&mut b).map_err(|e| e.to_string())?;
            buf.extend_from_slice(&b);
        }
        Packet::Subscribe(p) => {
            let mut b = bytes::BytesMut::new();
            p.write(&mut b).map_err(|e| e.to_string())?;
            buf.extend_from_slice(&b);
        }
        Packet::Unsubscribe(p) => {
            let mut b = bytes::BytesMut::new();
            p.write(&mut b).map_err(|e| e.to_string())?;
            buf.extend_from_slice(&b);
        }
        Packet::PingReq => {
            let p = v4::PingReq;
            let mut b = bytes::BytesMut::new();
            p.write(&mut b).map_err(|e| e.to_string())?;
            buf.extend_from_slice(&b);
        }
        Packet::Disconnect => {
            let p = v4::Disconnect;
            let mut b = bytes::BytesMut::new();
            p.write(&mut b).map_err(|e| e.to_string())?;
            buf.extend_from_slice(&b);
        }
        Packet::PubAck(p) => {
            let mut b = bytes::BytesMut::new();
            p.write(&mut b).map_err(|e| e.to_string())?;
            buf.extend_from_slice(&b);
        }
        _ => return Err("unsupported packet type for encoding".to_string()),
    }
    Ok(buf)
}

/// Convert a parsed MQTT packet to an Elle struct value.
fn packet_to_value(packet: &Packet) -> ElleValue {
    let a = api();
    match packet {
        Packet::ConnAck(p) => {
            let code = match p.code {
                v4::ConnectReturnCode::Success => 0,
                v4::ConnectReturnCode::RefusedProtocolVersion => 1,
                v4::ConnectReturnCode::BadClientId => 2,
                v4::ConnectReturnCode::ServiceUnavailable => 3,
                v4::ConnectReturnCode::BadUserNamePassword => 4,
                v4::ConnectReturnCode::NotAuthorized => 5,
            };
            a.build_struct(&[
                ("type", a.keyword("connack")),
                ("session-present", a.boolean(p.session_present)),
                ("code", a.int(code)),
            ])
        }
        Packet::Publish(p) => {
            let packet_id = if p.pkid == 0 {
                a.nil()
            } else {
                a.int(p.pkid as i64)
            };
            a.build_struct(&[
                ("type", a.keyword("publish")),
                ("topic", a.string(p.topic.as_str())),
                ("payload", a.bytes(&p.payload)),
                ("qos", a.int(qos_to_int(p.qos))),
                ("retain", a.boolean(p.retain)),
                ("packet-id", packet_id),
            ])
        }
        Packet::SubAck(p) => {
            let codes: Vec<ElleValue> = p
                .return_codes
                .iter()
                .map(|c| match c {
                    v4::SubscribeReasonCode::Success(qos) => a.int(qos_to_int(*qos)),
                    v4::SubscribeReasonCode::Failure => a.int(128),
                })
                .collect();
            a.build_struct(&[
                ("type", a.keyword("suback")),
                ("packet-id", a.int(p.pkid as i64)),
                ("codes", a.array(&codes)),
            ])
        }
        Packet::UnsubAck(p) => {
            a.build_struct(&[
                ("type", a.keyword("unsuback")),
                ("packet-id", a.int(p.pkid as i64)),
            ])
        }
        Packet::PubAck(p) => {
            a.build_struct(&[
                ("type", a.keyword("puback")),
                ("packet-id", a.int(p.pkid as i64)),
            ])
        }
        Packet::PingResp => {
            a.build_struct(&[("type", a.keyword("pingresp"))])
        }
        _ => {
            a.build_struct(&[("type", a.keyword("unknown"))])
        }
    }
}

/// Helper to get a struct field by keyword name via the stable ABI.
fn struct_get_kw(val: ElleValue, key: &str) -> ElleValue {
    let a = api();
    a.get_struct_field(val, key)
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

extern "C" fn prim_mqtt_state(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let mut protocol = 4u8;
    let mut keep_alive = 60u16;

    if nargs > 0 {
        let arg0 = unsafe { a.arg(args, nargs, 0) };
        if a.check_struct(arg0) {
            let proto_val = struct_get_kw(arg0, "protocol");
            if let Some(i) = a.get_int(proto_val) {
                if i == 4 || i == 5 {
                    protocol = i as u8;
                } else {
                    return mqtt_err("mqtt/state", "protocol must be 4 or 5");
                }
            }
            let ka_val = struct_get_kw(arg0, "keep-alive");
            if let Some(i) = a.get_int(ka_val) {
                keep_alive = i as u16;
            }
        }
    }

    let state = MqttState {
        protocol: Cell::new(protocol),
        keep_alive: Cell::new(keep_alive),
        next_packet_id: Cell::new(1),
        incoming: RefCell::new(bytes::BytesMut::new()),
        packets: RefCell::new(VecDeque::new()),
        connected: Cell::new(false),
    };
    a.ok(a.external("mqtt-state", state))
}

extern "C" fn prim_mqtt_encode_connect(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "mqtt/encode-connect";
    let st = match get_state(args, nargs, 0, name) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let opts = unsafe { a.arg(args, nargs, 1) };
    if !a.check_struct(opts) {
        return a.err("type-error", &format!("{}: expected struct for opts", name));
    }

    let client_id_val = struct_get_kw(opts, "client-id");
    let client_id = a.get_string(client_id_val)
        .unwrap_or("")
        .to_string();

    let clean_session_val = struct_get_kw(opts, "clean-session");
    let clean_session = a.get_bool(clean_session_val).unwrap_or(true);

    let mut connect = v4::Connect::new(&client_id);
    connect.keep_alive = st.keep_alive.get();
    connect.clean_session = clean_session;

    let username_val = struct_get_kw(opts, "username");
    if let Some(u) = a.get_string(username_val) {
        let password_val = struct_get_kw(opts, "password");
        let password = a.get_string(password_val)
            .unwrap_or("")
            .to_string();
        connect.login = Some(v4::Login {
            username: u.to_string(),
            password,
        });
    }

    let packet = Packet::Connect(connect);
    match encode_packet(&packet) {
        Ok(data) => a.ok(a.bytes(&data)),
        Err(e) => mqtt_err(name, e),
    }
}

extern "C" fn prim_mqtt_encode_publish(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "mqtt/encode-publish";
    let st = match get_state(args, nargs, 0, name) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let topic_val = unsafe { a.arg(args, nargs, 1) };
    let topic = match a.get_string(topic_val) {
        Some(s) => s.to_string(),
        None => {
            return a.err("type-error", &format!("{}: expected string for topic", name));
        }
    };

    let payload_val = unsafe { a.arg(args, nargs, 2) };
    let payload: Vec<u8> = if let Some(b) = a.get_bytes(payload_val) {
        b.to_vec()
    } else if let Some(s) = a.get_string(payload_val) {
        s.as_bytes().to_vec()
    } else {
        return a.err(
            "type-error",
            &format!("{}: expected bytes or string for payload", name),
        );
    };

    let mut qos = QoS::AtMostOnce;
    let mut retain = false;

    if nargs > 3 {
        let opts = unsafe { a.arg(args, nargs, 3) };
        if a.check_struct(opts) {
            let qos_val = struct_get_kw(opts, "qos");
            if let Some(i) = a.get_int(qos_val) {
                qos = match qos_from_int(i) {
                    Some(q) => q,
                    None => return mqtt_err(name, format!("invalid QoS level: {}", i)),
                };
            }
            let retain_val = struct_get_kw(opts, "retain");
            if let Some(b) = a.get_bool(retain_val) {
                retain = b;
            }
        }
    }

    let pkid = if qos != QoS::AtMostOnce {
        let id = st.next_packet_id.get();
        st.next_packet_id
            .set(if id == u16::MAX { 1 } else { id + 1 });
        id
    } else {
        0
    };

    let mut publish = v4::Publish::new(&topic, qos, payload);
    publish.pkid = pkid;
    publish.retain = retain;

    let packet = Packet::Publish(publish);
    match encode_packet(&packet) {
        Ok(data) => a.ok(a.bytes(&data)),
        Err(e) => mqtt_err(name, e),
    }
}

extern "C" fn prim_mqtt_encode_subscribe(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "mqtt/encode-subscribe";
    let st = match get_state(args, nargs, 0, name) {
        Ok(s) => s,
        Err(e) => return e,
    };

    // args[1] is an array of [topic qos] pairs
    let topics_arg = unsafe { a.arg(args, nargs, 1) };
    let topics_len = match a.get_array_len(topics_arg) {
        Some(l) => l,
        None => {
            return a.err(
                "type-error",
                &format!("{}: expected array of [topic qos] pairs", name),
            );
        }
    };

    let pkid = st.next_packet_id.get();
    st.next_packet_id
        .set(if pkid == u16::MAX { 1 } else { pkid + 1 });

    let mut subscribe = v4::Subscribe::new("", QoS::AtMostOnce); // placeholder
    subscribe.pkid = pkid;
    subscribe.filters.clear();

    for i in 0..topics_len {
        let item = a.get_array_item(topics_arg, i);
        let pair_len = match a.get_array_len(item) {
            Some(l) => l,
            None => return mqtt_err(name, "each topic must be [topic qos]"),
        };
        if pair_len < 2 {
            return mqtt_err(name, "each topic must be [topic qos]");
        }
        let topic_val = a.get_array_item(item, 0);
        let qos_val = a.get_array_item(item, 1);
        let topic = match a.get_string(topic_val) {
            Some(s) => s.to_string(),
            None => return mqtt_err(name, "topic must be a string"),
        };
        let qos = match a.get_int(qos_val).and_then(qos_from_int) {
            Some(q) => q,
            None => return mqtt_err(name, "qos must be 0, 1, or 2"),
        };
        subscribe
            .filters
            .push(v4::SubscribeFilter { path: topic, qos });
    }

    let packet = Packet::Subscribe(subscribe);
    match encode_packet(&packet) {
        Ok(data) => a.ok(a.bytes(&data)),
        Err(e) => mqtt_err(name, e),
    }
}

extern "C" fn prim_mqtt_encode_unsubscribe(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "mqtt/encode-unsubscribe";
    let st = match get_state(args, nargs, 0, name) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let topics_arg = unsafe { a.arg(args, nargs, 1) };
    let topics_len = match a.get_array_len(topics_arg) {
        Some(l) => l,
        None => {
            return a.err(
                "type-error",
                &format!("{}: expected array of topic strings", name),
            );
        }
    };

    let pkid = st.next_packet_id.get();
    st.next_packet_id
        .set(if pkid == u16::MAX { 1 } else { pkid + 1 });

    let mut topics = Vec::with_capacity(topics_len);
    for i in 0..topics_len {
        let item = a.get_array_item(topics_arg, i);
        match a.get_string(item) {
            Some(s) => topics.push(s.to_string()),
            None => return mqtt_err(name, "each topic must be a string"),
        }
    }

    let mut unsub = v4::Unsubscribe::new(topics[0].clone());
    unsub.pkid = pkid;
    unsub.topics = topics;

    let packet = Packet::Unsubscribe(unsub);
    match encode_packet(&packet) {
        Ok(data) => a.ok(a.bytes(&data)),
        Err(e) => mqtt_err(name, e),
    }
}

extern "C" fn prim_mqtt_encode_ping(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "mqtt/encode-ping";
    let _st = match get_state(args, nargs, 0, name) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let packet = Packet::PingReq;
    match encode_packet(&packet) {
        Ok(data) => a.ok(a.bytes(&data)),
        Err(e) => mqtt_err(name, e),
    }
}

extern "C" fn prim_mqtt_encode_disconnect(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "mqtt/encode-disconnect";
    let _st = match get_state(args, nargs, 0, name) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let packet = Packet::Disconnect;
    match encode_packet(&packet) {
        Ok(data) => a.ok(a.bytes(&data)),
        Err(e) => mqtt_err(name, e),
    }
}

extern "C" fn prim_mqtt_encode_puback(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "mqtt/encode-puback";
    let _st = match get_state(args, nargs, 0, name) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let pkid_val = unsafe { a.arg(args, nargs, 1) };
    let pkid = match a.get_int(pkid_val) {
        Some(i) if i > 0 && i <= u16::MAX as i64 => i as u16,
        _ => return mqtt_err(name, "packet-id must be a positive integer"),
    };
    let puback = v4::PubAck::new(pkid);
    let packet = Packet::PubAck(puback);
    match encode_packet(&packet) {
        Ok(data) => a.ok(a.bytes(&data)),
        Err(e) => mqtt_err(name, e),
    }
}

extern "C" fn prim_mqtt_feed(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "mqtt/feed";
    let st = match get_state(args, nargs, 0, name) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let data_val = unsafe { a.arg(args, nargs, 1) };
    let new_data: Vec<u8> = if let Some(b) = a.get_bytes(data_val) {
        b.to_vec()
    } else {
        return a.err(
            "type-error",
            &format!("{}: expected bytes, got {}", name, a.type_name(data_val)),
        );
    };

    let mut incoming = st.incoming.borrow_mut();
    incoming.extend_from_slice(&new_data);

    // Try to parse as many packets as possible from the buffer
    let mut packets = st.packets.borrow_mut();
    loop {
        match mqttbytes::v4::read(&mut incoming, 65536) {
            Ok(packet) => {
                // Track CONNACK for connected state
                if let Packet::ConnAck(ref ack) = packet {
                    if matches!(ack.code, v4::ConnectReturnCode::Success) {
                        st.connected.set(true);
                    }
                }
                packets.push_back(packet);
            }
            Err(mqttbytes::Error::InsufficientBytes(_)) => break,
            Err(e) => return mqtt_err(name, e),
        }
    }

    a.ok(a.int(packets.len() as i64))
}

extern "C" fn prim_mqtt_poll(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "mqtt/poll";
    let st = match get_state(args, nargs, 0, name) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let mut packets = st.packets.borrow_mut();
    match packets.pop_front() {
        Some(packet) => a.ok(packet_to_value(&packet)),
        None => a.ok(a.nil()),
    }
}

extern "C" fn prim_mqtt_poll_all(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "mqtt/poll-all";
    let st = match get_state(args, nargs, 0, name) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let mut packets = st.packets.borrow_mut();
    let vals: Vec<ElleValue> = packets.drain(..).map(|p| packet_to_value(&p)).collect();
    a.ok(a.array(&vals))
}

extern "C" fn prim_mqtt_next_packet_id(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "mqtt/next-packet-id";
    let st = match get_state(args, nargs, 0, name) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let id = st.next_packet_id.get();
    st.next_packet_id
        .set(if id == u16::MAX { 1 } else { id + 1 });
    a.ok(a.int(id as i64))
}

extern "C" fn prim_mqtt_connected(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "mqtt/connected?";
    let st = match get_state(args, nargs, 0, name) {
        Ok(s) => s,
        Err(e) => return e,
    };
    a.ok(a.boolean(st.connected.get()))
}

extern "C" fn prim_mqtt_keep_alive(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "mqtt/keep-alive";
    let st = match get_state(args, nargs, 0, name) {
        Ok(s) => s,
        Err(e) => return e,
    };
    a.ok(a.int(st.keep_alive.get() as i64))
}

// ---------------------------------------------------------------------------
// Registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::range("mqtt/state", prim_mqtt_state, SIG_ERROR, 0, 1,
        "Create MQTT state. Optional opts: {:protocol 4 :keep-alive 60}", "mqtt",
        r#"(mqtt/state {:protocol 4 :keep-alive 60})"#),
    EllePrimDef::exact("mqtt/encode-connect", prim_mqtt_encode_connect, SIG_ERROR, 2,
        "Encode a CONNECT packet. Returns bytes to send over TCP.", "mqtt",
        r#"(mqtt/encode-connect st {:client-id "my-client"})"#),
    EllePrimDef::range("mqtt/encode-publish", prim_mqtt_encode_publish, SIG_ERROR, 3, 4,
        "Encode a PUBLISH packet. Optional opts: {:qos 1 :retain true}", "mqtt",
        r#"(mqtt/encode-publish st "topic" "hello" {:qos 1})"#),
    EllePrimDef::exact("mqtt/encode-subscribe", prim_mqtt_encode_subscribe, SIG_ERROR, 2,
        "Encode a SUBSCRIBE packet. Topics: [[\"topic\" 0] ...]", "mqtt",
        r#"(mqtt/encode-subscribe st [["sensors/#" 0]])"#),
    EllePrimDef::exact("mqtt/encode-unsubscribe", prim_mqtt_encode_unsubscribe, SIG_ERROR, 2,
        "Encode an UNSUBSCRIBE packet. Topics: [\"topic\" ...]", "mqtt",
        r#"(mqtt/encode-unsubscribe st ["sensors/#"])"#),
    EllePrimDef::exact("mqtt/encode-ping", prim_mqtt_encode_ping, SIG_ERROR, 1,
        "Encode a PINGREQ packet.", "mqtt",
        r#"(mqtt/encode-ping st)"#),
    EllePrimDef::exact("mqtt/encode-disconnect", prim_mqtt_encode_disconnect, SIG_ERROR, 1,
        "Encode a DISCONNECT packet.", "mqtt",
        r#"(mqtt/encode-disconnect st)"#),
    EllePrimDef::exact("mqtt/encode-puback", prim_mqtt_encode_puback, SIG_ERROR, 2,
        "Encode a PUBACK packet for a given packet ID.", "mqtt",
        r#"(mqtt/encode-puback st 1)"#),
    EllePrimDef::exact("mqtt/feed", prim_mqtt_feed, SIG_ERROR, 2,
        "Feed raw TCP bytes into the MQTT parser. Returns number of queued packets.", "mqtt",
        r#"(mqtt/feed st data)"#),
    EllePrimDef::exact("mqtt/poll", prim_mqtt_poll, SIG_OK, 1,
        "Drain one parsed packet as a struct, or nil if none.", "mqtt",
        r#"(mqtt/poll st)"#),
    EllePrimDef::exact("mqtt/poll-all", prim_mqtt_poll_all, SIG_OK, 1,
        "Drain all parsed packets as an array.", "mqtt",
        r#"(mqtt/poll-all st)"#),
    EllePrimDef::exact("mqtt/next-packet-id", prim_mqtt_next_packet_id, SIG_OK, 1,
        "Get and increment the packet ID counter.", "mqtt",
        r#"(mqtt/next-packet-id st)"#),
    EllePrimDef::exact("mqtt/connected?", prim_mqtt_connected, SIG_OK, 1,
        "True after a successful CONNACK has been received.", "mqtt",
        r#"(mqtt/connected? st)"#),
    EllePrimDef::exact("mqtt/keep-alive", prim_mqtt_keep_alive, SIG_OK, 1,
        "Return the keep-alive interval in seconds.", "mqtt",
        r#"(mqtt/keep-alive st)"#),
];
