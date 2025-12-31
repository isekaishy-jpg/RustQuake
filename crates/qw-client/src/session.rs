use qw_common::OobMessage;

use crate::handshake::{build_connect, build_getchallenge, parse_challenge, ConnectRequest};
use qw_common::InfoString;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SessionState {
    Disconnected,
    ChallengeSent,
    ConnectSent,
    Connected,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub state: SessionState,
    pub qport: u16,
    pub userinfo: String,
    pub challenge: Option<i32>,
}

impl Session {
    pub fn new(qport: u16, userinfo: impl Into<String>) -> Self {
        Self {
            state: SessionState::Disconnected,
            qport,
            userinfo: userinfo.into(),
            challenge: None,
        }
    }

    #[allow(dead_code)]
    pub fn new_with_info(qport: u16, info: InfoString) -> Self {
        Self::new(qport, info.as_str().to_string())
    }

    pub fn start(&mut self) -> Vec<u8> {
        self.state = SessionState::ChallengeSent;
        build_getchallenge()
    }

    pub fn handle_oob(&mut self, msg: &OobMessage) -> Option<Vec<u8>> {
        match msg {
            OobMessage::Challenge(_) => {
                let challenge = parse_challenge(msg)?;
                self.challenge = Some(challenge);
                self.state = SessionState::ConnectSent;
                let request = ConnectRequest::new(self.qport, challenge, self.userinfo.clone());
                Some(build_connect(&request))
            }
            OobMessage::Connection(_) => {
                self.state = SessionState::Connected;
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qw_common::{parse_oob_message, out_of_band_payload, S2C_CHALLENGE, S2C_CONNECTION, build_out_of_band};

    #[test]
    fn handshake_flow() {
        let mut session = Session::new(27001, "\\name\\player");
        let packet = session.start();
        assert_eq!(session.state, SessionState::ChallengeSent);
        assert_eq!(out_of_band_payload(&packet).unwrap(), b"getchallenge\n");

        let challenge_packet = build_out_of_band(&[S2C_CHALLENGE, b'1', b'2', b'3', 0]);
        let payload = out_of_band_payload(&challenge_packet).unwrap();
        let msg = parse_oob_message(payload).unwrap();
        let connect_packet = session.handle_oob(&msg).unwrap();
        assert_eq!(session.state, SessionState::ConnectSent);
        assert!(out_of_band_payload(&connect_packet).unwrap().starts_with(b"connect"));

        let conn_packet = build_out_of_band(&[S2C_CONNECTION, 0]);
        let payload = out_of_band_payload(&conn_packet).unwrap();
        let msg = parse_oob_message(payload).unwrap();
        session.handle_oob(&msg);
        assert_eq!(session.state, SessionState::Connected);
    }
}
