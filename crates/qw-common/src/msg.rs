// Message buffer read/write helpers for QuakeWorld network payloads.

use std::fmt;

use crate::protocol::{
    CM_ANGLE1, CM_ANGLE2, CM_ANGLE3, CM_BUTTONS, CM_FORWARD, CM_IMPULSE, CM_SIDE, CM_UP,
};
use crate::types::UserCmd;

#[derive(Debug)]
pub enum SizeBufError {
    Overflow,
    TooLarge,
}

impl fmt::Display for SizeBufError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SizeBufError::Overflow => write!(f, "size buffer overflow"),
            SizeBufError::TooLarge => write!(f, "write larger than buffer"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SizeBuf {
    data: Vec<u8>,
    maxsize: usize,
    allow_overflow: bool,
    overflowed: bool,
}

impl SizeBuf {
    pub fn new(maxsize: usize) -> Self {
        Self {
            data: Vec::with_capacity(maxsize),
            maxsize,
            allow_overflow: false,
            overflowed: false,
        }
    }

    pub fn with_overflow(mut self, allow: bool) -> Self {
        self.allow_overflow = allow;
        self
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.overflowed = false;
    }

    pub fn is_overflowed(&self) -> bool {
        self.overflowed
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn maxsize(&self) -> usize {
        self.maxsize
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn set_u8(&mut self, index: usize, value: u8) -> Result<(), SizeBufError> {
        if index >= self.data.len() {
            return Err(SizeBufError::Overflow);
        }
        self.data[index] = value;
        Ok(())
    }

    fn get_space(&mut self, length: usize) -> Result<usize, SizeBufError> {
        if self.data.len() + length > self.maxsize {
            if !self.allow_overflow {
                return Err(SizeBufError::Overflow);
            }
            if length > self.maxsize {
                return Err(SizeBufError::TooLarge);
            }
            self.data.clear();
            self.overflowed = true;
        }

        let start = self.data.len();
        self.data.resize(self.data.len() + length, 0);
        Ok(start)
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), SizeBufError> {
        let start = self.get_space(bytes.len())?;
        self.data[start..start + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }

    pub fn write_i8(&mut self, value: i8) -> Result<(), SizeBufError> {
        self.write_bytes(&[value as u8])
    }

    pub fn write_u8(&mut self, value: u8) -> Result<(), SizeBufError> {
        self.write_bytes(&[value])
    }

    pub fn write_i16(&mut self, value: i16) -> Result<(), SizeBufError> {
        self.write_bytes(&value.to_le_bytes())
    }

    pub fn write_u16(&mut self, value: u16) -> Result<(), SizeBufError> {
        self.write_bytes(&value.to_le_bytes())
    }

    pub fn write_i32(&mut self, value: i32) -> Result<(), SizeBufError> {
        self.write_bytes(&value.to_le_bytes())
    }

    pub fn write_u32(&mut self, value: u32) -> Result<(), SizeBufError> {
        self.write_bytes(&value.to_le_bytes())
    }

    pub fn write_f32(&mut self, value: f32) -> Result<(), SizeBufError> {
        self.write_bytes(&value.to_le_bytes())
    }

    pub fn write_string(&mut self, value: Option<&str>) -> Result<(), SizeBufError> {
        match value {
            Some(text) => {
                self.write_bytes(text.as_bytes())?;
                self.write_u8(0)
            }
            None => self.write_u8(0),
        }
    }

    pub fn write_coord(&mut self, value: f32) -> Result<(), SizeBufError> {
        let scaled = (value * 8.0).round() as i32;
        self.write_i16(scaled as i16)
    }

    pub fn write_angle(&mut self, value: f32) -> Result<(), SizeBufError> {
        let scaled = ((value * 256.0 / 360.0) as i32) & 0xFF;
        self.write_u8(scaled as u8)
    }

    pub fn write_angle16(&mut self, value: f32) -> Result<(), SizeBufError> {
        let scaled = ((value * 65536.0 / 360.0) as i32) & 0xFFFF;
        self.write_u16(scaled as u16)
    }

    pub fn write_delta_usercmd(
        &mut self,
        from: &UserCmd,
        cmd: &UserCmd,
    ) -> Result<(), SizeBufError> {
        let mut bits: u8 = 0;
        if cmd.angles.x != from.angles.x {
            bits |= CM_ANGLE1;
        }
        if cmd.angles.y != from.angles.y {
            bits |= CM_ANGLE2;
        }
        if cmd.angles.z != from.angles.z {
            bits |= CM_ANGLE3;
        }
        if cmd.forwardmove != from.forwardmove {
            bits |= CM_FORWARD;
        }
        if cmd.sidemove != from.sidemove {
            bits |= CM_SIDE;
        }
        if cmd.upmove != from.upmove {
            bits |= CM_UP;
        }
        if cmd.buttons != from.buttons {
            bits |= CM_BUTTONS;
        }
        if cmd.impulse != from.impulse {
            bits |= CM_IMPULSE;
        }

        self.write_u8(bits)?;

        if bits & CM_ANGLE1 != 0 {
            self.write_angle16(cmd.angles.x)?;
        }
        if bits & CM_ANGLE2 != 0 {
            self.write_angle16(cmd.angles.y)?;
        }
        if bits & CM_ANGLE3 != 0 {
            self.write_angle16(cmd.angles.z)?;
        }
        if bits & CM_FORWARD != 0 {
            self.write_i16(cmd.forwardmove)?;
        }
        if bits & CM_SIDE != 0 {
            self.write_i16(cmd.sidemove)?;
        }
        if bits & CM_UP != 0 {
            self.write_i16(cmd.upmove)?;
        }
        if bits & CM_BUTTONS != 0 {
            self.write_u8(cmd.buttons)?;
        }
        if bits & CM_IMPULSE != 0 {
            self.write_u8(cmd.impulse)?;
        }

        self.write_u8(cmd.msec)
    }
}

#[derive(Debug)]
pub enum MsgReadError {
    Eof,
}

#[derive(Debug)]
pub struct MsgReader<'a> {
    data: &'a [u8],
    cursor: usize,
    bad_read: bool,
}

impl<'a> MsgReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            cursor: 0,
            bad_read: false,
        }
    }

    pub fn bad_read(&self) -> bool {
        self.bad_read
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.cursor)
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], MsgReadError> {
        if self.cursor + count > self.data.len() {
            self.bad_read = true;
            return Err(MsgReadError::Eof);
        }
        let start = self.cursor;
        self.cursor += count;
        Ok(&self.data[start..start + count])
    }

    pub fn read_i8(&mut self) -> Result<i8, MsgReadError> {
        let bytes = self.take(1)?;
        Ok(bytes[0] as i8)
    }

    pub fn read_u8(&mut self) -> Result<u8, MsgReadError> {
        let bytes = self.take(1)?;
        Ok(bytes[0])
    }

    pub fn read_i16(&mut self) -> Result<i16, MsgReadError> {
        let bytes = self.take(2)?;
        Ok(i16::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub fn read_u16(&mut self) -> Result<u16, MsgReadError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub fn read_i32(&mut self) -> Result<i32, MsgReadError> {
        let bytes = self.take(4)?;
        Ok(i32::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub fn read_u32(&mut self) -> Result<u32, MsgReadError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub fn read_f32(&mut self) -> Result<f32, MsgReadError> {
        let bytes = self.take(4)?;
        Ok(f32::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub fn read_string(&mut self) -> Result<String, MsgReadError> {
        let mut out = Vec::new();
        loop {
            let value = self.read_i8()?;
            if value == 0 {
                break;
            }
            out.push(value as u8);
        }
        Ok(String::from_utf8_lossy(&out).to_string())
    }

    pub fn read_string_line(&mut self) -> Result<String, MsgReadError> {
        let mut out = Vec::new();
        loop {
            let value = self.read_i8()?;
            if value == 0 || value == b'\n' as i8 {
                break;
            }
            out.push(value as u8);
        }
        Ok(String::from_utf8_lossy(&out).to_string())
    }

    pub fn read_coord(&mut self) -> Result<f32, MsgReadError> {
        Ok(self.read_i16()? as f32 * (1.0 / 8.0))
    }

    pub fn read_angle(&mut self) -> Result<f32, MsgReadError> {
        Ok(self.read_i8()? as f32 * (360.0 / 256.0))
    }

    pub fn read_angle16(&mut self) -> Result<f32, MsgReadError> {
        Ok(self.read_i16()? as f32 * (360.0 / 65536.0))
    }

    pub fn read_delta_usercmd(&mut self, from: &UserCmd) -> Result<UserCmd, MsgReadError> {
        let mut cmd = *from;
        let bits = self.read_u8()?;

        if bits & CM_ANGLE1 != 0 {
            cmd.angles.x = self.read_angle16()?;
        }
        if bits & CM_ANGLE2 != 0 {
            cmd.angles.y = self.read_angle16()?;
        }
        if bits & CM_ANGLE3 != 0 {
            cmd.angles.z = self.read_angle16()?;
        }
        if bits & CM_FORWARD != 0 {
            cmd.forwardmove = self.read_i16()?;
        }
        if bits & CM_SIDE != 0 {
            cmd.sidemove = self.read_i16()?;
        }
        if bits & CM_UP != 0 {
            cmd.upmove = self.read_i16()?;
        }
        if bits & CM_BUTTONS != 0 {
            cmd.buttons = self.read_u8()?;
        }
        if bits & CM_IMPULSE != 0 {
            cmd.impulse = self.read_u8()?;
        }

        cmd.msec = self.read_u8()?;
        Ok(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Vec3;

    #[test]
    fn write_and_read_numbers() {
        let mut buf = SizeBuf::new(64);
        buf.write_i8(-5).unwrap();
        buf.write_u8(250).unwrap();
        buf.write_i16(-1234).unwrap();
        buf.write_u16(65530).unwrap();
        buf.write_i32(-123456).unwrap();
        buf.write_u32(123456).unwrap();
        buf.write_f32(3.5).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        assert_eq!(reader.read_i8().unwrap(), -5);
        assert_eq!(reader.read_u8().unwrap(), 250);
        assert_eq!(reader.read_i16().unwrap(), -1234);
        assert_eq!(reader.read_u16().unwrap(), 65530);
        assert_eq!(reader.read_i32().unwrap(), -123456);
        assert_eq!(reader.read_u32().unwrap(), 123456);
        assert!((reader.read_f32().unwrap() - 3.5).abs() < f32::EPSILON);
    }

    #[test]
    fn write_and_read_strings() {
        let mut buf = SizeBuf::new(64);
        buf.write_string(Some("hello")).unwrap();
        buf.write_string(None).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        assert_eq!(reader.read_string().unwrap(), "hello");
        assert_eq!(reader.read_string().unwrap(), "");
    }

    #[test]
    fn coord_and_angle_round_trip() {
        let mut buf = SizeBuf::new(64);
        buf.write_coord(12.25).unwrap();
        buf.write_angle(90.0).unwrap();
        buf.write_angle16(45.0).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        assert!((reader.read_coord().unwrap() - 12.25).abs() < 0.2);
        assert!((reader.read_angle().unwrap() - 90.0).abs() < 1.5);
        assert!((reader.read_angle16().unwrap() - 45.0).abs() < 0.2);
    }

    #[test]
    fn delta_usercmd_round_trip() {
        let from = UserCmd::default();
        let cmd = UserCmd {
            msec: 12,
            angles: Vec3::new(10.0, 20.0, 30.0),
            forwardmove: 200,
            sidemove: -40,
            upmove: 8,
            buttons: 2,
            impulse: 5,
        };

        let mut buf = SizeBuf::new(128);
        buf.write_delta_usercmd(&from, &cmd).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let decoded = reader.read_delta_usercmd(&from).unwrap();

        assert_eq!(decoded.msec, cmd.msec);
        assert_eq!(decoded.forwardmove, cmd.forwardmove);
        assert_eq!(decoded.sidemove, cmd.sidemove);
        assert_eq!(decoded.upmove, cmd.upmove);
        assert_eq!(decoded.buttons, cmd.buttons);
        assert_eq!(decoded.impulse, cmd.impulse);
        assert!((decoded.angles.x - cmd.angles.x).abs() < 0.2);
        assert!((decoded.angles.y - cmd.angles.y).abs() < 0.2);
        assert!((decoded.angles.z - cmd.angles.z).abs() < 0.2);
    }
}
