use std::io;

use crate::client::{Client, ClientPacket};
use crate::net::NetClient;
use crate::session::Session;
use crate::state::ClientState;
use qw_common::{clc, NetchanError, OobMessage, SizeBuf, SizeBufError, UPDATE_BACKUP, UserCmd};

#[derive(Debug)]
#[allow(dead_code)]
pub enum RunnerError {
    Io(io::Error),
    Client(crate::client::ClientError),
    Netchan(NetchanError),
    Buffer(SizeBufError),
    NotConnected,
}

impl From<io::Error> for RunnerError {
    fn from(err: io::Error) -> Self {
        RunnerError::Io(err)
    }
}

impl From<crate::client::ClientError> for RunnerError {
    fn from(err: crate::client::ClientError) -> Self {
        RunnerError::Client(err)
    }
}

impl From<NetchanError> for RunnerError {
    fn from(err: NetchanError) -> Self {
        RunnerError::Netchan(err)
    }
}

impl From<SizeBufError> for RunnerError {
    fn from(err: SizeBufError) -> Self {
        RunnerError::Buffer(err)
    }
}

pub struct ClientRunner {
    pub net: NetClient,
    pub session: Session,
    pub client: Client,
    pub state: ClientState,
}

impl ClientRunner {
    pub fn new(net: NetClient, session: Session) -> Self {
        let qport = session.qport;
        Self {
            net,
            session,
            client: Client::new(qport),
            state: ClientState::new(),
        }
    }

    pub fn start_connect(&mut self) -> Result<(), RunnerError> {
        let packet = self.session.start();
        self.net.send(&packet)?;
        Ok(())
    }

    pub fn poll_once(&mut self, buf: &mut [u8]) -> Result<Option<ClientPacket>, RunnerError> {
        let Some((size, _)) = self.net.recv(buf)? else {
            return Ok(None);
        };
        let packet = &buf[..size];
        let parsed = self.client.handle_packet(packet)?;
        match &parsed {
            ClientPacket::OutOfBand(msg) => {
                if let Some(response) = self.session.handle_oob(msg) {
                    self.net.send(&response)?;
                }
                if matches!(msg, OobMessage::Connection(_)) {
                    self.session.state = crate::session::SessionState::Connected;
                }
            }
            ClientPacket::Messages(_) => {}
        }

        if let ClientPacket::Messages(messages) = &parsed {
            let incoming_sequence = self.client.netchan.incoming_sequence();
            for message in messages {
                self.state.apply_message(message, incoming_sequence);
            }
        }

        Ok(Some(parsed))
    }

    pub fn send_move(&mut self, cmd: UserCmd) -> Result<(), RunnerError> {
        if self.session.state != crate::session::SessionState::Connected {
            return Err(RunnerError::NotConnected);
        }

        let sequence = self.client.netchan.outgoing_sequence();
        self.state.store_outgoing_cmd(sequence, cmd);

        if self.state.valid_sequence != 0
            && sequence.wrapping_sub(self.state.valid_sequence as u32)
                >= (UPDATE_BACKUP as u32 - 1)
        {
            self.state.valid_sequence = 0;
        }

        let delta_sequence = if self.state.valid_sequence != 0 {
            let delta = (self.state.valid_sequence & 0xff) as u8;
            self.state
                .set_outgoing_delta_sequence(sequence, self.state.valid_sequence);
            Some(delta)
        } else {
            self.state.set_outgoing_delta_sequence(sequence, -1);
            None
        };

        let cmds = [
            self.state.outgoing_cmd(sequence.wrapping_sub(2)),
            self.state.outgoing_cmd(sequence.wrapping_sub(1)),
            self.state.outgoing_cmd(sequence),
        ];

        let message = clc::MoveMessage {
            sequence,
            lost: 0,
            cmds,
            delta_sequence,
        };

        let mut buf = SizeBuf::new(128);
        clc::write_move_message(&mut buf, &message)?;

        let packet = self.client.netchan.build_packet(buf.as_slice(), true)?;
        self.net.send(&packet)?;
        Ok(())
    }

