use qw_common::{
    build_out_of_band, is_out_of_band, out_of_band_payload, parse_oob_message, parse_svc_stream,
    Netchan, NetchanError, OobMessage, QuakeFs, SvcMessage, SvcParseError,
};
use crate::handshake;

#[derive(Debug)]
#[allow(dead_code)]
pub enum ClientPacket {
    OutOfBand(OobMessage),
    Messages(Vec<SvcMessage>),
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum ClientError {
    Netchan(NetchanError),
    Parse(SvcParseError),
}

impl From<NetchanError> for ClientError {
    fn from(err: NetchanError) -> Self {
        ClientError::Netchan(err)
    }
}

impl From<SvcParseError> for ClientError {
    fn from(err: SvcParseError) -> Self {
        ClientError::Parse(err)
    }
}

#[allow(dead_code)]
pub struct Client {
    pub fs: QuakeFs,
    pub netchan: Netchan,
}

#[allow(dead_code)]
impl Client {
    pub fn new(qport: u16) -> Self {
        Self {
            fs: QuakeFs::new(),
            netchan: Netchan::new(qport),
        }
    }

    pub fn handle_packet(&mut self, packet: &[u8]) -> Result<ClientPacket, ClientError> {
        if is_out_of_band(packet) {
            let payload = out_of_band_payload(packet).unwrap_or(&[]);
            let message = parse_oob_message(payload)
                .unwrap_or_else(|| OobMessage::Unknown(0, String::new()));
            return Ok(ClientPacket::OutOfBand(message));
        }

        let payload = self.netchan.process_packet(packet, false)?;
        let mut reader = qw_common::MsgReader::new(payload);
        let messages = parse_svc_stream(&mut reader)?;
        Ok(ClientPacket::Messages(messages))
    }

    pub fn build_oob_message(text: &str) -> Vec<u8> {
        build_out_of_band(text.as_bytes())
    }

    pub fn build_getchallenge() -> Vec<u8> {
        handshake::build_getchallenge()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qw_common::{SizeBuf, SvcMessage, Svc};

    #[test]
    fn handles_oob() {
        let mut client = Client::new(27001);
        let packet = build_out_of_band(&[qw_common::S2C_CHALLENGE, b'4', b'2', 0]);
        let result = client.handle_packet(&packet).unwrap();
        match result {
            ClientPacket::OutOfBand(OobMessage::Challenge(text)) => assert_eq!(text, "42"),
            _ => panic!("expected out-of-band"),
        }
    }

    #[test]
    fn handles_svc_packet() {
        let mut sender = Netchan::new(27001);
        let mut buf = SizeBuf::new(64);
        buf.write_u8(Svc::Print as u8).unwrap();
        buf.write_u8(2).unwrap();
        buf.write_string(Some("hello")).unwrap();

        let packet = sender.build_packet(buf.as_slice(), false).unwrap();

        let mut client = Client::new(27001);
        let result = client.handle_packet(&packet).unwrap();
        match result {
            ClientPacket::Messages(messages) => {
                assert_eq!(
                    messages,
                    vec![SvcMessage::Print {
                        level: 2,
                        message: "hello".to_string()
                    }]
                );
            }
            _ => panic!("expected svc messages"),
        }
    }
}
