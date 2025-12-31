// Server-to-client message parsing.

use crate::client_messages::{
    ServerData, StringListChunk, parse_serverdata, parse_string_list_chunk,
};
use crate::msg::{MsgReadError, MsgReader, SizeBuf, SizeBufError};
use crate::protocol::{
    DEFAULT_SOUND_PACKET_ATTENUATION, DEFAULT_SOUND_PACKET_VOLUME, DEFAULT_VIEWHEIGHT, PF_COMMAND,
    PF_EFFECTS, PF_MODEL, PF_MSEC, PF_SKINNUM, PF_VELOCITY1, PF_VELOCITY2, PF_VELOCITY3,
    PF_WEAPONFRAME, SND_ATTENUATION, SND_VOLUME, SU_ARMOR, SU_IDEALPITCH, SU_INWATER, SU_ONGROUND,
    SU_PUNCH1, SU_PUNCH2, SU_PUNCH3, SU_VELOCITY1, SU_VELOCITY2, SU_VELOCITY3, SU_VIEWHEIGHT,
    SU_WEAPON, SU_WEAPONFRAME, Svc, TE_BLOOD, TE_EXPLOSION, TE_GUNSHOT, TE_KNIGHTSPIKE,
    TE_LAVASPLASH, TE_LIGHTNING1, TE_LIGHTNING2, TE_LIGHTNING3, TE_LIGHTNINGBLOOD, TE_SPIKE,
    TE_SUPERSPIKE, TE_TAREXPLOSION, TE_TELEPORT, TE_WIZSPIKE, U_ANGLE1, U_ANGLE2, U_ANGLE3,
    U_COLORMAP, U_EFFECTS, U_FRAME, U_MODEL, U_MOREBITS, U_ORIGIN1, U_ORIGIN2, U_ORIGIN3, U_REMOVE,
    U_SKIN, U_SOLID,
};
use crate::types::{EntityState, UserCmd, Vec3};