    pub fn send_string_cmd(&mut self, text: &str) -> Result<(), RunnerError> {
        if self.session.state != crate::session::SessionState::Connected {
            return Err(RunnerError::NotConnected);
        }
        let mut buf = SizeBuf::new(128);
        clc::write_string_cmd(&mut buf, text)?;
        let packet = self.client.netchan.build_packet(buf.as_slice(), true)?;
        self.net.send(&packet)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionState;
    use qw_common::{
        build_out_of_band, out_of_band_payload, Clc, MsgReader, Netchan, S2C_CHALLENGE,
        S2C_CONNECTION, SvcMessage, UserCmd, Vec3,
    };
    use std::net::UdpSocket;
    use std::time::Duration;

    #[test]
    fn handshake_round_trip() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();

        let net = NetClient::connect(server_addr).unwrap();
        let session = Session::new(27001, "\\name\\player");
        let mut runner = ClientRunner::new(net, session);

        runner.start_connect().unwrap();

        let mut server_buf = [0u8; 256];
        let (size, client_addr) = server.recv_from(&mut server_buf).unwrap();
        assert_eq!(
            out_of_band_payload(&server_buf[..size]).unwrap(),
            b"getchallenge\n"
        );

        let challenge = build_out_of_band(&[S2C_CHALLENGE, b'1', b'2', b'3', 0]);
        server.send_to(&challenge, client_addr).unwrap();

        let mut client_buf = [0u8; 512];
        for _ in 0..10 {
            let _ = runner.poll_once(&mut client_buf).unwrap();
            if runner.session.state == SessionState::ConnectSent {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(runner.session.state, SessionState::ConnectSent);

        let (size, _) = server.recv_from(&mut server_buf).unwrap();
        let payload = out_of_band_payload(&server_buf[..size]).unwrap();
        let text = String::from_utf8_lossy(payload);
        assert!(text.starts_with("connect"));

        let connection = build_out_of_band(&[S2C_CONNECTION, 0]);
        server.send_to(&connection, client_addr).unwrap();
        for _ in 0..10 {
            let _ = runner.poll_once(&mut client_buf).unwrap();
            if runner.session.state == SessionState::Connected {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(runner.session.state, SessionState::Connected);
    }

    #[test]
    fn applies_state_from_messages() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();

        let net = NetClient::connect(server_addr).unwrap();
        let session = Session::new(27001, "\\name\\player");
        let mut runner = ClientRunner::new(net, session);

        let mut server_chan = qw_common::Netchan::new(27001);
        let mut buf = qw_common::SizeBuf::new(128);
        qw_common::write_svc_message(
            &mut buf,
            &SvcMessage::UpdateUserInfo {
                slot: 0,
                user_id: 7,
                userinfo: "\\name\\unit".to_string(),
            },
        )
        .unwrap();
        let packet = server_chan.build_packet(buf.as_slice(), false).unwrap();

        let local_port = runner.net.local_addr().unwrap().port();
        server
            .send_to(&packet, std::net::SocketAddr::from(([127, 0, 0, 1], local_port)))
            .unwrap();

        let mut client_buf = [0u8; 256];
        for _ in 0..10 {
            let _ = runner.poll_once(&mut client_buf).unwrap();
            if runner.state.players[0].user_id == 7 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(runner.state.players[0].user_id, 7);
    }

    #[test]
    fn sends_move_with_delta() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();

        let net = NetClient::connect(server_addr).unwrap();
        let mut session = Session::new(27001, "\\name\\player");
        session.state = SessionState::Connected;
        let mut runner = ClientRunner::new(net, session);

        let cmd = UserCmd {
            msec: 10,
            angles: Vec3::new(0.0, 90.0, 180.0),
            forwardmove: 100,
            sidemove: 10,
            upmove: 0,
            buttons: 1,
            impulse: 0,
        };
        runner.send_move(cmd).unwrap();

        let mut server_buf = [0u8; 512];
        let (size, _) = server.recv_from(&mut server_buf).unwrap();
        let _ = &server_buf[..size];

        runner.state.valid_sequence = 1;
        let cmd = UserCmd {
            msec: 11,
            angles: Vec3::new(0.0, 90.0, 180.0),
            forwardmove: 200,
            sidemove: 10,
            upmove: 0,
            buttons: 2,
            impulse: 0,
        };
        runner.send_move(cmd).unwrap();

        let (size, _) = server.recv_from(&mut server_buf).unwrap();
        let packet = &server_buf[..size];

        let mut server_chan = Netchan::new(27001);
        let payload = server_chan.process_packet(packet, true).unwrap();

        let mut reader = MsgReader::new(payload);
        assert_eq!(reader.read_u8().unwrap(), Clc::Move as u8);
        let _checksum = reader.read_u8().unwrap();
        let _lost = reader.read_u8().unwrap();
        let base = UserCmd::default();
        let cmd0 = reader.read_delta_usercmd(&base).unwrap();
        let cmd1 = reader.read_delta_usercmd(&cmd0).unwrap();
        let cmd2 = reader.read_delta_usercmd(&cmd1).unwrap();
        assert_eq!(cmd2.buttons, 2);
        assert_eq!(cmd2.forwardmove, 200);

        assert_eq!(reader.read_u8().unwrap(), Clc::Delta as u8);
        assert_eq!(reader.read_u8().unwrap(), 1);
    }

    #[test]
    fn sends_string_cmd() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();

        let net = NetClient::connect(server_addr).unwrap();
        let mut session = Session::new(27001, "\\name\\player");
        session.state = SessionState::Connected;
        let mut runner = ClientRunner::new(net, session);

        runner.send_string_cmd("prespawn").unwrap();

        let mut server_buf = [0u8; 256];
        let (size, _) = server.recv_from(&mut server_buf).unwrap();
        let packet = &server_buf[..size];

        let mut server_chan = Netchan::new(27001);
        let payload = server_chan.process_packet(packet, true).unwrap();
        let mut reader = MsgReader::new(payload);
        assert_eq!(reader.read_u8().unwrap(), Clc::StringCmd as u8);
        assert_eq!(reader.read_string().unwrap(), "prespawn");
    }
}
