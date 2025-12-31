// Out-of-band packet helpers (0xFFFFFFFF header).

use crate::protocol::{
    A2A_ACK, A2A_ECHO, A2A_NACK, A2A_PING, A2C_CLIENT_COMMAND, A2C_PRINT, S2C_CHALLENGE,
    S2C_CONNECTION, S2M_HEARTBEAT, S2M_SHUTDOWN,
};

const OOB_HEADER: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

pub fn build_out_of_band(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&OOB_HEADER);
    out.extend_from_slice(payload);
    out
}

pub fn is_out_of_band(packet: &[u8]) -> bool {
    packet.len() >= 4 && packet[0..4] == OOB_HEADER
}

pub fn out_of_band_payload(packet: &[u8]) -> Option<&[u8]> {
    if is_out_of_band(packet) {
        Some(&packet[4..])
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OobMessage {
    Challenge(String),
    Connection(String),
    Print(String),
    ClientCommand(String),
    Echo(String),
    Ping,
    Ack,
    Nack(String),
    Heartbeat(String),
    Shutdown(String),
    Unknown(u8, String),
}

pub fn parse_oob_message(payload: &[u8]) -> Option<OobMessage> {
    if payload.is_empty() {
        return None;
    }
    let code = payload[0];
    let text = parse_oob_text(&payload[1..]);
    let msg = match code {
        S2C_CHALLENGE => OobMessage::Challenge(text),
        S2C_CONNECTION => OobMessage::Connection(text),
        A2C_PRINT => OobMessage::Print(text),
        A2C_CLIENT_COMMAND => OobMessage::ClientCommand(text),
        A2A_ECHO => OobMessage::Echo(text),
        A2A_PING => OobMessage::Ping,
        A2A_ACK => OobMessage::Ack,
        A2A_NACK => OobMessage::Nack(text),
        S2M_HEARTBEAT => OobMessage::Heartbeat(text),
        S2M_SHUTDOWN => OobMessage::Shutdown(text),
        _ => OobMessage::Unknown(code, text),
    };
    Some(msg)
}

fn parse_oob_text(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|b| *b == 0 || *b == b'\n')
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_of_band_round_trip() {
        let payload = b"getchallenge";
        let packet = build_out_of_band(payload);
        assert!(is_out_of_band(&packet));
        assert_eq!(out_of_band_payload(&packet).unwrap(), payload);
    }

    #[test]
    fn parses_challenge_message() {
        let payload = [S2C_CHALLENGE, b'1', b'2', b'3', 0];
        let msg = parse_oob_message(&payload).unwrap();
        assert_eq!(msg, OobMessage::Challenge("123".to_string()));
    }
}
