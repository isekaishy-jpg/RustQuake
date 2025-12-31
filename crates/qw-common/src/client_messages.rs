// Parsing helpers for client-facing network messages.

use crate::msg::{MsgReadError, MsgReader, SizeBuf, SizeBufError};
use crate::protocol::PROTOCOL_VERSION;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MoveVars {
    pub gravity: f32,
    pub stopspeed: f32,
    pub maxspeed: f32,
    pub spectatormaxspeed: f32,
    pub accelerate: f32,
    pub airaccelerate: f32,
    pub wateraccelerate: f32,
    pub friction: f32,
    pub waterfriction: f32,
    pub entgravity: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServerData {
    pub protocol: i32,
    pub server_count: i32,
    pub game_dir: String,
    pub player_num: u8,
    pub spectator: bool,
    pub level_name: String,
    pub movevars: MoveVars,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StringListChunk {
    pub start: u8,
    pub items: Vec<String>,
    pub next: u8,
}

#[derive(Debug)]
pub enum ServerDataError {
    Read(MsgReadError),
    UnsupportedProtocol(i32),
}

impl From<MsgReadError> for ServerDataError {
    fn from(err: MsgReadError) -> Self {
        ServerDataError::Read(err)
    }
}

pub fn parse_serverdata(reader: &mut MsgReader) -> Result<ServerData, ServerDataError> {
    let protocol = reader.read_i32()?;
    if protocol != PROTOCOL_VERSION {
        return Err(ServerDataError::UnsupportedProtocol(protocol));
    }

    let server_count = reader.read_i32()?;
    let game_dir = reader.read_string()?;
    let mut player_num = reader.read_u8()?;
    let spectator = (player_num & 0x80) != 0;
    if spectator {
        player_num &= 0x7F;
    }
    let level_name = reader.read_string()?;

    let movevars = MoveVars {
        gravity: reader.read_f32()?,
        stopspeed: reader.read_f32()?,
        maxspeed: reader.read_f32()?,
        spectatormaxspeed: reader.read_f32()?,
        accelerate: reader.read_f32()?,
        airaccelerate: reader.read_f32()?,
        wateraccelerate: reader.read_f32()?,
        friction: reader.read_f32()?,
        waterfriction: reader.read_f32()?,
        entgravity: reader.read_f32()?,
    };

    Ok(ServerData {
        protocol,
        server_count,
        game_dir,
        player_num,
        spectator,
        level_name,
        movevars,
    })
}

pub fn write_serverdata(buf: &mut SizeBuf, data: &ServerData) -> Result<(), SizeBufError> {
    buf.write_i32(data.protocol)?;
    buf.write_i32(data.server_count)?;
    buf.write_string(Some(&data.game_dir))?;

    let mut player = data.player_num & 0x7F;
    if data.spectator {
        player |= 0x80;
    }
    buf.write_u8(player)?;
    buf.write_string(Some(&data.level_name))?;

    buf.write_f32(data.movevars.gravity)?;
    buf.write_f32(data.movevars.stopspeed)?;
    buf.write_f32(data.movevars.maxspeed)?;
    buf.write_f32(data.movevars.spectatormaxspeed)?;
    buf.write_f32(data.movevars.accelerate)?;
    buf.write_f32(data.movevars.airaccelerate)?;
    buf.write_f32(data.movevars.wateraccelerate)?;
    buf.write_f32(data.movevars.friction)?;
    buf.write_f32(data.movevars.waterfriction)?;
    buf.write_f32(data.movevars.entgravity)?;
    Ok(())
}

pub fn parse_string_list_chunk(reader: &mut MsgReader) -> Result<StringListChunk, MsgReadError> {
    let start = reader.read_u8()?;
    let mut items = Vec::new();
    loop {
        let item = reader.read_string()?;
        if item.is_empty() {
            break;
        }
        items.push(item);
    }
    let next = reader.read_u8()?;
    Ok(StringListChunk { start, items, next })
}

pub fn write_string_list_chunk(
    buf: &mut SizeBuf,
    chunk: &StringListChunk,
) -> Result<(), SizeBufError> {
    buf.write_u8(chunk.start)?;
    for item in &chunk.items {
        buf.write_string(Some(item))?;
    }
    buf.write_string(Some(""))?;
    buf.write_u8(chunk.next)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::MsgReader;

    #[test]
    fn serverdata_round_trip() {
        let data = ServerData {
            protocol: PROTOCOL_VERSION,
            server_count: 42,
            game_dir: "id1".to_string(),
            player_num: 3,
            spectator: true,
            level_name: "start".to_string(),
            movevars: MoveVars {
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
        write_serverdata(&mut buf, &data).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let parsed = parse_serverdata(&mut reader).unwrap();

        assert_eq!(parsed, data);
    }

    #[test]
    fn string_list_chunk_round_trip() {
        let chunk = StringListChunk {
            start: 2,
            items: vec!["sound1".to_string(), "sound2".to_string()],
            next: 5,
        };

        let mut buf = SizeBuf::new(256);
        write_string_list_chunk(&mut buf, &chunk).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let parsed = parse_string_list_chunk(&mut reader).unwrap();

        assert_eq!(parsed, chunk);
    }
}
