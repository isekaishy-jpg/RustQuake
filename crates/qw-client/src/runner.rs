use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, Write};

use crate::client::{Client, ClientPacket};
use crate::net::NetClient;
use crate::session::Session;
use crate::state::ClientState;
use qw_common::{
    clc, find_game_dir, find_id1_dir, locate_data_dir, Bsp, BspError, DataPathError, FsError,
    NetchanError, OobMessage, QuakeFs, SizeBuf, SizeBufError, SvcMessage, UPDATE_BACKUP, UserCmd,
};
use std::path::PathBuf;

#[derive(Debug)]
#[allow(dead_code)]
pub enum RunnerError {
    Io(io::Error),
    Client(crate::client::ClientError),
    Netchan(NetchanError),
    Buffer(SizeBufError),
    Fs(FsError),
    Bsp(BspError),
    DataPath(DataPathError),
    MissingGameDir(String),
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

impl From<FsError> for RunnerError {
    fn from(err: FsError) -> Self {
        RunnerError::Fs(err)
    }
}

impl From<BspError> for RunnerError {
    fn from(err: BspError) -> Self {
        RunnerError::Bsp(err)
    }
}

impl From<DataPathError> for RunnerError {
    fn from(err: DataPathError) -> Self {
        RunnerError::DataPath(err)
    }
}

pub struct ClientRunner {
    pub net: NetClient,
    pub session: Session,
    pub client: Client,
    pub state: ClientState,
    fs_game_dir: Option<String>,
    data_dir: Option<PathBuf>,
    download_queue: VecDeque<String>,
    download: Option<DownloadState>,
    signon_phase: SignonPhase,
}

impl ClientRunner {
    pub fn new(net: NetClient, session: Session) -> Self {
        let qport = session.qport;
        Self {
            net,
            session,
            client: Client::new(qport),
            state: ClientState::new(),
            fs_game_dir: None,
            data_dir: None,
            download_queue: VecDeque::new(),
            download: None,
            signon_phase: SignonPhase::Idle,
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
                    self.send_string_cmd("new")?;
                }
            }
            ClientPacket::Messages(_) => {}
        }

