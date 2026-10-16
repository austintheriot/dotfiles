//! Pins the JSON shape of the socket protocol. The fixture files are
//! append-only: a line that stops decoding means an old CLI can no longer
//! reach a new daemon.

use agent_notify_core::wire::{
    AgentEvent, DecodeError, MAX_LINE_BYTES, Reply, Request, SessionSource, decode_request,
    encode_line,
};

const REQUESTS: &str = include_str!("fixtures/wire-v1.jsonl");
const REPLIES: &str = include_str!("fixtures/reply-v1.jsonl");

fn as_value(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("valid JSON")
}

#[test]
fn every_request_fixture_decodes_and_re_encodes_to_the_same_json() {
    let mut count = 0;
    for line in REQUESTS.lines().filter(|line| !line.trim().is_empty()) {
        let request = decode_request(line).unwrap_or_else(|error| panic!("{error:?}: {line}"));
        let encoded = encode_line(&request).expect("encodes");
        assert!(encoded.ends_with('\n'));
        assert_eq!(as_value(&encoded), as_value(line), "round trip changed {line}");
        count += 1;
    }
    assert_eq!(count, 24, "a fixture line went missing");
}

#[test]
fn every_reply_fixture_round_trips() {
    for line in REPLIES.lines().filter(|line| !line.trim().is_empty()) {
        let reply: Reply = serde_json::from_str(line).expect("decodes");
        assert_eq!(as_value(&serde_json::to_string(&reply).expect("encodes")), as_value(line));
    }
}

fn first_request_with(change: impl Fn(&mut serde_json::Value)) -> String {
    let mut value = as_value(REQUESTS.lines().next().expect("a first line"));
    change(&mut value);
    value.to_string()
}

#[test]
fn unknown_fields_are_ignored() {
    let line = first_request_with(|value| {
        value["future_field"] = serde_json::json!(1);
        value["origin"]["future_origin_field"] = serde_json::json!("x");
    });
    assert!(decode_request(&line).is_ok());
}

#[test]
fn versions_outside_the_accepted_range_are_rejected() {
    for version in [0, 2] {
        let line = first_request_with(|value| value["v"] = serde_json::json!(version));
        assert_eq!(decode_request(&line), Err(DecodeError::UnsupportedVersion { version }));
    }
}

#[test]
fn a_missing_version_is_malformed() {
    let line = first_request_with(|value| {
        value.as_object_mut().expect("an object").remove("v");
    });
    assert!(matches!(decode_request(&line), Err(DecodeError::Malformed { .. })));
}

#[test]
fn unknown_request_and_event_kinds_are_reported_as_unknown_kind() {
    let unknown_request = first_request_with(|value| value["type"] = serde_json::json!("teleport"));
    assert_eq!(decode_request(&unknown_request), Err(DecodeError::UnknownKind));
    let unknown_event =
        first_request_with(|value| value["event"]["type"] = serde_json::json!("future_event"));
    assert_eq!(decode_request(&unknown_event), Err(DecodeError::UnknownKind));
}

#[test]
fn an_oversized_line_is_rejected_before_parsing() {
    let line = "x".repeat(MAX_LINE_BYTES + 1);
    assert_eq!(decode_request(&line), Err(DecodeError::LineTooLong { bytes: MAX_LINE_BYTES + 1 }));
}

#[test]
fn text_fields_are_cleaned_on_decode() {
    let line = first_request_with(|value| {
        value["event"] = serde_json::json!({"type": "explicit", "kind": "info", "body": "ok\u{202e}gpj.exe"});
    });
    let Ok(Request::Agent { event: AgentEvent::Explicit { body, .. }, .. }) = decode_request(&line) else {
        panic!("expected an explicit event");
    };
    assert_eq!(body.as_str(), "okgpj.exe");
}

#[test]
fn an_invalid_pane_id_is_malformed() {
    let line = first_request_with(|value| value["origin"]["pane"]["pane"] = serde_json::json!("12"));
    assert!(matches!(decode_request(&line), Err(DecodeError::Malformed { .. })));
}

#[test]
fn unknown_values_of_open_enums_decode_to_unknown() {
    let line = first_request_with(|value| value["event"]["source"] = serde_json::json!("teleport"));
    let Ok(Request::Agent { event: AgentEvent::SessionStarted { source }, .. }) = decode_request(&line) else {
        panic!("expected a session start");
    };
    assert_eq!(source, SessionSource::Unknown);
}

#[test]
fn unknown_values_of_closed_enums_reject_the_request() {
    let unknown_kind = first_request_with(|value| {
        value["event"] = serde_json::json!({"type": "explicit", "kind": "urgent", "body": "x"});
    });
    assert!(decode_request(&unknown_kind).is_err());
    let unknown_quiet = first_request_with(|value| {
        value["event"] = serde_json::json!({"type": "quiet", "until": "never", "max_minutes": 5});
    });
    assert!(decode_request(&unknown_quiet).is_err());
    let unknown_head =
        first_request_with(|value| value["origin"]["repo"]["head"]["type"] = serde_json::json!("tag"));
    assert!(decode_request(&unknown_head).is_err());
}
