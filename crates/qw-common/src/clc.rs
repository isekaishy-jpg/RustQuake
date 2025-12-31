// Client-to-server message helpers.

use crate::com_checksum::block_sequence_crc_byte;
use crate::msg::{SizeBuf, SizeBufError};
use crate::protocol::Clc;
use crate::types::UserCmd;

#[derive(Debug, Clone)]
pub struct MoveMessage {
    pub sequence: u32,
    pub lost: u8,
    pub cmds: [UserCmd; 3],
    pub delta_sequence: Option<u8>,
}

pub fn write_string_cmd(buf: &mut SizeBuf, text: &str) -> Result<(), SizeBufError> {
    buf.write_u8(Clc::StringCmd as u8)?;
    buf.write_string(Some(text))
}

pub fn write_nop(buf: &mut SizeBuf) -> Result<(), SizeBufError> {
    buf.write_u8(Clc::Nop as u8)
}

pub fn write_move_message(buf: &mut SizeBuf, message: &MoveMessage) -> Result<(), SizeBufError> {
    buf.write_u8(Clc::Move as u8)?;
    let checksum_index = buf.len();
    buf.write_u8(0)?;
    buf.write_u8(message.lost)?;

    let nullcmd = UserCmd::default();
    buf.write_delta_usercmd(&nullcmd, &message.cmds[0])?;
    buf.write_delta_usercmd(&message.cmds[0], &message.cmds[1])?;
    buf.write_delta_usercmd(&message.cmds[1], &message.cmds[2])?;

    let checksum = block_sequence_crc_byte(
        &buf.as_slice()[checksum_index + 1..],
        message.sequence as i32,
    );
    buf.set_u8(checksum_index, checksum)?;

    if let Some(delta) = message.delta_sequence {
        buf.write_u8(Clc::Delta as u8)?;
        buf.write_u8(delta)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::{MsgReadError, MsgReader};
    use crate::types::Vec3;

    fn read_move_message(
        reader: &mut MsgReader,
    ) -> Result<(u8, u8, [UserCmd; 3], Option<u8>), MsgReadError> {
        let cmd = reader.read_u8()?;
        assert_eq!(cmd, Clc::Move as u8);
        let checksum = reader.read_u8()?;
        let lost = reader.read_u8()?;
        let base = UserCmd::default();
        let cmd0 = reader.read_delta_usercmd(&base)?;
        let cmd1 = reader.read_delta_usercmd(&cmd0)?;
        let cmd2 = reader.read_delta_usercmd(&cmd1)?;
        let delta = if reader.remaining() > 0 {
            let clc = reader.read_u8()?;
            assert_eq!(clc, Clc::Delta as u8);
            Some(reader.read_u8()?)
        } else {
            None
        };
        Ok((checksum, lost, [cmd0, cmd1, cmd2], delta))
    }

    fn quantize_angle16(value: f32) -> f32 {
        let scaled = (value * 65536.0 / 360.0) as i32;
        let stored = scaled as i16;
        stored as f32 * (360.0 / 65536.0)
    }

    #[test]
    fn writes_move_message_and_checksum() {
        let cmd0 = UserCmd {
            msec: 10,
            angles: Vec3::new(0.0, 90.0, 180.0),
            forwardmove: 100,
            sidemove: 10,
            upmove: 0,
            buttons: 1,
            impulse: 0,
        };
        let cmd1 = UserCmd {
            msec: 11,
            angles: Vec3::new(0.0, 90.0, 270.0),
            forwardmove: 200,
            sidemove: 20,
            upmove: 0,
            buttons: 2,
            impulse: 1,
        };
        let cmd2 = UserCmd {
            msec: 12,
            angles: Vec3::new(10.0, 90.0, 270.0),
            forwardmove: 300,
            sidemove: 30,
            upmove: 5,
            buttons: 3,
            impulse: 2,
        };

        let message = MoveMessage {
            sequence: 99,
            lost: 2,
            cmds: [cmd0, cmd1, cmd2],
            delta_sequence: Some(7),
        };

        let mut buf = SizeBuf::new(256);
        write_move_message(&mut buf, &message).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let (checksum, lost, parsed_cmds, delta) = read_move_message(&mut reader).unwrap();
        assert_eq!(lost, message.lost);
        let mut expected_cmds = message.cmds;
        for cmd in &mut expected_cmds {
            cmd.angles.x = quantize_angle16(cmd.angles.x);
            cmd.angles.y = quantize_angle16(cmd.angles.y);
            cmd.angles.z = quantize_angle16(cmd.angles.z);
        }
        assert_eq!(parsed_cmds, expected_cmds);
        assert_eq!(delta, Some(7));

        let mut base = message.clone();
        base.delta_sequence = None;
        let mut base_buf = SizeBuf::new(256);
        write_move_message(&mut base_buf, &base).unwrap();
        let expected = base_buf.as_slice()[1];
        assert_eq!(checksum, expected);
    }

    #[test]
    fn writes_move_message_without_delta() {
        let message = MoveMessage {
            sequence: 1,
            lost: 0,
            cmds: [UserCmd::default(), UserCmd::default(), UserCmd::default()],
            delta_sequence: None,
        };

        let mut buf = SizeBuf::new(128);
        write_move_message(&mut buf, &message).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let (_, _, _, delta) = read_move_message(&mut reader).unwrap();
        assert_eq!(delta, None);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn writes_string_cmd() {
        let mut buf = SizeBuf::new(64);
        write_string_cmd(&mut buf, "prespawn").unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        assert_eq!(reader.read_u8().unwrap(), Clc::StringCmd as u8);
        assert_eq!(reader.read_string().unwrap(), "prespawn");
        assert_eq!(reader.remaining(), 0);
    }
}
