use qw_common::{OobMessage, PROTOCOL_VERSION, build_out_of_band, out_of_band_payload};

#[derive(Debug, Clone)]
pub struct ConnectRequest {
    pub protocol: i32,
    pub qport: u16,
    pub challenge: i32,
    pub userinfo: String,
}

impl ConnectRequest {
    pub fn new(qport: u16, challenge: i32, userinfo: impl Into<String>) -> Self {
        Self {
            protocol: PROTOCOL_VERSION,
            qport,
            challenge,
            userinfo: userinfo.into(),
        }
    }
}

pub fn build_getchallenge() -> Vec<u8> {
    build_out_of_band(b"getchallenge\n")
}

pub fn build_connect(request: &ConnectRequest) -> Vec<u8> {
    let text = format!(
        "connect {} {} {} \"{}\"\n",
        request.protocol, request.qport, request.challenge, request.userinfo
    );
    build_out_of_band(text.as_bytes())
}

pub fn parse_challenge(message: &OobMessage) -> Option<i32> {
    if let OobMessage::Challenge(text) = message {
        text.trim().parse::<i32>().ok()
    } else {
        None
    }
}

#[allow(dead_code)]
pub fn is_oob_packet(packet: &[u8]) -> bool {
    out_of_band_payload(packet).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use qw_common::{OobMessage, S2C_CHALLENGE, out_of_band_payload};

    #[test]
    fn builds_getchallenge() {
        let packet = build_getchallenge();
        let payload = out_of_band_payload(&packet).unwrap();
        assert_eq!(payload, b"getchallenge\n");
    }

    #[test]
    fn builds_connect() {
        let req = ConnectRequest::new(27001, 12345, "\\name\\player");
        let packet = build_connect(&req);
        let payload = out_of_band_payload(&packet).unwrap();
        let text = String::from_utf8_lossy(payload);
        assert!(text.contains("connect"));
        assert!(text.contains("12345"));
    }

    #[test]
    fn parses_challenge() {
        let msg = OobMessage::Challenge("42".to_string());
        assert_eq!(parse_challenge(&msg), Some(42));

        let packet = build_out_of_band(&[S2C_CHALLENGE, b'1', b'2', 0]);
        let payload = out_of_band_payload(&packet).unwrap();
        let parsed = qw_common::parse_oob_message(payload).unwrap();
        assert_eq!(parse_challenge(&parsed), Some(12));
    }
}