#[derive(Debug, Clone, PartialEq)]
pub enum SvcMessage {
    Nop,
    Disconnect,
    ServerData(ServerData),
    Version(i32),
    Time(f32),
    Print {
        level: u8,
        message: String,
    },
    CenterPrint(String),
    StuffText(String),
    SoundList(StringListChunk),
    ModelList(StringListChunk),
    LightStyle {
        style: u8,
        value: String,
    },
    UpdateName {
        slot: u8,
        name: String,
    },
    SetView {
        entity: u16,
    },
    SetAngle(Vec3),
    ClientData(ClientDataMessage),
    Damage {
        armor: u8,
        blood: u8,
        origin: Vec3,
    },
    SetPause(bool),
    SignonNum(u8),
    SpawnStatic(EntityState),
    SpawnStaticSound {
        origin: Vec3,
        sound: u8,
        volume: u8,
        attenuation: u8,
    },
    Intermission {
        origin: Vec3,
        angles: Vec3,
    },
    Finale(String),
    CdTrack(u8),
    SellScreen,
    SmallKick,
    BigKick,
    MuzzleFlash {
        entity: u16,
    },
    UpdateStat {
        index: u8,
        value: u8,
    },
    UpdateStatLong {
        index: u8,
        value: i32,
    },
    KilledMonster,
    FoundSecret,
    MaxSpeed(f32),
    EntGravity(f32),
    UpdateColors {
        slot: u8,
        colors: u8,
    },
    Particle(ParticleEffect),
    TempEntity(TempEntityMessage),
    Sound(SoundMessage),
    StopSound {
        entity: u16,
        channel: u8,
    },
    Download {
        size: i16,
        percent: u8,
        data: Vec<u8>,
    },
    Nails {
        projectiles: Vec<NailProjectile>,
    },
    ChokeCount(u8),
    UpdateFrags {
        slot: u8,
        frags: i16,
    },
    UpdatePing {
        slot: u8,
        ping: i16,
    },
    UpdatePl {
        slot: u8,
        packet_loss: u8,
    },
    UpdateEnterTime {
        slot: u8,
        seconds_ago: f32,
    },
    UpdateUserInfo {
        slot: u8,
        user_id: i32,
        userinfo: String,
    },
    SetInfo {
        slot: u8,
        key: String,
        value: String,
    },
    ServerInfo {
        key: String,
        value: String,
    },
    PlayerInfo(PlayerInfoMessage),
    SpawnBaseline {
        entity: u16,
        baseline: EntityState,
    },
    PacketEntities(PacketEntitiesUpdate),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerInfoMessage {
    pub num: u8,
    pub flags: u16,
    pub origin: Vec3,
    pub frame: u8,
    pub msec: Option<u8>,
    pub command: Option<UserCmd>,
    pub velocity: [i16; 3],
    pub model_index: Option<u8>,
    pub skin_num: Option<u8>,
    pub effects: Option<u8>,
    pub weapon_frame: Option<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoundMessage {
    pub entity: u16,
    pub channel: u8,
    pub sound_num: u8,
    pub volume: u8,
    pub attenuation: f32,
    pub origin: Vec3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParticleEffect {
    pub origin: Vec3,
    pub direction: Vec3,
    pub count: u16,
    pub color: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TempEntityMessage {
    pub kind: u8,
    pub origin: Option<Vec3>,
    pub start: Option<Vec3>,
    pub end: Option<Vec3>,
    pub count: Option<u8>,
    pub entity: Option<u16>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClientDataMessage {
    pub bits: u16,
    pub view_height: i8,
    pub ideal_pitch: i8,
    pub punch_angle: Vec3,
    pub velocity: Vec3,
    pub items: i32,
    pub onground: bool,
    pub inwater: bool,
    pub weapon_frame: u8,
    pub armor: u8,
    pub weapon: u8,
    pub health: i16,
    pub ammo: u8,
    pub ammo_counts: [u8; 4],
    pub active_weapon: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NailProjectile {
    pub origin: Vec3,
    pub angles: Vec3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityDelta {
    pub number: u16,
    pub remove: bool,
    pub flags: u16,
    pub model_index: Option<u8>,
    pub frame: Option<u8>,
    pub colormap: Option<u8>,
    pub skin_num: Option<u8>,
    pub effects: Option<u8>,
    pub origin: [Option<f32>; 3],
    pub angles: [Option<f32>; 3],
    pub solid: bool,
}

impl EntityDelta {
    pub fn apply_to(&self, from: &EntityState) -> EntityState {
        let mut state = *from;
        state.number = self.number as i32;
        state.flags = self.flags as i32;

        if let Some(value) = self.model_index {
            state.modelindex = value as i32;
        }
        if let Some(value) = self.frame {
            state.frame = value as i32;
        }
        if let Some(value) = self.colormap {
            state.colormap = value as i32;
        }
        if let Some(value) = self.skin_num {
            state.skinnum = value as i32;
        }
        if let Some(value) = self.effects {
            state.effects = value as i32;
        }

        if let Some(value) = self.origin[0] {
            state.origin.x = value;
        }
        if let Some(value) = self.origin[1] {
            state.origin.y = value;
        }
        if let Some(value) = self.origin[2] {
            state.origin.z = value;
        }

        if let Some(value) = self.angles[0] {
            state.angles.x = value;
        }
        if let Some(value) = self.angles[1] {
            state.angles.y = value;
        }
        if let Some(value) = self.angles[2] {
            state.angles.z = value;
        }

        state
    }

    pub fn compute_flags(&self) -> u16 {
        let mut flags = 0u16;
        if self.origin[0].is_some() {
            flags |= U_ORIGIN1;
        }
        if self.origin[1].is_some() {
            flags |= U_ORIGIN2;
        }
        if self.origin[2].is_some() {
            flags |= U_ORIGIN3;
        }
        if self.angles[1].is_some() {
            flags |= U_ANGLE2;
        }
        if self.frame.is_some() {
            flags |= U_FRAME;
        }
        if self.remove {
            flags |= U_REMOVE;
        }

        let mut more = 0u8;
        if self.angles[0].is_some() {
            more |= U_ANGLE1;
        }
        if self.angles[2].is_some() {
            more |= U_ANGLE3;
        }
        if self.model_index.is_some() {
            more |= U_MODEL;
        }
        if self.colormap.is_some() {
            more |= U_COLORMAP;
        }
        if self.skin_num.is_some() {
            more |= U_SKIN;
        }
        if self.effects.is_some() {
            more |= U_EFFECTS;
        }
        if self.solid {
            more |= U_SOLID;
        }

        if more != 0 {
            flags |= U_MOREBITS;
            flags |= more as u16;
        }

        flags
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PacketEntitiesUpdate {
    pub delta_from: Option<u8>,
    pub entities: Vec<EntityDelta>,
}

#[derive(Debug)]
pub enum SvcParseError {
    Read(MsgReadError),
    UnknownSvc(u8),
    UnsupportedSvc(Svc),
}

impl From<MsgReadError> for SvcParseError {
    fn from(err: MsgReadError) -> Self {
        SvcParseError::Read(err)
    }
}

fn parse_baseline(reader: &mut MsgReader) -> Result<EntityState, MsgReadError> {
    let modelindex = reader.read_u8()? as i32;
    let frame = reader.read_u8()? as i32;
    let colormap = reader.read_u8()? as i32;
    let skinnum = reader.read_u8()? as i32;
    let origin = Vec3::new(
        reader.read_coord()?,
        reader.read_coord()?,
        reader.read_coord()?,
    );
    let angles = Vec3::new(
        reader.read_angle()?,
        reader.read_angle()?,
        reader.read_angle()?,
    );

    Ok(EntityState {
        number: 0,
        flags: 0,
        origin,
        angles,
        modelindex,
        frame,
        colormap,
        skinnum,
        effects: 0,
    })
}

fn parse_sound(reader: &mut MsgReader) -> Result<SoundMessage, MsgReadError> {
    let field = reader.read_u16()?;
    let volume = if field & SND_VOLUME != 0 {
        reader.read_u8()?
    } else {
        DEFAULT_SOUND_PACKET_VOLUME
    };
    let attenuation = if field & SND_ATTENUATION != 0 {
        reader.read_u8()? as f32 / 64.0
    } else {
        DEFAULT_SOUND_PACKET_ATTENUATION
    };
    let sound_num = reader.read_u8()?;
    let origin = Vec3::new(
        reader.read_coord()?,
        reader.read_coord()?,
        reader.read_coord()?,
    );

    let entity = ((field >> 3) & 1023) as u16;
    let channel = (field & 7) as u8;

    Ok(SoundMessage {
        entity,
        channel,
        sound_num,
        volume,
        attenuation,
        origin,
    })
}

fn parse_clientdata(reader: &mut MsgReader) -> Result<ClientDataMessage, MsgReadError> {
    let bits = reader.read_u16()?;
    let view_height = if bits & SU_VIEWHEIGHT != 0 {
        reader.read_i8()?
    } else {
        DEFAULT_VIEWHEIGHT
    };
    let ideal_pitch = if bits & SU_IDEALPITCH != 0 {
        reader.read_i8()?
    } else {
        0
    };

    let punch_bits = [SU_PUNCH1, SU_PUNCH2, SU_PUNCH3];
    let velocity_bits = [SU_VELOCITY1, SU_VELOCITY2, SU_VELOCITY3];
    let mut punch = [0.0; 3];
    let mut velocity = [0.0; 3];
    for i in 0..3 {
        if bits & punch_bits[i] != 0 {
            punch[i] = reader.read_i8()? as f32;
        }
        if bits & velocity_bits[i] != 0 {
            velocity[i] = reader.read_i8()? as f32 * 16.0;
        }
    }

    let items = reader.read_i32()?;
    let onground = bits & SU_ONGROUND != 0;
    let inwater = bits & SU_INWATER != 0;

    let weapon_frame = if bits & SU_WEAPONFRAME != 0 {
        reader.read_u8()?
    } else {
        0
    };
    let armor = if bits & SU_ARMOR != 0 {
        reader.read_u8()?
    } else {
        0
    };
    let weapon = if bits & SU_WEAPON != 0 {
        reader.read_u8()?
    } else {
        0
    };

    let health = reader.read_i16()?;
    let ammo = reader.read_u8()?;
    let mut ammo_counts = [0u8; 4];
    for count in &mut ammo_counts {
        *count = reader.read_u8()?;
    }
    let active_weapon = reader.read_u8()?;

    Ok(ClientDataMessage {
        bits,
        view_height,
        ideal_pitch,
        punch_angle: Vec3::new(punch[0], punch[1], punch[2]),
        velocity: Vec3::new(velocity[0], velocity[1], velocity[2]),
        items,
        onground,
        inwater,
        weapon_frame,
        armor,
        weapon,
        health,
        ammo,
        ammo_counts,
        active_weapon,
    })
}

fn decode_nail_projectile(bits: [u8; 6]) -> NailProjectile {
    let packed_x = (bits[0] as i32) + (((bits[1] & 0x0f) as i32) << 8);
    let packed_y = ((bits[1] >> 4) as i32) + ((bits[2] as i32) << 4);
    let packed_z = (bits[3] as i32) + (((bits[4] & 0x0f) as i32) << 8);

    let origin = Vec3::new(
        ((packed_x << 1) - 4096) as f32,
        ((packed_y << 1) - 4096) as f32,
        ((packed_z << 1) - 4096) as f32,
    );
    let pitch = 360.0 * (bits[4] >> 4) as f32 / 16.0;
    let yaw = 360.0 * (bits[5] as f32) / 256.0;

    NailProjectile {
        origin,
        angles: Vec3::new(pitch, yaw, 0.0),
    }
}

fn quantize_nail_coord(value: f32) -> i32 {
    let scaled = ((value + 4096.0) / 2.0).round() as i32;
    scaled.clamp(0, 4095)
}

fn quantize_nail_angle(value: f32, steps: i32) -> u8 {
    let normalized = value.rem_euclid(360.0);
    let scaled = (normalized / 360.0) * steps as f32;
    let index = scaled.round() as i32;
    index.clamp(0, steps - 1) as u8
}

fn encode_nail_projectile(projectile: &NailProjectile) -> [u8; 6] {
    let packed_x = quantize_nail_coord(projectile.origin.x);
    let packed_y = quantize_nail_coord(projectile.origin.y);
    let packed_z = quantize_nail_coord(projectile.origin.z);

    let pitch = quantize_nail_angle(projectile.angles.x, 16);
    let yaw = quantize_nail_angle(projectile.angles.y, 256);

    let b0 = (packed_x & 0xff) as u8;
    let b1_low = ((packed_x >> 8) & 0x0f) as u8;
    let b1_high = (packed_y & 0x0f) as u8;
    let b1 = (b1_high << 4) | b1_low;
    let b2 = ((packed_y >> 4) & 0xff) as u8;
    let b3 = (packed_z & 0xff) as u8;
    let b4_low = ((packed_z >> 8) & 0x0f) as u8;
    let b4 = (pitch << 4) | b4_low;
    let b5 = yaw;

    [b0, b1, b2, b3, b4, b5]
}

fn quantize_clientdata_i8(value: f32) -> i8 {
    value.round().clamp(-128.0, 127.0) as i8
}

fn quantize_clientdata_velocity(value: f32) -> i8 {
    (value / 16.0).round().clamp(-128.0, 127.0) as i8
}

fn write_clientdata(buf: &mut SizeBuf, data: &ClientDataMessage) -> Result<(), SizeBufError> {
    let mut bits = data.bits;
    if data.onground {
        bits |= SU_ONGROUND;
    } else {
        bits &= !SU_ONGROUND;
    }
    if data.inwater {
        bits |= SU_INWATER;
    } else {
        bits &= !SU_INWATER;
    }

    buf.write_u16(bits)?;
    if bits & SU_VIEWHEIGHT != 0 {
        buf.write_i8(data.view_height)?;
    }
    if bits & SU_IDEALPITCH != 0 {
        buf.write_i8(data.ideal_pitch)?;
    }

    let punch_bits = [SU_PUNCH1, SU_PUNCH2, SU_PUNCH3];
    let velocity_bits = [SU_VELOCITY1, SU_VELOCITY2, SU_VELOCITY3];
    let punch = [data.punch_angle.x, data.punch_angle.y, data.punch_angle.z];
    let velocity = [data.velocity.x, data.velocity.y, data.velocity.z];
    for i in 0..3 {
        if bits & punch_bits[i] != 0 {
            buf.write_i8(quantize_clientdata_i8(punch[i]))?;
        }
        if bits & velocity_bits[i] != 0 {
            buf.write_i8(quantize_clientdata_velocity(velocity[i]))?;
        }
    }

    buf.write_i32(data.items)?;
    if bits & SU_WEAPONFRAME != 0 {
        buf.write_u8(data.weapon_frame)?;
    }
    if bits & SU_ARMOR != 0 {
        buf.write_u8(data.armor)?;
    }
    if bits & SU_WEAPON != 0 {
        buf.write_u8(data.weapon)?;
    }

    buf.write_i16(data.health)?;
    buf.write_u8(data.ammo)?;
    for count in data.ammo_counts {
        buf.write_u8(count)?;
    }
    buf.write_u8(data.active_weapon)?;
    Ok(())
}

fn parse_entity_delta(reader: &mut MsgReader, word: u16) -> Result<EntityDelta, MsgReadError> {
    let mut bits = word;
    let number = (bits & 0x1ff) as u16;
    bits &= !0x1ff;

    if bits & U_MOREBITS != 0 {
        bits |= reader.read_u8()? as u16;
    }

    let model_index = if bits & (U_MODEL as u16) != 0 {
        Some(reader.read_u8()?)
    } else {
        None
    };

    let frame = if bits & U_FRAME != 0 {
        Some(reader.read_u8()?)
    } else {
        None
    };

    let colormap = if bits & (U_COLORMAP as u16) != 0 {
        Some(reader.read_u8()?)
    } else {
        None
    };

    let skin_num = if bits & (U_SKIN as u16) != 0 {
        Some(reader.read_u8()?)
    } else {
        None
    };

    let effects = if bits & (U_EFFECTS as u16) != 0 {
        Some(reader.read_u8()?)
    } else {
        None
    };

    let mut origin = [None; 3];
    let mut angles = [None; 3];

    if bits & U_ORIGIN1 != 0 {
        origin[0] = Some(reader.read_coord()?);
    }
    if bits & (U_ANGLE1 as u16) != 0 {
        angles[0] = Some(reader.read_angle()?);
    }
    if bits & U_ORIGIN2 != 0 {
        origin[1] = Some(reader.read_coord()?);
    }
    if bits & U_ANGLE2 != 0 {
        angles[1] = Some(reader.read_angle()?);
    }
    if bits & U_ORIGIN3 != 0 {
        origin[2] = Some(reader.read_coord()?);
    }
    if bits & (U_ANGLE3 as u16) != 0 {
        angles[2] = Some(reader.read_angle()?);
    }

    let solid = bits & (U_SOLID as u16) != 0;

    Ok(EntityDelta {
        number,
        remove: bits & U_REMOVE != 0,
        flags: bits,
        model_index,
        frame,
        colormap,
        skin_num,
        effects,
        origin,
        angles,
        solid,
    })
}

fn parse_temp_entity(reader: &mut MsgReader) -> Result<TempEntityMessage, SvcParseError> {
    let kind = reader.read_u8()?;
    match kind {
        TE_LIGHTNING1 | TE_LIGHTNING2 | TE_LIGHTNING3 => {
            let entity = reader.read_u16()?;
            let start = Vec3::new(
                reader.read_coord()?,
                reader.read_coord()?,
                reader.read_coord()?,
            );
            let end = Vec3::new(
                reader.read_coord()?,
                reader.read_coord()?,
                reader.read_coord()?,
            );
            Ok(TempEntityMessage {
                kind,
                origin: None,
                start: Some(start),
                end: Some(end),
                count: None,
                entity: Some(entity),
            })
        }
        TE_GUNSHOT | TE_BLOOD => {
            let count = reader.read_u8()?;
            let origin = Vec3::new(
                reader.read_coord()?,
                reader.read_coord()?,
                reader.read_coord()?,
            );
            Ok(TempEntityMessage {
                kind,
                origin: Some(origin),
                start: None,
                end: None,
                count: Some(count),
                entity: None,
            })
        }
        TE_SPIKE | TE_SUPERSPIKE | TE_WIZSPIKE | TE_KNIGHTSPIKE | TE_EXPLOSION
        | TE_TAREXPLOSION | TE_LAVASPLASH | TE_TELEPORT | TE_LIGHTNINGBLOOD => {
            let origin = Vec3::new(
                reader.read_coord()?,
                reader.read_coord()?,
                reader.read_coord()?,
            );
            Ok(TempEntityMessage {
                kind,
                origin: Some(origin),
                start: None,
                end: None,
                count: None,
                entity: None,
            })
        }
        _ => Err(SvcParseError::UnsupportedSvc(Svc::TempEntity)),
    }
}

fn parse_packet_entities(
    reader: &mut MsgReader,
    delta: bool,
) -> Result<PacketEntitiesUpdate, MsgReadError> {
    let delta_from = if delta { Some(reader.read_u8()?) } else { None };
    let mut entities = Vec::new();
    loop {
        let word = reader.read_u16()?;
        if word == 0 {
            break;
        }
        let delta = parse_entity_delta(reader, word)?;
        if entities.len() < crate::protocol::MAX_PACKET_ENTITIES {
            entities.push(delta);
        }
    }
    Ok(PacketEntitiesUpdate {
        delta_from,
        entities,
    })
}

fn write_baseline(buf: &mut SizeBuf, baseline: &EntityState) -> Result<(), SizeBufError> {
    buf.write_u8(baseline.modelindex as u8)?;
    buf.write_u8(baseline.frame as u8)?;
    buf.write_u8(baseline.colormap as u8)?;
    buf.write_u8(baseline.skinnum as u8)?;
    buf.write_coord(baseline.origin.x)?;
    buf.write_coord(baseline.origin.y)?;
    buf.write_coord(baseline.origin.z)?;
    buf.write_angle(baseline.angles.x)?;
    buf.write_angle(baseline.angles.y)?;
    buf.write_angle(baseline.angles.z)?;
    Ok(())
}

fn write_entity_delta(buf: &mut SizeBuf, delta: &EntityDelta) -> Result<(), SizeBufError> {
    let flags = delta.compute_flags();
    let mut word = (delta.number & 0x1ff) as u16;
    word |= flags & !0x1ff;
    buf.write_u16(word)?;

    if flags & U_MOREBITS != 0 {
        buf.write_u8((flags & 0xff) as u8)?;
    }

    if delta.model_index.is_some() {
        buf.write_u8(delta.model_index.unwrap())?;
    }
    if delta.frame.is_some() {
        buf.write_u8(delta.frame.unwrap())?;
    }
    if delta.colormap.is_some() {
        buf.write_u8(delta.colormap.unwrap())?;
    }
    if delta.skin_num.is_some() {
        buf.write_u8(delta.skin_num.unwrap())?;
    }
    if delta.effects.is_some() {
        buf.write_u8(delta.effects.unwrap())?;
    }
    if let Some(value) = delta.origin[0] {
        buf.write_coord(value)?;
    }
    if let Some(value) = delta.angles[0] {
        buf.write_angle(value)?;
    }
    if let Some(value) = delta.origin[1] {
        buf.write_coord(value)?;
    }
    if let Some(value) = delta.angles[1] {
        buf.write_angle(value)?;
    }
    if let Some(value) = delta.origin[2] {
        buf.write_coord(value)?;
    }
    if let Some(value) = delta.angles[2] {
        buf.write_angle(value)?;
    }

    Ok(())
}

fn parse_particle(reader: &mut MsgReader) -> Result<ParticleEffect, MsgReadError> {
    let origin = Vec3::new(
        reader.read_coord()?,
        reader.read_coord()?,
        reader.read_coord()?,
    );
    let direction = Vec3::new(
        reader.read_i8()? as f32 * (1.0 / 16.0),
        reader.read_i8()? as f32 * (1.0 / 16.0),
        reader.read_i8()? as f32 * (1.0 / 16.0),
    );
    let msgcount = reader.read_u8()?;
    let count = if msgcount == 255 {
        1024
    } else {
        msgcount as u16
    };
    let color = reader.read_u8()?;
    Ok(ParticleEffect {
        origin,
        direction,
        count,
        color,
    })
}

pub fn parse_svc_message(reader: &mut MsgReader) -> Result<SvcMessage, SvcParseError> {
    let cmd = reader.read_u8()?;
    let svc = Svc::try_from(cmd).map_err(|_| SvcParseError::UnknownSvc(cmd))?;
    match svc {
        Svc::Nop => Ok(SvcMessage::Nop),
        Svc::Disconnect => Ok(SvcMessage::Disconnect),
        Svc::Version => Ok(SvcMessage::Version(reader.read_i32()?)),
        Svc::ServerData => Ok(SvcMessage::ServerData(parse_serverdata(reader).map_err(
            |err| match err {
                crate::client_messages::ServerDataError::Read(e) => SvcParseError::Read(e),
                _ => SvcParseError::UnsupportedSvc(Svc::ServerData),
            },
        )?)),
        Svc::Time => Ok(SvcMessage::Time(reader.read_f32()?)),
        Svc::Print => {
            let level = reader.read_u8()?;
            let message = reader.read_string()?;
            Ok(SvcMessage::Print { level, message })
        }
        Svc::CenterPrint => {
            let message = reader.read_string()?;
            Ok(SvcMessage::CenterPrint(message))
        }
        Svc::StuffText => {
            let text = reader.read_string()?;
            Ok(SvcMessage::StuffText(text))
        }
        Svc::SoundList => Ok(SvcMessage::SoundList(parse_string_list_chunk(reader)?)),
        Svc::ModelList => Ok(SvcMessage::ModelList(parse_string_list_chunk(reader)?)),
        Svc::LightStyle => {
            let style = reader.read_u8()?;
            let value = reader.read_string()?;
            Ok(SvcMessage::LightStyle { style, value })
        }
        Svc::UpdateName => {
            let slot = reader.read_u8()?;
            let name = reader.read_string()?;
            Ok(SvcMessage::UpdateName { slot, name })
        }
        Svc::SetView => {
            let entity = reader.read_u16()?;
            Ok(SvcMessage::SetView { entity })
        }
        Svc::SetAngle => {
            let angles = Vec3::new(
                reader.read_angle()?,
                reader.read_angle()?,
                reader.read_angle()?,
            );
            Ok(SvcMessage::SetAngle(angles))
        }
        Svc::Damage => {
            let armor = reader.read_u8()?;
            let blood = reader.read_u8()?;
            let origin = Vec3::new(
                reader.read_coord()?,
                reader.read_coord()?,
                reader.read_coord()?,
            );
            Ok(SvcMessage::Damage {
                armor,
                blood,
                origin,
            })
        }
        Svc::SetPause => {
            let paused = reader.read_u8()? != 0;
            Ok(SvcMessage::SetPause(paused))
        }
        Svc::SignonNum => {
            let value = reader.read_u8()?;
            Ok(SvcMessage::SignonNum(value))
        }
        Svc::SpawnStatic => {
            let baseline = parse_baseline(reader)?;
            Ok(SvcMessage::SpawnStatic(baseline))
        }
        Svc::SpawnStaticSound => {
            let origin = Vec3::new(
                reader.read_coord()?,
                reader.read_coord()?,
                reader.read_coord()?,
            );
            let sound = reader.read_u8()?;
            let volume = reader.read_u8()?;
            let attenuation = reader.read_u8()?;
            Ok(SvcMessage::SpawnStaticSound {
                origin,
                sound,
                volume,
                attenuation,
            })
        }
        Svc::Intermission => {
            let origin = Vec3::new(
                reader.read_coord()?,
                reader.read_coord()?,
                reader.read_coord()?,
            );
            let angles = Vec3::new(
                reader.read_angle()?,
                reader.read_angle()?,
                reader.read_angle()?,
            );
            Ok(SvcMessage::Intermission { origin, angles })
        }
        Svc::Finale => {
            let text = reader.read_string()?;
            Ok(SvcMessage::Finale(text))
        }
        Svc::CdTrack => {
            let track = reader.read_u8()?;
            Ok(SvcMessage::CdTrack(track))
        }
        Svc::SellScreen => Ok(SvcMessage::SellScreen),
        Svc::SmallKick => Ok(SvcMessage::SmallKick),
        Svc::BigKick => Ok(SvcMessage::BigKick),
        Svc::MuzzleFlash => {
            let entity = reader.read_u16()?;
            Ok(SvcMessage::MuzzleFlash { entity })
        }
        Svc::UpdateStat => {
            let index = reader.read_u8()?;
            let value = reader.read_u8()?;
            Ok(SvcMessage::UpdateStat { index, value })
        }
        Svc::UpdateStatLong => {
            let index = reader.read_u8()?;
            let value = reader.read_i32()?;
            Ok(SvcMessage::UpdateStatLong { index, value })
        }
        Svc::KilledMonster => Ok(SvcMessage::KilledMonster),
        Svc::FoundSecret => Ok(SvcMessage::FoundSecret),
        Svc::MaxSpeed => {
            let value = reader.read_f32()?;
            Ok(SvcMessage::MaxSpeed(value))
        }
        Svc::EntGravity => {
            let value = reader.read_f32()?;
            Ok(SvcMessage::EntGravity(value))
        }
        Svc::UpdateColors => {
            let slot = reader.read_u8()?;
            let colors = reader.read_u8()?;
            Ok(SvcMessage::UpdateColors { slot, colors })
        }
        Svc::Particle => Ok(SvcMessage::Particle(parse_particle(reader)?)),
        Svc::TempEntity => Ok(SvcMessage::TempEntity(parse_temp_entity(reader)?)),
        Svc::Sound => Ok(SvcMessage::Sound(parse_sound(reader)?)),
        Svc::ClientData => Ok(SvcMessage::ClientData(parse_clientdata(reader)?)),
        Svc::StopSound => {
            let field = reader.read_u16()?;
            let entity = ((field >> 3) & 1023) as u16;
            let channel = (field & 7) as u8;
            Ok(SvcMessage::StopSound { entity, channel })
        }
        Svc::Download => {
            let size = reader.read_i16()?;
            let percent = reader.read_u8()?;
            let data = if size > 0 {
                reader.read_bytes(size as usize)?
            } else {
                Vec::new()
            };
            Ok(SvcMessage::Download {
                size,
                percent,
                data,
            })
        }
        Svc::Nails => {
            let count = reader.read_u8()?;
            let mut projectiles = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let mut bits = [0u8; 6];
                for idx in 0..6 {
                    bits[idx] = reader.read_u8()?;
                }
                projectiles.push(decode_nail_projectile(bits));
            }
            Ok(SvcMessage::Nails { projectiles })
        }
        Svc::ChokeCount => {
            let count = reader.read_u8()?;
            Ok(SvcMessage::ChokeCount(count))
        }
        Svc::UpdateFrags => {
            let slot = reader.read_u8()?;
            let frags = reader.read_i16()?;
            Ok(SvcMessage::UpdateFrags { slot, frags })
        }
        Svc::UpdatePing => {
            let slot = reader.read_u8()?;
            let ping = reader.read_i16()?;
            Ok(SvcMessage::UpdatePing { slot, ping })
        }
        Svc::UpdatePl => {
            let slot = reader.read_u8()?;
            let packet_loss = reader.read_u8()?;
            Ok(SvcMessage::UpdatePl { slot, packet_loss })
        }
        Svc::UpdateEnterTime => {
            let slot = reader.read_u8()?;
            let seconds_ago = reader.read_f32()?;
            Ok(SvcMessage::UpdateEnterTime { slot, seconds_ago })
        }
        Svc::UpdateUserInfo => {
            let slot = reader.read_u8()?;
            let user_id = reader.read_i32()?;
            let userinfo = reader.read_string()?;
            Ok(SvcMessage::UpdateUserInfo {
                slot,
                user_id,
                userinfo,
            })
        }
        Svc::SetInfo => {
            let slot = reader.read_u8()?;
            let key = reader.read_string()?;
            let value = reader.read_string()?;
            Ok(SvcMessage::SetInfo { slot, key, value })
        }
        Svc::ServerInfo => {
            let key = reader.read_string()?;
            let value = reader.read_string()?;
            Ok(SvcMessage::ServerInfo { key, value })
        }
        Svc::PlayerInfo => {
            let num = reader.read_u8()?;
            let flags = reader.read_u16()?;
            let origin = Vec3::new(
                reader.read_coord()?,
                reader.read_coord()?,
                reader.read_coord()?,
            );
            let frame = reader.read_u8()?;

            let msec = if flags as u32 & PF_MSEC != 0 {
                Some(reader.read_u8()?)
            } else {
                None
            };

            let command = if flags as u32 & PF_COMMAND != 0 {
                Some(reader.read_delta_usercmd(&UserCmd::default())?)
            } else {
                None
            };

            let mut velocity = [0i16; 3];
            let vel_flags = [PF_VELOCITY1, PF_VELOCITY2, PF_VELOCITY3];
            for (i, flag) in vel_flags.iter().enumerate() {
                if flags as u32 & flag != 0 {
                    velocity[i] = reader.read_i16()?;
                }
            }

            let model_index = if flags as u32 & PF_MODEL != 0 {
                Some(reader.read_u8()?)
            } else {
                None
            };

            let skin_num = if flags as u32 & PF_SKINNUM != 0 {
                Some(reader.read_u8()?)
            } else {
                None
            };

            let effects = if flags as u32 & PF_EFFECTS != 0 {
                Some(reader.read_u8()?)
            } else {
                None
            };

            let weapon_frame = if flags as u32 & PF_WEAPONFRAME != 0 {
                Some(reader.read_u8()?)
            } else {
                None
            };

            Ok(SvcMessage::PlayerInfo(PlayerInfoMessage {
                num,
                flags,
                origin,
                frame,
                msec,
                command,
                velocity,
                model_index,
                skin_num,
                effects,
                weapon_frame,
            }))
        }
        Svc::SpawnBaseline => {
            let entity = reader.read_u16()?;
            let baseline = parse_baseline(reader)?;
            Ok(SvcMessage::SpawnBaseline { entity, baseline })
        }
        Svc::PacketEntities => Ok(SvcMessage::PacketEntities(parse_packet_entities(
            reader, false,
        )?)),
        Svc::DeltaPacketEntities => Ok(SvcMessage::PacketEntities(parse_packet_entities(
            reader, true,
        )?)),
        _ => Err(SvcParseError::UnsupportedSvc(svc)),
    }
}

pub fn parse_svc_stream(reader: &mut MsgReader) -> Result<Vec<SvcMessage>, SvcParseError> {
    let mut messages = Vec::new();
    while reader.remaining() > 0 {
        messages.push(parse_svc_message(reader)?);
    }
    Ok(messages)
}

pub fn write_svc_message(buf: &mut SizeBuf, message: &SvcMessage) -> Result<(), SizeBufError> {
    match message {
        SvcMessage::Nop => {
            buf.write_u8(Svc::Nop as u8)?;
        }
        SvcMessage::Disconnect => {
            buf.write_u8(Svc::Disconnect as u8)?;
        }
        SvcMessage::Version(value) => {
            buf.write_u8(Svc::Version as u8)?;
            buf.write_i32(*value)?;
        }
        SvcMessage::ServerData(data) => {
            buf.write_u8(Svc::ServerData as u8)?;
            crate::client_messages::write_serverdata(buf, data)?;
        }
        SvcMessage::Time(value) => {
            buf.write_u8(Svc::Time as u8)?;
            buf.write_f32(*value)?;
        }
        SvcMessage::Print { level, message } => {
            buf.write_u8(Svc::Print as u8)?;
            buf.write_u8(*level)?;
            buf.write_string(Some(message))?;
        }
        SvcMessage::CenterPrint(text) => {
            buf.write_u8(Svc::CenterPrint as u8)?;
            buf.write_string(Some(text))?;
        }
        SvcMessage::StuffText(text) => {
            buf.write_u8(Svc::StuffText as u8)?;
            buf.write_string(Some(text))?;
        }
        SvcMessage::SoundList(chunk) => {
            buf.write_u8(Svc::SoundList as u8)?;
            crate::client_messages::write_string_list_chunk(buf, chunk)?;
        }
        SvcMessage::ModelList(chunk) => {
            buf.write_u8(Svc::ModelList as u8)?;
            crate::client_messages::write_string_list_chunk(buf, chunk)?;
        }
        SvcMessage::LightStyle { style, value } => {
            buf.write_u8(Svc::LightStyle as u8)?;
            buf.write_u8(*style)?;
            buf.write_string(Some(value))?;
        }
        SvcMessage::UpdateName { slot, name } => {
            buf.write_u8(Svc::UpdateName as u8)?;
            buf.write_u8(*slot)?;
            buf.write_string(Some(name))?;
        }
        SvcMessage::SetView { entity } => {
            buf.write_u8(Svc::SetView as u8)?;
            buf.write_u16(*entity)?;
        }
        SvcMessage::SetAngle(angles) => {
            buf.write_u8(Svc::SetAngle as u8)?;
            buf.write_angle(angles.x)?;
            buf.write_angle(angles.y)?;
            buf.write_angle(angles.z)?;
        }
        SvcMessage::ClientData(data) => {
            buf.write_u8(Svc::ClientData as u8)?;
            write_clientdata(buf, data)?;
        }
        SvcMessage::Damage {
            armor,
            blood,
            origin,
        } => {
            buf.write_u8(Svc::Damage as u8)?;
            buf.write_u8(*armor)?;
            buf.write_u8(*blood)?;
            buf.write_coord(origin.x)?;
            buf.write_coord(origin.y)?;
            buf.write_coord(origin.z)?;
        }
        SvcMessage::SetPause(paused) => {
            buf.write_u8(Svc::SetPause as u8)?;
            buf.write_u8(if *paused { 1 } else { 0 })?;
        }
        SvcMessage::SignonNum(value) => {
            buf.write_u8(Svc::SignonNum as u8)?;
            buf.write_u8(*value)?;
        }
        SvcMessage::SpawnStatic(baseline) => {
            buf.write_u8(Svc::SpawnStatic as u8)?;
            write_baseline(buf, baseline)?;
        }
        SvcMessage::SpawnStaticSound {
            origin,
            sound,
            volume,
            attenuation,
        } => {
            buf.write_u8(Svc::SpawnStaticSound as u8)?;
            buf.write_coord(origin.x)?;
            buf.write_coord(origin.y)?;
            buf.write_coord(origin.z)?;
            buf.write_u8(*sound)?;
            buf.write_u8(*volume)?;
            buf.write_u8(*attenuation)?;
        }
        SvcMessage::Intermission { origin, angles } => {
            buf.write_u8(Svc::Intermission as u8)?;
            buf.write_coord(origin.x)?;
            buf.write_coord(origin.y)?;
            buf.write_coord(origin.z)?;
            buf.write_angle(angles.x)?;
            buf.write_angle(angles.y)?;
            buf.write_angle(angles.z)?;
        }
        SvcMessage::Finale(text) => {
            buf.write_u8(Svc::Finale as u8)?;
            buf.write_string(Some(text))?;
        }
        SvcMessage::CdTrack(track) => {
            buf.write_u8(Svc::CdTrack as u8)?;
            buf.write_u8(*track)?;
        }
        SvcMessage::SellScreen => {
            buf.write_u8(Svc::SellScreen as u8)?;
        }
        SvcMessage::SmallKick => {
            buf.write_u8(Svc::SmallKick as u8)?;
        }
        SvcMessage::BigKick => {
            buf.write_u8(Svc::BigKick as u8)?;
        }
        SvcMessage::MuzzleFlash { entity } => {
            buf.write_u8(Svc::MuzzleFlash as u8)?;
            buf.write_u16(*entity)?;
        }
        SvcMessage::UpdateStat { index, value } => {
            buf.write_u8(Svc::UpdateStat as u8)?;
            buf.write_u8(*index)?;
            buf.write_u8(*value)?;
        }
        SvcMessage::UpdateStatLong { index, value } => {
            buf.write_u8(Svc::UpdateStatLong as u8)?;
            buf.write_u8(*index)?;
            buf.write_i32(*value)?;
        }
        SvcMessage::KilledMonster => {
            buf.write_u8(Svc::KilledMonster as u8)?;
        }
        SvcMessage::FoundSecret => {
            buf.write_u8(Svc::FoundSecret as u8)?;
        }
        SvcMessage::MaxSpeed(value) => {
            buf.write_u8(Svc::MaxSpeed as u8)?;
            buf.write_f32(*value)?;
        }
        SvcMessage::EntGravity(value) => {
            buf.write_u8(Svc::EntGravity as u8)?;
            buf.write_f32(*value)?;
        }
        SvcMessage::UpdateColors { slot, colors } => {
            buf.write_u8(Svc::UpdateColors as u8)?;
            buf.write_u8(*slot)?;
            buf.write_u8(*colors)?;
        }
        SvcMessage::Particle(effect) => {
            buf.write_u8(Svc::Particle as u8)?;
            buf.write_coord(effect.origin.x)?;
            buf.write_coord(effect.origin.y)?;
            buf.write_coord(effect.origin.z)?;
            let dir = effect.direction;
            buf.write_i8((dir.x * 16.0).round().clamp(-128.0, 127.0) as i8)?;
            buf.write_i8((dir.y * 16.0).round().clamp(-128.0, 127.0) as i8)?;
            buf.write_i8((dir.z * 16.0).round().clamp(-128.0, 127.0) as i8)?;
            let msgcount = if effect.count >= 1024 {
                255
            } else {
                effect.count.min(254) as u8
            };
            buf.write_u8(msgcount)?;
            buf.write_u8(effect.color)?;
        }
        SvcMessage::TempEntity(temp) => {
            buf.write_u8(Svc::TempEntity as u8)?;
            buf.write_u8(temp.kind)?;
            if let Some(entity) = temp.entity {
                buf.write_u16(entity)?;
            }
            if let Some(count) = temp.count {
                buf.write_u8(count)?;
            }
            if let Some(origin) = temp.origin {
                buf.write_coord(origin.x)?;
                buf.write_coord(origin.y)?;
                buf.write_coord(origin.z)?;
            }
            if let Some(start) = temp.start {
                buf.write_coord(start.x)?;
                buf.write_coord(start.y)?;
                buf.write_coord(start.z)?;
            }
            if let Some(end) = temp.end {
                buf.write_coord(end.x)?;
                buf.write_coord(end.y)?;
                buf.write_coord(end.z)?;
            }
        }
        SvcMessage::Sound(sound) => {
            buf.write_u8(Svc::Sound as u8)?;
            let mut field = ((sound.entity as u16) << 3) | (sound.channel as u16 & 7);
            if sound.volume != DEFAULT_SOUND_PACKET_VOLUME {
                field |= SND_VOLUME;
            }
            if (sound.attenuation - DEFAULT_SOUND_PACKET_ATTENUATION).abs() > f32::EPSILON {
                field |= SND_ATTENUATION;
            }
            buf.write_u16(field)?;
            if field & SND_VOLUME != 0 {
                buf.write_u8(sound.volume)?;
            }
            if field & SND_ATTENUATION != 0 {
                let raw = (sound.attenuation * 64.0).round().clamp(0.0, 255.0);
                buf.write_u8(raw as u8)?;
            }
            buf.write_u8(sound.sound_num)?;
            buf.write_coord(sound.origin.x)?;
            buf.write_coord(sound.origin.y)?;
            buf.write_coord(sound.origin.z)?;
        }
        SvcMessage::StopSound { entity, channel } => {
            buf.write_u8(Svc::StopSound as u8)?;
            let field = ((entity & 1023) << 3) | (*channel as u16 & 7);
            buf.write_u16(field)?;
        }
        SvcMessage::Download {
            size,
            percent,
            data,
        } => {
            buf.write_u8(Svc::Download as u8)?;
            buf.write_i16(*size)?;
            buf.write_u8(*percent)?;
            if *size > 0 {
                let count = (*size as usize).min(data.len());
                buf.write_bytes(&data[..count])?;
            }
        }
        SvcMessage::Nails { projectiles } => {
            buf.write_u8(Svc::Nails as u8)?;
            let count = projectiles.len().min(u8::MAX as usize) as u8;
            buf.write_u8(count)?;
            for projectile in projectiles.iter().take(count as usize) {
                let bits = encode_nail_projectile(projectile);
                for byte in bits {
                    buf.write_u8(byte)?;
                }
            }
        }
        SvcMessage::ChokeCount(count) => {
            buf.write_u8(Svc::ChokeCount as u8)?;
            buf.write_u8(*count)?;
        }
        SvcMessage::UpdateFrags { slot, frags } => {
            buf.write_u8(Svc::UpdateFrags as u8)?;
            buf.write_u8(*slot)?;
            buf.write_i16(*frags)?;
        }
        SvcMessage::UpdatePing { slot, ping } => {
            buf.write_u8(Svc::UpdatePing as u8)?;
            buf.write_u8(*slot)?;
            buf.write_i16(*ping)?;
        }
        SvcMessage::UpdatePl { slot, packet_loss } => {
            buf.write_u8(Svc::UpdatePl as u8)?;
            buf.write_u8(*slot)?;
            buf.write_u8(*packet_loss)?;
        }
        SvcMessage::UpdateEnterTime { slot, seconds_ago } => {
            buf.write_u8(Svc::UpdateEnterTime as u8)?;
            buf.write_u8(*slot)?;
            buf.write_f32(*seconds_ago)?;
        }
        SvcMessage::UpdateUserInfo {
            slot,
            user_id,
            userinfo,
        } => {
            buf.write_u8(Svc::UpdateUserInfo as u8)?;
            buf.write_u8(*slot)?;
            buf.write_i32(*user_id)?;
            buf.write_string(Some(userinfo))?;
        }
        SvcMessage::SetInfo { slot, key, value } => {
            buf.write_u8(Svc::SetInfo as u8)?;
            buf.write_u8(*slot)?;
            buf.write_string(Some(key))?;
            buf.write_string(Some(value))?;
        }
        SvcMessage::ServerInfo { key, value } => {
            buf.write_u8(Svc::ServerInfo as u8)?;
            buf.write_string(Some(key))?;
            buf.write_string(Some(value))?;
        }
        SvcMessage::PlayerInfo(info) => {
            buf.write_u8(Svc::PlayerInfo as u8)?;
            buf.write_u8(info.num)?;
            buf.write_u16(info.flags)?;
            buf.write_coord(info.origin.x)?;
            buf.write_coord(info.origin.y)?;
            buf.write_coord(info.origin.z)?;
            buf.write_u8(info.frame)?;

            if info.flags as u32 & PF_MSEC != 0 {
                buf.write_u8(info.msec.unwrap_or(0))?;
            }
            if info.flags as u32 & PF_COMMAND != 0 {
                let base = UserCmd::default();
                let cmd = info.command.as_ref().unwrap_or(&base);
                buf.write_delta_usercmd(&base, cmd)?;
            }

            let vel_flags = [PF_VELOCITY1, PF_VELOCITY2, PF_VELOCITY3];
            for (i, flag) in vel_flags.iter().enumerate() {
                if info.flags as u32 & flag != 0 {
                    buf.write_i16(info.velocity[i])?;
                }
            }

            if info.flags as u32 & PF_MODEL != 0 {
                buf.write_u8(info.model_index.unwrap_or(0))?;
            }
            if info.flags as u32 & PF_SKINNUM != 0 {
                buf.write_u8(info.skin_num.unwrap_or(0))?;
            }
            if info.flags as u32 & PF_EFFECTS != 0 {
                buf.write_u8(info.effects.unwrap_or(0))?;
            }
            if info.flags as u32 & PF_WEAPONFRAME != 0 {
                buf.write_u8(info.weapon_frame.unwrap_or(0))?;
            }
        }
        SvcMessage::SpawnBaseline { entity, baseline } => {
            buf.write_u8(Svc::SpawnBaseline as u8)?;
            buf.write_u16(*entity)?;
            write_baseline(buf, baseline)?;
        }
        SvcMessage::PacketEntities(update) => {
            if let Some(from) = update.delta_from {
                buf.write_u8(Svc::DeltaPacketEntities as u8)?;
                buf.write_u8(from)?;
            } else {
                buf.write_u8(Svc::PacketEntities as u8)?;
            }
            for entity in &update.entities {
                write_entity_delta(buf, entity)?;
            }
            buf.write_u16(0)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client_messages::MoveVars;
    use crate::protocol::PROTOCOL_VERSION;

    #[test]
    fn parses_print_message() {
        let mut buf = SizeBuf::new(64);
        buf.write_u8(Svc::Print as u8).unwrap();
        buf.write_u8(2).unwrap();
        buf.write_string(Some("hello")).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(
            msg,
            SvcMessage::Print {
                level: 2,
                message: "hello".to_string()
            }
        );
    }

    #[test]
    fn parses_serverdata_message() {
        let serverdata = ServerData {
            protocol: PROTOCOL_VERSION,
            server_count: 1,
            game_dir: "id1".to_string(),
            player_num: 1,
            spectator: false,
            level_name: "dm3".to_string(),
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
        write_svc_message(&mut buf, &SvcMessage::ServerData(serverdata.clone())).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(msg, SvcMessage::ServerData(serverdata));
    }

    #[test]
    fn parses_stream() {
        let mut buf = SizeBuf::new(128);
        write_svc_message(
            &mut buf,
            &SvcMessage::Print {
                level: 1,
                message: "one".to_string(),
            },
        )
        .unwrap();
        write_svc_message(
            &mut buf,
            &SvcMessage::Print {
                level: 2,
                message: "two".to_string(),
            },
        )
        .unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let messages = parse_svc_stream(&mut reader).unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn parses_update_userinfo() {
        let mut buf = SizeBuf::new(128);
        buf.write_u8(Svc::UpdateUserInfo as u8).unwrap();
        buf.write_u8(3).unwrap();
        buf.write_i32(1234).unwrap();
        buf.write_string(Some("\\name\\player")).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(
            msg,
            SvcMessage::UpdateUserInfo {
                slot: 3,
                user_id: 1234,
                userinfo: "\\name\\player".to_string()
            }
        );
    }

    #[test]
    fn parses_update_frags() {
        let mut buf = SizeBuf::new(64);
        write_svc_message(&mut buf, &SvcMessage::UpdateFrags { slot: 4, frags: 15 }).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(msg, SvcMessage::UpdateFrags { slot: 4, frags: 15 });
    }

    #[test]
    fn parses_setview_and_setangle() {
        let quantize_angle8 = |value: f32| {
            let scaled = ((value * 256.0 / 360.0) as i32) & 0xFF;
            let stored = scaled as i8;
            stored as f32 * (360.0 / 256.0)
        };
        let mut buf = SizeBuf::new(64);
        write_svc_message(&mut buf, &SvcMessage::SetView { entity: 12 }).unwrap();
        write_svc_message(&mut buf, &SvcMessage::SetAngle(Vec3::new(1.0, 2.0, 3.0))).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(msg, SvcMessage::SetView { entity: 12 });
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(
            msg,
            SvcMessage::SetAngle(Vec3::new(
                quantize_angle8(1.0),
                quantize_angle8(2.0),
                quantize_angle8(3.0)
            ))
        );
    }

    #[test]
    fn parses_clientdata() {
        let data = ClientDataMessage {
            bits: SU_VIEWHEIGHT
                | SU_IDEALPITCH
                | SU_PUNCH1
                | SU_VELOCITY1
                | SU_ONGROUND
                | SU_WEAPONFRAME
                | SU_ARMOR
                | SU_WEAPON,
            view_height: 20,
            ideal_pitch: -5,
            punch_angle: Vec3::new(4.0, 0.0, 0.0),
            velocity: Vec3::new(32.0, 0.0, 0.0),
            items: 123,
            onground: true,
            inwater: false,
            weapon_frame: 2,
            armor: 50,
            weapon: 3,
            health: 75,
            ammo: 4,
            ammo_counts: [5, 6, 7, 8],
            active_weapon: 2,
        };
        let mut buf = SizeBuf::new(128);
        write_svc_message(&mut buf, &SvcMessage::ClientData(data.clone())).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(msg, SvcMessage::ClientData(data));
    }

    #[test]
    fn parses_lightstyle() {
        let mut buf = SizeBuf::new(64);
        write_svc_message(
            &mut buf,
            &SvcMessage::LightStyle {
                style: 2,
                value: "abc".to_string(),
            },
        )
        .unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(
            msg,
            SvcMessage::LightStyle {
                style: 2,
                value: "abc".to_string()
            }
        );
    }

    #[test]
    fn parses_sound_message_defaults() {
        let sound = SoundMessage {
            entity: 5,
            channel: 2,
            sound_num: 7,
            volume: DEFAULT_SOUND_PACKET_VOLUME,
            attenuation: DEFAULT_SOUND_PACKET_ATTENUATION,
            origin: Vec3::new(1.0, 2.0, 3.0),
        };
        let mut buf = SizeBuf::new(64);
        write_svc_message(&mut buf, &SvcMessage::Sound(sound.clone())).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(msg, SvcMessage::Sound(sound));
    }

    #[test]
    fn parses_download_chunk() {
        let chunk = SvcMessage::Download {
            size: 2,
            percent: 50,
            data: vec![1, 2],
        };
        let mut buf = SizeBuf::new(32);
        write_svc_message(&mut buf, &chunk).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(msg, chunk);
    }

    #[test]
    fn parses_nail_projectiles() {
        let projectiles = vec![NailProjectile {
            origin: Vec3::new(0.0, 0.0, 0.0),
            angles: Vec3::new(90.0, 90.0, 0.0),
        }];
        let message = SvcMessage::Nails {
            projectiles: projectiles.clone(),
        };
        let mut buf = SizeBuf::new(32);
        write_svc_message(&mut buf, &message).unwrap();

        let expected = [Svc::Nails as u8, 1, 0x00, 0x08, 0x80, 0x00, 0x48, 0x40];
        assert_eq!(buf.as_slice(), &expected);

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(msg, message);
    }

    #[test]
    fn parses_update_ping() {
        let mut buf = SizeBuf::new(64);
        write_svc_message(&mut buf, &SvcMessage::UpdatePing { slot: 7, ping: 123 }).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(msg, SvcMessage::UpdatePing { slot: 7, ping: 123 });
    }

    #[test]
    fn parses_update_pl() {
        let mut buf = SizeBuf::new(64);
        write_svc_message(
            &mut buf,
            &SvcMessage::UpdatePl {
                slot: 2,
                packet_loss: 9,
            },
        )
        .unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(
            msg,
            SvcMessage::UpdatePl {
                slot: 2,
                packet_loss: 9
            }
        );
    }

    #[test]
    fn parses_update_enter_time() {
        let mut buf = SizeBuf::new(64);
        write_svc_message(
            &mut buf,
            &SvcMessage::UpdateEnterTime {
                slot: 1,
                seconds_ago: 2.5,
            },
        )
        .unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(
            msg,
            SvcMessage::UpdateEnterTime {
                slot: 1,
                seconds_ago: 2.5
            }
        );
    }

    #[test]
    fn parses_maxspeed() {
        let mut buf = SizeBuf::new(32);
        write_svc_message(&mut buf, &SvcMessage::MaxSpeed(420.0)).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(msg, SvcMessage::MaxSpeed(420.0));
    }

    #[test]
    fn parses_entgravity() {
        let mut buf = SizeBuf::new(32);
        write_svc_message(&mut buf, &SvcMessage::EntGravity(0.75)).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(msg, SvcMessage::EntGravity(0.75));
    }

    #[test]
    fn parses_temp_entity_gunshot() {
        let temp = TempEntityMessage {
            kind: TE_GUNSHOT,
            origin: Some(Vec3::new(1.0, 2.0, 3.0)),
            start: None,
            end: None,
            count: Some(4),
            entity: None,
        };
        let mut buf = SizeBuf::new(64);
        write_svc_message(&mut buf, &SvcMessage::TempEntity(temp.clone())).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(msg, SvcMessage::TempEntity(temp));
    }

    #[test]
    fn parses_temp_entity_lightning() {
        let temp = TempEntityMessage {
            kind: TE_LIGHTNING1,
            origin: None,
            start: Some(Vec3::new(0.0, 1.0, 2.0)),
            end: Some(Vec3::new(3.0, 4.0, 5.0)),
            count: None,
            entity: Some(12),
        };
        let mut buf = SizeBuf::new(64);
        write_svc_message(&mut buf, &SvcMessage::TempEntity(temp.clone())).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(msg, SvcMessage::TempEntity(temp));
    }

    #[test]
    fn parses_kick_messages() {
        let mut buf = SizeBuf::new(8);
        write_svc_message(&mut buf, &SvcMessage::SmallKick).unwrap();
        write_svc_message(&mut buf, &SvcMessage::BigKick).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        assert_eq!(
            parse_svc_message(&mut reader).unwrap(),
            SvcMessage::SmallKick
        );
        assert_eq!(parse_svc_message(&mut reader).unwrap(), SvcMessage::BigKick);
    }

    #[test]
    fn parses_time_and_version() {
        let mut buf = SizeBuf::new(16);
        write_svc_message(&mut buf, &SvcMessage::Version(28)).unwrap();
        write_svc_message(&mut buf, &SvcMessage::Time(12.5)).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        assert_eq!(
            parse_svc_message(&mut reader).unwrap(),
            SvcMessage::Version(28)
        );
        assert_eq!(
            parse_svc_message(&mut reader).unwrap(),
            SvcMessage::Time(12.5)
        );
    }

    #[test]
    fn parses_update_name_and_colors() {
        let mut buf = SizeBuf::new(32);
        write_svc_message(
            &mut buf,
            &SvcMessage::UpdateName {
                slot: 2,
                name: "unit".to_string(),
            },
        )
        .unwrap();
        write_svc_message(
            &mut buf,
            &SvcMessage::UpdateColors {
                slot: 2,
                colors: 0x3f,
            },
        )
        .unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        assert_eq!(
            parse_svc_message(&mut reader).unwrap(),
            SvcMessage::UpdateName {
                slot: 2,
                name: "unit".to_string()
            }
        );
        assert_eq!(
            parse_svc_message(&mut reader).unwrap(),
            SvcMessage::UpdateColors {
                slot: 2,
                colors: 0x3f
            }
        );
    }

    #[test]
    fn parses_signon_and_particle() {
        let effect = ParticleEffect {
            origin: Vec3::new(1.0, 2.0, 3.0),
            direction: Vec3::new(0.25, -0.5, 0.75),
            count: 12,
            color: 7,
        };
        let mut buf = SizeBuf::new(64);
        write_svc_message(&mut buf, &SvcMessage::SignonNum(3)).unwrap();
        write_svc_message(&mut buf, &SvcMessage::Particle(effect.clone())).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        assert_eq!(
            parse_svc_message(&mut reader).unwrap(),
            SvcMessage::SignonNum(3)
        );
        assert_eq!(
            parse_svc_message(&mut reader).unwrap(),
            SvcMessage::Particle(effect)
        );
    }

    #[test]
    fn caps_packetentities_count() {
        let mut entities = Vec::new();
        for i in 0..(crate::protocol::MAX_PACKET_ENTITIES + 4) {
            entities.push(EntityDelta {
                number: (i + 1) as u16,
                remove: false,
                flags: 0,
                model_index: None,
                frame: None,
                colormap: None,
                skin_num: None,
                effects: None,
                origin: [None, None, None],
                angles: [None, None, None],
                solid: false,
            });
        }
        let update = PacketEntitiesUpdate {
            delta_from: None,
            entities,
        };

        let mut buf = SizeBuf::new(4096);
        write_svc_message(&mut buf, &SvcMessage::PacketEntities(update)).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        match msg {
            SvcMessage::PacketEntities(update) => {
                assert_eq!(update.entities.len(), crate::protocol::MAX_PACKET_ENTITIES);
            }
            _ => panic!("expected packetentities"),
        }
    }

    #[test]
    fn parses_setinfo() {
        let mut buf = SizeBuf::new(128);
        buf.write_u8(Svc::SetInfo as u8).unwrap();
        buf.write_u8(1).unwrap();
        buf.write_string(Some("team")).unwrap();
        buf.write_string(Some("red")).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(
            msg,
            SvcMessage::SetInfo {
                slot: 1,
                key: "team".to_string(),
                value: "red".to_string()
            }
        );
    }

    #[test]
    fn parses_serverinfo() {
        let mut buf = SizeBuf::new(128);
        buf.write_u8(Svc::ServerInfo as u8).unwrap();
        buf.write_string(Some("hostname")).unwrap();
        buf.write_string(Some("server")).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(
            msg,
            SvcMessage::ServerInfo {
                key: "hostname".to_string(),
                value: "server".to_string()
            }
        );
    }

    #[test]
    fn parses_playerinfo() {
        let flags = (PF_MSEC | PF_VELOCITY1 | PF_MODEL | PF_WEAPONFRAME) as u16;
        let info = PlayerInfoMessage {
            num: 5,
            flags,
            origin: Vec3::new(1.0, 2.0, 3.0),
            frame: 7,
            msec: Some(15),
            command: None,
            velocity: [100, 0, 0],
            model_index: Some(2),
            skin_num: None,
            effects: None,
            weapon_frame: Some(3),
        };

        let mut buf = SizeBuf::new(256);
        write_svc_message(&mut buf, &SvcMessage::PlayerInfo(info.clone())).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        match msg {
            SvcMessage::PlayerInfo(parsed) => {
                assert_eq!(parsed.num, info.num);
                assert_eq!(parsed.flags, info.flags);
                assert_eq!(parsed.frame, info.frame);
                assert_eq!(parsed.msec, info.msec);
                assert_eq!(parsed.velocity[0], 100);
                assert_eq!(parsed.model_index, info.model_index);
                assert_eq!(parsed.weapon_frame, info.weapon_frame);
            }
            _ => panic!("expected playerinfo"),
        }
    }

    #[test]
    fn parses_spawnbaseline() {
        let baseline = EntityState {
            number: 0,
            flags: 0,
            origin: Vec3::new(1.0, 2.0, 3.0),
            angles: Vec3::new(10.0, 20.0, 30.0),
            modelindex: 2,
            frame: 3,
            colormap: 4,
            skinnum: 5,
            effects: 0,
        };

        let mut buf = SizeBuf::new(128);
        write_svc_message(
            &mut buf,
            &SvcMessage::SpawnBaseline {
                entity: 7,
                baseline,
            },
        )
        .unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        match msg {
            SvcMessage::SpawnBaseline { entity, baseline } => {
                assert_eq!(entity, 7);
                assert_eq!(baseline.modelindex, 2);
                assert_eq!(baseline.origin, Vec3::new(1.0, 2.0, 3.0));
            }
            _ => panic!("expected spawnbaseline"),
        }
    }

    #[test]
    fn parses_packetentities_round_trip() {
        let quantize_angle = |value: f32| {
            let scaled = ((value * 256.0 / 360.0) as i32) & 0xFF;
            (scaled as f32) * 360.0 / 256.0
        };
        let delta = EntityDelta {
            number: 1,
            remove: false,
            flags: 0,
            model_index: Some(2),
            frame: Some(3),
            colormap: Some(4),
            skin_num: None,
            effects: Some(6),
            origin: [Some(1.0), None, Some(3.0)],
            angles: [Some(quantize_angle(10.0)), Some(quantize_angle(20.0)), None],
            solid: false,
        };
        let update = PacketEntitiesUpdate {
            delta_from: None,
            entities: vec![EntityDelta {
                flags: delta.compute_flags(),
                ..delta
            }],
        };

        let mut buf = SizeBuf::new(256);
        write_svc_message(&mut buf, &SvcMessage::PacketEntities(update.clone())).unwrap();

        let mut reader = MsgReader::new(buf.as_slice());
        let msg = parse_svc_message(&mut reader).unwrap();
        assert_eq!(msg, SvcMessage::PacketEntities(update));
    }
}
