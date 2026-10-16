//! Turns daemon answers into text. Pure: `main` prints the result.

use crate::client::{Delivery, reply_code_name};

/// One line that says why the daemon gave no usable answer.
pub fn delivery_problem(delivery: &Delivery) -> String {
    match delivery {
        Delivery::Acked { .. } => "the daemon answered".to_string(),
        Delivery::Rejected { code: Some(code) } => format!("the daemon refused the request ({})", reply_code_name(*code)),
        Delivery::Rejected { code: None } => "the daemon refused the request".to_string(),
        Delivery::NoSocket(reason) => format!("no socket: {reason}"),
        Delivery::ConnectFailed(kind) => format!("the daemon is not running ({kind})"),
        Delivery::TimedOut => "the daemon did not answer in time".to_string(),
        Delivery::BadReply(message) => format!("the daemon sent a bad reply: {message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_notify_core::wire::ReplyCode;
    use std::io;

    #[test]
    fn every_failure_reads_as_one_line() {
        let cases = [
            (Delivery::Rejected { code: Some(ReplyCode::UnsupportedVersion) }, "the daemon refused the request (unsupported_version)"),
            (Delivery::Rejected { code: None }, "the daemon refused the request"),
            (Delivery::NoSocket("HOME is unset or relative"), "no socket: HOME is unset or relative"),
            (Delivery::ConnectFailed(io::ErrorKind::NotFound), "the daemon is not running (entity not found)"),
            (Delivery::TimedOut, "the daemon did not answer in time"),
            (Delivery::BadReply("expected value".to_string()), "the daemon sent a bad reply: expected value"),
        ];
        for (delivery, line) in cases {
            assert_eq!(delivery_problem(&delivery), line, "{delivery:?}");
        }
    }
}
