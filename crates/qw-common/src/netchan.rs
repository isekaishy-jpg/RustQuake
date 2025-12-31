// QuakeWorld netchan header encode/decode helpers.

use crate::defs::MAX_MSGLEN;
use crate::msg::{MsgReadError, MsgReader, SizeBuf, SizeBufError};

const RELIABLE_FLAG: u32 = 1 << 31;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetchanHeader {
    pub sequence: u32,
    pub reliable: bool,
    pub ack: u32,
    pub ack_reliable: bool,
    pub qport: Option<u16>,
}

#[derive(Debug)]
pub enum NetchanError {
    BufferOverflow,
    OutOfOrder,
    Read(MsgReadError),
    Write(SizeBufError),
}

impl From<MsgReadError> for NetchanError {
    fn from(err: MsgReadError) -> Self {
        NetchanError::Read(err)
    }
}

impl From<SizeBufError> for NetchanError {
    fn from(err: SizeBufError) -> Self {
        NetchanError::Write(err)
    }
}

#[derive(Debug)]
pub struct Netchan {
    pub qport: u16,
    outgoing_sequence: u32,
    incoming_sequence: u32,
    incoming_reliable_sequence: u32,
    incoming_acknowledged: u32,
    incoming_reliable_acknowledged: u32,
    reliable_sequence: u32,
    last_reliable_sequence: u32,
    reliable_buffer: Vec<u8>,
    reliable_length: usize,
    message: SizeBuf,
    received_any: bool,
}

impl Netchan {
    pub fn new(qport: u16) -> Self {
        Self {
            qport,
            outgoing_sequence: 0,
            incoming_sequence: 0,
            incoming_reliable_sequence: 0,
            incoming_acknowledged: 0,
            incoming_reliable_acknowledged: 0,
            reliable_sequence: 0,
            last_reliable_sequence: 0,
            reliable_buffer: Vec::new(),
            reliable_length: 0,
            message: SizeBuf::new(MAX_MSGLEN).with_overflow(true),
            received_any: false,
        }
    }

    pub fn queue_reliable(&mut self, payload: &[u8]) -> Result<(), NetchanError> {
        self.message.write_bytes(payload)?;
        Ok(())
    }

    pub fn build_packet(
        &mut self,
        unreliable: &[u8],
        include_qport: bool,
    ) -> Result<Vec<u8>, NetchanError> {
        if self.message.is_overflowed() {
            return Err(NetchanError::BufferOverflow);
        }

        let mut send_reliable = false;
        if self.incoming_acknowledged > self.last_reliable_sequence
            && self.incoming_reliable_acknowledged != self.reliable_sequence
        {
            send_reliable = true;
        }

        if self.reliable_length == 0 && self.message.len() > 0 {
            self.reliable_buffer = self.message.as_slice().to_vec();
            self.reliable_length = self.message.len();
            self.message.clear();
            self.reliable_sequence ^= 1;
            send_reliable = true;
        }

        let header_size = if include_qport { 10 } else { 8 };
        let mut send = SizeBuf::new(MAX_MSGLEN + header_size);

        let header = NetchanHeader {
            sequence: self.outgoing_sequence,
            reliable: send_reliable,
            ack: self.incoming_sequence,
            ack_reliable: self.incoming_reliable_sequence != 0,
            qport: if include_qport { Some(self.qport) } else { None },
        };

        self.outgoing_sequence = self.outgoing_sequence.wrapping_add(1);
        header.write(&mut send)?;

        if send_reliable {
            send.write_bytes(&self.reliable_buffer[..self.reliable_length])?;
            self.last_reliable_sequence = self.outgoing_sequence;
        }

        if send.len() + unreliable.len() <= send.maxsize() {
            send.write_bytes(unreliable)?;
        }

        Ok(send.as_slice().to_vec())
    }

    pub fn process_packet<'a>(
        &mut self,
        packet: &'a [u8],
        expects_qport: bool,
    ) -> Result<&'a [u8], NetchanError> {
        let mut reader = MsgReader::new(packet);
        let header = NetchanHeader::read(&mut reader, expects_qport)?;

        if self.received_any && header.sequence <= self.incoming_sequence {
            return Err(NetchanError::OutOfOrder);
        }

        self.received_any = true;
        self.incoming_sequence = header.sequence;
        self.incoming_reliable_sequence = if header.reliable { 1 } else { 0 };
        self.incoming_acknowledged = header.ack;
        self.incoming_reliable_acknowledged = if header.ack_reliable { 1 } else { 0 };

        let offset = packet.len() - reader.remaining();
        Ok(&packet[offset..])
    }

    pub fn incoming_sequence(&self) -> u32 {
        self.incoming_sequence
    }

    pub fn outgoing_sequence(&self) -> u32 {
        self.outgoing_sequence
    }
}

impl NetchanHeader {
    pub fn write(&self, buf: &mut SizeBuf) -> Result<(), SizeBufError> {
        let seq = (self.sequence & !RELIABLE_FLAG) | if self.reliable { RELIABLE_FLAG } else { 0 };
        let ack = (self.ack & !RELIABLE_FLAG) | if self.ack_reliable { RELIABLE_FLAG } else { 0 };

        buf.write_u32(seq)?;
        buf.write_u32(ack)?;
        if let Some(qport) = self.qport {
            buf.write_u16(qport)?;
        }
        Ok(())
    }

    pub fn read(reader: &mut MsgReader, expects_qport: bool) -> Result<Self, MsgReadError> {
        let raw_seq = reader.read_u32()?;
        let raw_ack = reader.read_u32()?;

        let reliable = (raw_seq & RELIABLE_FLAG) != 0;
        let ack_reliable = (raw_ack & RELIABLE_FLAG) != 0;

        let sequence = raw_seq & !RELIABLE_FLAG;
        let ack = raw_ack & !RELIABLE_FLAG;

        let qport = if expects_qport {
            Some(reader.read_u16()?)
        } else {
            None
        };

        Ok(NetchanHeader {
            sequence,
            reliable,
            ack,
            ack_reliable,
            qport,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::SizeBuf;

    #[test]
    fn round_trip_with_qport() {
        let header = NetchanHeader {
            sequence: 123,
            reliable: true,
            ack: 98,
            ack_reliable: false,
            qport: Some(27001),
        };

        let mut buf = SizeBuf::new(16);
        header.write(&mut buf).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let decoded = NetchanHeader::read(&mut reader, true).unwrap();

        assert_eq!(decoded, header);
    }

    #[test]
    fn round_trip_without_qport() {
        let header = NetchanHeader {
            sequence: 1,
            reliable: false,
            ack: 2,
            ack_reliable: true,
            qport: None,
        };

        let mut buf = SizeBuf::new(16);
        header.write(&mut buf).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let decoded = NetchanHeader::read(&mut reader, false).unwrap();

        assert_eq!(decoded, header);
    }

    #[test]
    fn builds_and_processes_packet() {
        let mut sender = Netchan::new(27001);
        let packet = sender.build_packet(b"hello", true).unwrap();

        let mut receiver = Netchan::new(27001);
        let payload = receiver.process_packet(&packet, true).unwrap();
        assert_eq!(payload, b"hello");
    }
}
