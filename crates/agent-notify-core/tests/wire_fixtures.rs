//! Pins the JSON shape of the socket protocol. The fixture files are
//! append-only: a line that stops decoding means an old CLI can no longer
//! reach a new daemon.

use agent_notify_core::wire::{
    AgentEvent, DecodeError, Kind, MAX_LINE_BYTES, Reply, ReplyCode, Request, SessionSource,
    decode_request, encode_line,
};
use serde_json::json;

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
    assert_eq!(count, 25, "a fixture line went missing");
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

fn request_line(number: usize) -> serde_json::Value {
    as_value(REQUESTS.lines().nth(number - 1).expect("a fixture line"))
}

fn with(mut value: serde_json::Value, change: impl Fn(&mut serde_json::Value)) -> String {
    change(&mut value);
    value.to_string()
}

/// A closed enum takes `known` and refuses `unknown` with `bad_request`, so
/// the CLI falls back. The known value is the positive control: without it a
/// request broken some other way would also pass.
fn assert_closed(name: &str, request: &serde_json::Value, set: impl Fn(&mut serde_json::Value, &str), known: &str, unknown: &str) {
    let accepted = with(request.clone(), |value| set(value, known));
    assert!(decode_request(&accepted).is_ok(), "{name}: {known:?} must decode");
    let refused = with(request.clone(), |value| set(value, unknown));
    let error = decode_request(&refused).expect_err(name);
    assert!(matches!(error, DecodeError::Malformed { .. }), "{name}: {error:?}");
    assert_eq!(error.reply_code(), ReplyCode::BadRequest, "{name}");
}

#[test]
fn an_unknown_kind_value_is_a_bad_request() {
    let explicit = |value: &mut serde_json::Value, kind: &str| value["event"] = json!({"type": "explicit", "kind": kind, "body": "x"});
    assert_closed("Kind", &request_line(1), explicit, "attention", "urgent");
}

#[test]
fn an_unknown_quiet_until_value_is_a_bad_request() {
    let quiet = |value: &mut serde_json::Value, until: &str| value["event"] = json!({"type": "quiet", "until": until, "max_minutes": 5});
    assert_closed("QuietUntil", &request_line(1), quiet, "next_prompt", "never");
}

#[test]
fn an_unknown_dnd_command_is_a_bad_request() {
    let dnd = |value: &mut serde_json::Value, command: &str| value["control"]["command"] = json!({"type": command});
    assert_closed("DndCommand", &request_line(17), dnd, "off", "sideways");
}

#[test]
fn an_unknown_repo_head_is_a_bad_request() {
    let head = |value: &mut serde_json::Value, head: &str| value["origin"]["repo"]["head"] = json!({"type": head, "name": "v1", "short_sha": "a1b2c3d"});
    let line = request_line(1);
    assert!(line["origin"]["repo"]["main_repo_name"].is_string(), "the fixture must carry a full repo");
    assert_closed("RepoHead", &line, head, "detached", "tag");
}

#[test]
fn attention_outranks_info() {
    assert!(Kind::Attention > Kind::Info);
    let mut kinds = vec![Kind::Attention, Kind::Info, Kind::Attention];
    kinds.sort();
    assert_eq!(kinds, vec![Kind::Info, Kind::Attention, Kind::Attention]);
}

#[test]
fn a_version_that_is_not_a_u16_is_malformed() {
    for version in [json!(65_536), json!(-1), json!(1.5)] {
        let line = first_request_with(|value| value["v"] = version.clone());
        let error = decode_request(&line).expect_err("an out-of-range v");
        assert!(matches!(error, DecodeError::Malformed { .. }), "{version}: {error:?}");
        assert_eq!(error.reply_code(), ReplyCode::BadRequest, "{version}");
    }
}

/// A valid request padded with an ignored field to exactly `bytes` bytes.
fn request_of_length(bytes: usize) -> String {
    let unpadded = first_request_with(|value| value["pad"] = json!("")).len();
    let line = first_request_with(|value| value["pad"] = json!("x".repeat(bytes - unpadded)));
    assert_eq!(line.len(), bytes);
    line
}

#[test]
fn a_line_of_exactly_the_limit_decodes_and_one_byte_more_is_too_long() {
    assert!(decode_request(&request_of_length(MAX_LINE_BYTES)).is_ok());
    let over = decode_request(&request_of_length(MAX_LINE_BYTES + 1)).expect_err("one byte over");
    assert_eq!(over, DecodeError::LineTooLong { bytes: MAX_LINE_BYTES + 1 });
    assert_eq!(over.reply_code(), ReplyCode::LineTooLong);
}

#[test]
fn the_limit_counts_the_line_without_its_newline() {
    let at_limit = request_of_length(MAX_LINE_BYTES);
    assert_eq!(decode_request(&format!("{at_limit}\n")), Err(DecodeError::LineTooLong { bytes: MAX_LINE_BYTES + 1 }), "the reader strips the newline before decoding");
    let encoded = encode_line(&decode_request(&at_limit).expect("decodes")).expect("encodes");
    assert!(decode_request(encoded.trim_end_matches('\n')).is_ok());
}

#[test]
fn a_newer_version_is_reported_even_when_the_rest_does_not_parse() {
    let removed_field = first_request_with(|value| {
        value["v"] = serde_json::json!(2);
        value["origin"].as_object_mut().expect("an object").remove("cwd");
    });
    assert_eq!(decode_request(&removed_field), Err(DecodeError::UnsupportedVersion { version: 2 }));
    let new_type = first_request_with(|value| {
        value["v"] = serde_json::json!(2);
        value["type"] = serde_json::json!("teleport");
    });
    assert_eq!(decode_request(&new_type), Err(DecodeError::UnsupportedVersion { version: 2 }));
}

#[test]
fn a_non_numeric_version_is_malformed() {
    let line = first_request_with(|value| value["v"] = serde_json::json!("1"));
    assert!(matches!(decode_request(&line), Err(DecodeError::Malformed { .. })));
}