        if let ClientPacket::Messages(messages) = &parsed {
            self.state.clear_frame_events();
            let incoming_sequence = self.client.netchan.incoming_sequence();
            let incoming_ack = self.client.netchan.incoming_acknowledged();
            for message in messages {
                if let SvcMessage::ChokeCount(count) = message {
                    self.state.mark_choked(*count, incoming_ack);
                }
                self.state.apply_message(message, incoming_sequence);
                self.handle_signon(message)?;
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
        self.client.netchan.queue_reliable(buf.as_slice())?;
        let packet = self.client.netchan.build_packet(&[], true)?;
        self.net.send(&packet)?;
        Ok(())
    }

    fn handle_signon(&mut self, message: &SvcMessage) -> Result<(), RunnerError> {
        match message {
            SvcMessage::ServerData(data) => {
                let cmd = format!("soundlist {} 0", data.server_count);
                self.signon_phase = SignonPhase::SoundList;
                self.send_string_cmd(&cmd)?;
            }
            SvcMessage::SoundList(chunk) => {
                let Some(data) = self.state.serverdata.clone() else {
                    return Ok(());
                };
                if chunk.next != 0 {
                    let cmd = format!("soundlist {} {}", data.server_count, chunk.next);
                    self.send_string_cmd(&cmd)?;
                } else {
                    self.ensure_filesystem(&data)?;
                    self.download_queue.clear();
                    self.queue_missing_sounds()?;
                    if self.start_next_download()? {
                        self.signon_phase = SignonPhase::ModelList;
                    } else {
                        let cmd = format!("modellist {} 0", data.server_count);
                        self.signon_phase = SignonPhase::ModelList;
                        self.send_string_cmd(&cmd)?;
                    }
                }
            }
            SvcMessage::ModelList(chunk) => {
                let Some(data) = self.state.serverdata.clone() else {
                    return Ok(());
                };
                if chunk.next != 0 {
                    let cmd = format!("modellist {} {}", data.server_count, chunk.next);
                    self.send_string_cmd(&cmd)?;
                } else {
                    self.ensure_filesystem(&data)?;
                    self.download_queue.clear();
                    self.queue_missing_models()?;
                    if self.start_next_download()? {
                        self.signon_phase = SignonPhase::Skins;
                    } else {
                        self.download_queue.clear();
                        self.queue_missing_skins()?;
                        if self.start_next_download()? {
                            self.signon_phase = SignonPhase::Prespawn;
                        } else {
                            let checksum2 = self.map_checksum2(&data)?;
                            let cmd = format!("prespawn {} 0 {}", data.server_count, checksum2);
                            self.signon_phase = SignonPhase::Done;
                            self.send_string_cmd(&cmd)?;
                        }
                    }
                }
            }
            SvcMessage::StuffText(text) => {
                self.handle_stufftext(text)?;
            }
            SvcMessage::Download { size, percent, data } => {
                self.handle_download(*size, *percent, data)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn map_checksum2(&mut self, data: &qw_common::ServerData) -> Result<u32, RunnerError> {
        self.ensure_filesystem(data)?;
        let map_name = map_path(&data.level_name);
        let bytes = self.client.fs.read(&map_name)?;
        let bsp = Bsp::from_bytes(bytes)?;
        let (_, checksum2) = bsp.map_checksums()?;
        Ok(checksum2)
    }

    fn handle_stufftext(&mut self, text: &str) -> Result<(), RunnerError> {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("cmd") {
                let command = rest.trim_start();
                if !command.is_empty() {
                    self.send_string_cmd(command)?;
                }
            }
        }
        Ok(())
    }

    fn ensure_filesystem(&mut self, data: &qw_common::ServerData) -> Result<(), RunnerError> {
        if !self.client.fs.is_empty() {
            if let Some(current) = self.fs_game_dir.as_deref() {
                if current == data.game_dir {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        }

        let base_dir = match &self.data_dir {
            Some(path) => path.clone(),
            None => {
                let path = locate_data_dir()?;
                self.data_dir = Some(path.clone());
                path
            }
        };

        let id1 = find_id1_dir(&base_dir)
            .ok_or_else(|| RunnerError::MissingGameDir("id1".to_string()))?;

        let mut fs = QuakeFs::new();
        fs.add_game_dir(&id1)?;

        if data.game_dir != "id1" {
            let mod_dir = find_game_dir(&base_dir, &data.game_dir)
                .ok_or_else(|| RunnerError::MissingGameDir(data.game_dir.clone()))?;
            fs.add_game_dir(&mod_dir)?;
        }

        let download_dir = self.download_root()?.join(&data.game_dir);
        fs::create_dir_all(&download_dir)?;
        fs.add_game_dir(&download_dir)?;

        self.client.fs = fs;
        self.fs_game_dir = Some(data.game_dir.clone());
        Ok(())
    }

    fn queue_missing_sounds(&mut self) -> Result<(), RunnerError> {
        for name in &self.state.sounds {
            if name.is_empty() {
                continue;
            }
            let path = format!("sound/{}", name);
            if !self.client.fs.contains(&path) {
                self.download_queue.push_back(path);
            }
        }
        Ok(())
    }

    fn queue_missing_models(&mut self) -> Result<(), RunnerError> {
        for name in &self.state.models {
            if name.is_empty() || name.starts_with('*') {
                continue;
            }
            if !self.client.fs.contains(name) {
                self.download_queue.push_back(name.clone());
            }
        }
        Ok(())
    }

    fn queue_missing_skins(&mut self) -> Result<(), RunnerError> {
        for player in &self.state.players {
            let Some(skin) = qw_common::value_for_key(player.userinfo.as_str(), "skin") else {
                continue;
            };
            let trimmed = skin.trim();
            if trimmed.is_empty() {
                continue;
            }
            let base = qw_common::strip_extension(trimmed);
            if base.is_empty() {
                continue;
            }
            let path = format!("skins/{}.pcx", base);
            if !self.client.fs.contains(&path) {
                self.download_queue.push_back(path);
            }
        }
        Ok(())
    }

    fn start_next_download(&mut self) -> Result<bool, RunnerError> {
        if self.download.is_some() {
            return Ok(true);
        }

        let mut name = None;
        while let Some(candidate) = self.download_queue.pop_front() {
            if is_safe_download_path(&candidate) {
                name = Some(candidate);
                break;
            }
        }

        let Some(name) = name else {
            return Ok(false);
        };
        let Some(data) = &self.state.serverdata else {
            return Ok(false);
        };

        let download_root = self.download_root()?;
        let final_path = download_root.join(&data.game_dir).join(&name);
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let filename = final_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "download.tmp".to_string());
        let temp_path = final_path.with_file_name(format!("{}.tmp", filename));

        let file = File::create(&temp_path)?;
        self.download = Some(DownloadState {
            temp_path,
            final_path,
            file,
        });

        let cmd = format!("download {}", name);
        self.send_string_cmd(&cmd)?;
        Ok(true)
    }

    fn handle_download(
        &mut self,
        size: i16,
        percent: u8,
        data: &[u8],
    ) -> Result<(), RunnerError> {
        let Some(state) = &mut self.download else {
            return Ok(());
        };

        if size == -1 {
            let temp_path = state.temp_path.clone();
            self.download = None;
            let _ = fs::remove_file(temp_path);
            if self.start_next_download()? {
                return Ok(());
            }
            self.resume_signon()?;
            return Ok(());
        }

        if size > 0 {
            state.file.write_all(data)?;
        }

        if percent != 100 {
            self.send_string_cmd("nextdl")?;
            return Ok(());
        }

        state.file.flush()?;
        let temp_path = state.temp_path.clone();
        let final_path = state.final_path.clone();
        self.download = None;
        let _ = fs::rename(&temp_path, &final_path);

        if self.start_next_download()? {
            return Ok(());
        }
        self.resume_signon()?;
        Ok(())
    }

    fn resume_signon(&mut self) -> Result<(), RunnerError> {
        let Some(data) = self.state.serverdata.clone() else {
            return Ok(());
        };
        match self.signon_phase {
            SignonPhase::ModelList => {
                let cmd = format!("modellist {} 0", data.server_count);
                self.send_string_cmd(&cmd)?;
            }
            SignonPhase::Skins => {
                self.download_queue.clear();
                self.queue_missing_skins()?;
                if self.start_next_download()? {
                    self.signon_phase = SignonPhase::Prespawn;
                    return Ok(());
                }
                let checksum2 = self.map_checksum2(&data)?;
                let cmd = format!("prespawn {} 0 {}", data.server_count, checksum2);
                self.signon_phase = SignonPhase::Done;
                self.send_string_cmd(&cmd)?;
            }
            SignonPhase::Prespawn => {
                let checksum2 = self.map_checksum2(&data)?;
                let cmd = format!("prespawn {} 0 {}", data.server_count, checksum2);
                self.signon_phase = SignonPhase::Done;
                self.send_string_cmd(&cmd)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn download_root(&self) -> Result<PathBuf, RunnerError> {
        if let Ok(value) = std::env::var("RUSTQUAKE_DOWNLOAD_DIR") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(PathBuf::from(trimmed));
            }
        }
        let mut path = std::env::current_dir()?;
        path.push("data");
        path.push("downloads");
        Ok(path)
    }
}

fn is_safe_download_path(name: &str) -> bool {
    if name.is_empty() || name.contains(':') || name.contains('\0') {
        return false;
    }
    let path = std::path::Path::new(name);
    if path.is_absolute() {
        return false;
    }
    for component in path.components() {
        match component {
            std::path::Component::ParentDir
            | std::path::Component::Prefix(_)
            | std::path::Component::RootDir => {
                return false;
            }
            _ => {}
        }
    }
    true
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum SignonPhase {
    Idle,
    SoundList,
    ModelList,
    Skins,
    Prespawn,
    Done,
}

struct DownloadState {
    temp_path: PathBuf,
    final_path: PathBuf,
    file: File,
}

fn map_path(level_name: &str) -> String {
    let name = level_name.trim();
    if name.contains('/') || name.contains('\\') {
        if name.ends_with(".bsp") {
            name.to_string()
        } else {
            format!("{}.bsp", name)
        }
    } else if name.ends_with(".bsp") {
        format!("maps/{}", name)
    } else {
        format!("maps/{}.bsp", name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionState;
    use qw_common::{
        build_out_of_band, out_of_band_payload, Clc, MsgReader, Netchan, S2C_CHALLENGE,
        S2C_CONNECTION, ServerData, SizeBuf, SvcMessage, UserCmd, Vec3,
    };
    use std::net::UdpSocket;
    use std::sync::Mutex;
    use std::time::Duration;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

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

        let mut reader = MsgReader::new(&payload);
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
        let mut reader = MsgReader::new(&payload);
        assert_eq!(reader.read_u8().unwrap(), Clc::StringCmd as u8);
        assert_eq!(reader.read_string().unwrap(), "prespawn");
    }

    #[test]
    fn requests_soundlist_after_serverdata() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();

        let net = NetClient::connect(server_addr).unwrap();
        let mut session = Session::new(27001, "\\name\\player");
        session.state = SessionState::Connected;
        let mut runner = ClientRunner::new(net, session);

        let data = ServerData {
            protocol: qw_common::PROTOCOL_VERSION,
            server_count: 42,
            game_dir: "id1".to_string(),
            player_num: 1,
            spectator: false,
            level_name: "start".to_string(),
            movevars: qw_common::MoveVars {
                gravity: 800.0,
                stopspeed: 100.0,
                maxspeed: 320.0,
                spectatormaxspeed: 500.0,
                accelerate: 10.0,
                airaccelerate: 12.0,
                wateraccelerate: 8.0,
                friction: 4.0,
                waterfriction: 2.0,
                entgravity: 1.0,
            },
        };

        let mut buf = SizeBuf::new(256);
        qw_common::write_svc_message(&mut buf, &SvcMessage::ServerData(data.clone())).unwrap();
        let mut server_chan = Netchan::new(27001);
        let packet = server_chan.build_packet(buf.as_slice(), false).unwrap();

        let client_port = runner.net.local_addr().unwrap().port();
        let client_addr = std::net::SocketAddr::from(([127, 0, 0, 1], client_port));
        server.send_to(&packet, client_addr).unwrap();

        let mut client_buf = [0u8; 512];
        for _ in 0..10 {
            if runner.poll_once(&mut client_buf).unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        let mut recv_buf = [0u8; 512];
        let (size, _) = server.recv_from(&mut recv_buf).unwrap();
        let mut recv_chan = Netchan::new(27001);
        let payload = recv_chan.process_packet(&recv_buf[..size], true).unwrap();

        let mut reader = MsgReader::new(&payload);
        assert_eq!(reader.read_u8().unwrap(), Clc::StringCmd as u8);
        assert_eq!(
            reader.read_string().unwrap(),
            format!("soundlist {} 0", data.server_count)
        );
    }

    #[test]
    fn builds_prespawn_with_map_checksum() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();

        let net = NetClient::connect(server_addr).unwrap();
        let mut session = Session::new(27001, "\\name\\player");
        session.state = SessionState::Connected;
        let mut runner = ClientRunner::new(net, session);

        let dir = temp_dir();
        let map_bytes = build_test_bsp();
        let maps_dir = dir.join("maps");
        std::fs::create_dir_all(&maps_dir).unwrap();
        std::fs::write(maps_dir.join("test.bsp"), &map_bytes).unwrap();
        runner.client.fs.add_game_dir(&dir).unwrap();

        let bsp = qw_common::Bsp::from_bytes(map_bytes).unwrap();
        let (_, checksum2) = bsp.map_checksums().unwrap();

        let data = ServerData {
            protocol: qw_common::PROTOCOL_VERSION,
            server_count: 7,
            game_dir: "id1".to_string(),
            player_num: 1,
            spectator: false,
            level_name: "test".to_string(),
            movevars: qw_common::MoveVars {
                gravity: 800.0,
                stopspeed: 100.0,
                maxspeed: 320.0,
                spectatormaxspeed: 500.0,
                accelerate: 10.0,
                airaccelerate: 12.0,
                wateraccelerate: 8.0,
                friction: 4.0,
                waterfriction: 2.0,
                entgravity: 1.0,
            },
        };
        runner.state.serverdata = Some(data.clone());

        let mut server_chan = Netchan::new(27001);
        let mut model_buf = SizeBuf::new(256);
        qw_common::write_svc_message(
            &mut model_buf,
            &SvcMessage::ModelList(qw_common::StringListChunk {
                start: 0,
                items: Vec::new(),
                next: 0,
            }),
        )
        .unwrap();
        let model_packet = server_chan
            .build_packet(model_buf.as_slice(), false)
            .unwrap();
        let client_port = runner.net.local_addr().unwrap().port();
        let client_addr = std::net::SocketAddr::from(([127, 0, 0, 1], client_port));
        server.send_to(&model_packet, client_addr).unwrap();

        let mut client_buf = [0u8; 512];
        for _ in 0..10 {
            if runner.poll_once(&mut client_buf).unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        let mut recv_chan = Netchan::new(27001);
        let payload = recv_payload(&server, &mut recv_chan);
        let mut reader = MsgReader::new(&payload);
        assert_eq!(reader.read_u8().unwrap(), Clc::StringCmd as u8);
        assert_eq!(
            reader.read_string().unwrap(),
            format!("prespawn {} 0 {}", data.server_count, checksum2)
        );

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn forwards_cmd_lines_from_stufftext() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();

        let net = NetClient::connect(server_addr).unwrap();
        let mut session = Session::new(27001, "\\name\\player");
        session.state = SessionState::Connected;
        let mut runner = ClientRunner::new(net, session);

        let mut buf = SizeBuf::new(128);
        qw_common::write_svc_message(
            &mut buf,
            &SvcMessage::StuffText("cmd spawn 1 0\n".to_string()),
        )
        .unwrap();
        let mut server_chan = Netchan::new(27001);
        let packet = server_chan.build_packet(buf.as_slice(), false).unwrap();

        let client_port = runner.net.local_addr().unwrap().port();
        let client_addr = std::net::SocketAddr::from(([127, 0, 0, 1], client_port));
        server.send_to(&packet, client_addr).unwrap();

        let mut client_buf = [0u8; 256];
        for _ in 0..10 {
            if runner.poll_once(&mut client_buf).unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        let mut recv_chan = Netchan::new(27001);
        let payload = recv_payload(&server, &mut recv_chan);
        let mut reader = MsgReader::new(&payload);
        assert_eq!(reader.read_u8().unwrap(), Clc::StringCmd as u8);
        assert_eq!(reader.read_string().unwrap(), "spawn 1 0");
    }

    fn recv_payload(server: &UdpSocket, chan: &mut Netchan) -> Vec<u8> {
        let mut buf = [0u8; 512];
        for _ in 0..5 {
            match server.recv_from(&mut buf) {
                Ok((size, _)) => {
                    let payload = chan.process_packet(&buf[..size], true).unwrap();
                    if !payload.is_empty() {
                        return payload.to_vec();
                    }
                }
                Err(err)
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut =>
                {
                    continue;
                }
                Err(err) => panic!("recv error: {:?}", err),
            }
        }
        panic!("no payload received");
    }

    fn temp_dir() -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut path = std::env::temp_dir();
        path.push(format!("rustquake-test-{}-{}", std::process::id(), nanos));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn build_test_bsp() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&qw_common::BSP_VERSION.to_le_bytes());

        let header_size = 4 + qw_common::HEADER_LUMPS * 8;
        let mut offsets = Vec::with_capacity(qw_common::HEADER_LUMPS);
        let mut payloads = Vec::with_capacity(qw_common::HEADER_LUMPS);
        let mut cursor = header_size;

        for i in 0..qw_common::HEADER_LUMPS {
            let payload = vec![i as u8; 4];
            offsets.push((cursor as u32, payload.len() as u32));
            cursor += payload.len();
            payloads.push(payload);
        }

        for (offset, length) in &offsets {
            data.extend_from_slice(&offset.to_le_bytes());
            data.extend_from_slice(&length.to_le_bytes());
        }

        for payload in payloads {
            data.extend_from_slice(&payload);
        }

        data
    }

    #[test]
    fn requests_download_for_missing_sound() {
        let _guard = ENV_LOCK.lock().unwrap();
        let download_dir = temp_dir();
        let old_download = std::env::var("RUSTQUAKE_DOWNLOAD_DIR").ok();
        // Safety: env var mutation is process-global; guard with ENV_LOCK.
        unsafe {
            std::env::set_var("RUSTQUAKE_DOWNLOAD_DIR", &download_dir);
        }

        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();

        let net = NetClient::connect(server_addr).unwrap();
        let mut session = Session::new(27001, "\\name\\player");
        session.state = SessionState::Connected;
        let mut runner = ClientRunner::new(net, session);

        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        runner.client.fs.add_game_dir(&dir).unwrap();

        runner.state.serverdata = Some(ServerData {
            protocol: qw_common::PROTOCOL_VERSION,
            server_count: 9,
            game_dir: "id1".to_string(),
            player_num: 1,
            spectator: false,
            level_name: "start".to_string(),
            movevars: qw_common::MoveVars {
                gravity: 800.0,
                stopspeed: 100.0,
                maxspeed: 320.0,
                spectatormaxspeed: 500.0,
                accelerate: 10.0,
                airaccelerate: 12.0,
                wateraccelerate: 8.0,
                friction: 4.0,
                waterfriction: 2.0,
                entgravity: 1.0,
            },
        });

        let mut buf = SizeBuf::new(256);
        qw_common::write_svc_message(
            &mut buf,
            &SvcMessage::SoundList(qw_common::StringListChunk {
                start: 0,
                items: vec!["misc/foo.wav".to_string()],
                next: 0,
            }),
        )
        .unwrap();
        let mut server_chan = Netchan::new(27001);
        let packet = server_chan.build_packet(buf.as_slice(), false).unwrap();

        let client_port = runner.net.local_addr().unwrap().port();
        let client_addr = std::net::SocketAddr::from(([127, 0, 0, 1], client_port));
        server.send_to(&packet, client_addr).unwrap();

        let mut client_buf = [0u8; 512];
        for _ in 0..10 {
            if runner.poll_once(&mut client_buf).unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        let mut recv_chan = Netchan::new(27001);
        let payload = recv_payload(&server, &mut recv_chan);
        let mut reader = MsgReader::new(&payload);
        assert_eq!(reader.read_u8().unwrap(), Clc::StringCmd as u8);
        assert_eq!(reader.read_string().unwrap(), "download sound/misc/foo.wav");

        if let Some(value) = old_download {
            unsafe {
                std::env::set_var("RUSTQUAKE_DOWNLOAD_DIR", value);
            }
        } else {
            unsafe {
                std::env::remove_var("RUSTQUAKE_DOWNLOAD_DIR");
            }
        }
        std::fs::remove_dir_all(download_dir).ok();
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn requests_download_for_missing_skin() {
        let _guard = ENV_LOCK.lock().unwrap();
        let download_dir = temp_dir();
        let old_download = std::env::var("RUSTQUAKE_DOWNLOAD_DIR").ok();
        // Safety: env var mutation is process-global; guard with ENV_LOCK.
        unsafe {
            std::env::set_var("RUSTQUAKE_DOWNLOAD_DIR", &download_dir);
        }

        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();

        let net = NetClient::connect(server_addr).unwrap();
        let mut session = Session::new(27001, "\\name\\player");
        session.state = SessionState::Connected;
        let mut runner = ClientRunner::new(net, session);

        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        runner.client.fs.add_game_dir(&dir).unwrap();

        runner.state.serverdata = Some(ServerData {
            protocol: qw_common::PROTOCOL_VERSION,
            server_count: 9,
            game_dir: "id1".to_string(),
            player_num: 1,
            spectator: false,
            level_name: "start".to_string(),
            movevars: qw_common::MoveVars {
                gravity: 800.0,
                stopspeed: 100.0,
                maxspeed: 320.0,
                spectatormaxspeed: 500.0,
                accelerate: 10.0,
                airaccelerate: 12.0,
                wateraccelerate: 8.0,
                friction: 4.0,
                waterfriction: 2.0,
                entgravity: 1.0,
            },
        });
        runner
            .state
            .players
            .get_mut(0)
            .unwrap()
            .userinfo
            .set("skin", "base")
            .unwrap();

        let mut buf = SizeBuf::new(256);
        qw_common::write_svc_message(
            &mut buf,
            &SvcMessage::ModelList(qw_common::StringListChunk {
                start: 0,
                items: Vec::new(),
                next: 0,
            }),
        )
        .unwrap();
        let mut server_chan = Netchan::new(27001);
        let packet = server_chan.build_packet(buf.as_slice(), false).unwrap();

        let client_port = runner.net.local_addr().unwrap().port();
        let client_addr = std::net::SocketAddr::from(([127, 0, 0, 1], client_port));
        server.send_to(&packet, client_addr).unwrap();

        let mut client_buf = [0u8; 512];
        for _ in 0..10 {
            if runner.poll_once(&mut client_buf).unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        let mut recv_chan = Netchan::new(27001);
        let payload = recv_payload(&server, &mut recv_chan);
        let mut reader = MsgReader::new(&payload);
        assert_eq!(reader.read_u8().unwrap(), Clc::StringCmd as u8);
        assert_eq!(reader.read_string().unwrap(), "download skins/base.pcx");

        if let Some(value) = old_download {
            unsafe {
                std::env::set_var("RUSTQUAKE_DOWNLOAD_DIR", value);
            }
        } else {
            unsafe {
                std::env::remove_var("RUSTQUAKE_DOWNLOAD_DIR");
            }
        }
        std::fs::remove_dir_all(download_dir).ok();
        std::fs::remove_dir_all(dir).ok();
    }
}
