// QuakeWorld protocol constants and message identifiers.

pub const PROTOCOL_VERSION: i32 = 28;
pub const QW_CHECK_HASH: u16 = 0x5157;

pub const PORT_CLIENT: u16 = 27001;
pub const PORT_MASTER: u16 = 27000;
pub const PORT_SERVER: u16 = 27500;

pub const S2C_CHALLENGE: u8 = b'c';
pub const S2C_CONNECTION: u8 = b'j';
pub const A2A_PING: u8 = b'k';
pub const A2A_ACK: u8 = b'l';
pub const A2A_NACK: u8 = b'm';
pub const A2A_ECHO: u8 = b'e';
pub const A2C_PRINT: u8 = b'n';
pub const S2M_HEARTBEAT: u8 = b'a';
pub const A2C_CLIENT_COMMAND: u8 = b'B';
pub const S2M_SHUTDOWN: u8 = b'C';

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Svc {
    Bad = 0,
    Nop = 1,
    Disconnect = 2,
    UpdateStat = 3,
    Version = 4,
    SetView = 5,
    Sound = 6,
    Time = 7,
    Print = 8,
    StuffText = 9,
    SetAngle = 10,
    ServerData = 11,
    LightStyle = 12,
    UpdateName = 13,
    UpdateFrags = 14,
    ClientData = 15,
    StopSound = 16,
    UpdateColors = 17,
    Particle = 18,
    Damage = 19,
    SpawnStatic = 20,
    SpawnBaseline = 22,
    TempEntity = 23,
    SetPause = 24,
    SignonNum = 25,
    CenterPrint = 26,
    KilledMonster = 27,
    FoundSecret = 28,
    SpawnStaticSound = 29,
    Intermission = 30,
    Finale = 31,
    CdTrack = 32,
    SellScreen = 33,
    SmallKick = 34,
    BigKick = 35,
    UpdatePing = 36,
    UpdateEnterTime = 37,
    UpdateStatLong = 38,
    MuzzleFlash = 39,
    UpdateUserInfo = 40,
    Download = 41,
    PlayerInfo = 42,
    Nails = 43,
    ChokeCount = 44,
    ModelList = 45,
    SoundList = 46,
    PacketEntities = 47,
    DeltaPacketEntities = 48,
    MaxSpeed = 49,
    EntGravity = 50,
    SetInfo = 51,
    ServerInfo = 52,
    UpdatePl = 53,
}

impl TryFrom<u8> for Svc {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Svc::Bad),
            1 => Ok(Svc::Nop),
            2 => Ok(Svc::Disconnect),
            3 => Ok(Svc::UpdateStat),
            4 => Ok(Svc::Version),
            5 => Ok(Svc::SetView),
            6 => Ok(Svc::Sound),
            7 => Ok(Svc::Time),
            8 => Ok(Svc::Print),
            9 => Ok(Svc::StuffText),
            10 => Ok(Svc::SetAngle),
            11 => Ok(Svc::ServerData),
            12 => Ok(Svc::LightStyle),
            13 => Ok(Svc::UpdateName),
            14 => Ok(Svc::UpdateFrags),
            15 => Ok(Svc::ClientData),
            16 => Ok(Svc::StopSound),
            17 => Ok(Svc::UpdateColors),
            18 => Ok(Svc::Particle),
            19 => Ok(Svc::Damage),
            20 => Ok(Svc::SpawnStatic),
            22 => Ok(Svc::SpawnBaseline),
            23 => Ok(Svc::TempEntity),
            24 => Ok(Svc::SetPause),
            25 => Ok(Svc::SignonNum),
            26 => Ok(Svc::CenterPrint),
            27 => Ok(Svc::KilledMonster),
            28 => Ok(Svc::FoundSecret),
            29 => Ok(Svc::SpawnStaticSound),
            30 => Ok(Svc::Intermission),
            31 => Ok(Svc::Finale),
            32 => Ok(Svc::CdTrack),
            33 => Ok(Svc::SellScreen),
            34 => Ok(Svc::SmallKick),
            35 => Ok(Svc::BigKick),
            36 => Ok(Svc::UpdatePing),
            37 => Ok(Svc::UpdateEnterTime),
            38 => Ok(Svc::UpdateStatLong),
            39 => Ok(Svc::MuzzleFlash),
            40 => Ok(Svc::UpdateUserInfo),
            41 => Ok(Svc::Download),
            42 => Ok(Svc::PlayerInfo),
            43 => Ok(Svc::Nails),
            44 => Ok(Svc::ChokeCount),
            45 => Ok(Svc::ModelList),
            46 => Ok(Svc::SoundList),
            47 => Ok(Svc::PacketEntities),
            48 => Ok(Svc::DeltaPacketEntities),
            49 => Ok(Svc::MaxSpeed),
            50 => Ok(Svc::EntGravity),
            51 => Ok(Svc::SetInfo),
            52 => Ok(Svc::ServerInfo),
            53 => Ok(Svc::UpdatePl),
            _ => Err(value),
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Clc {
    Bad = 0,
    Nop = 1,
    Move = 3,
    StringCmd = 4,
    Delta = 5,
    TMove = 6,
    Upload = 7,
}

impl TryFrom<u8> for Clc {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Clc::Bad),
            1 => Ok(Clc::Nop),
            3 => Ok(Clc::Move),
            4 => Ok(Clc::StringCmd),
            5 => Ok(Clc::Delta),
            6 => Ok(Clc::TMove),
            7 => Ok(Clc::Upload),
            _ => Err(value),
        }
    }
}

pub const PF_MSEC: u32 = 1 << 0;
pub const PF_COMMAND: u32 = 1 << 1;
pub const PF_VELOCITY1: u32 = 1 << 2;
pub const PF_VELOCITY2: u32 = 1 << 3;
pub const PF_VELOCITY3: u32 = 1 << 4;
pub const PF_MODEL: u32 = 1 << 5;
pub const PF_SKINNUM: u32 = 1 << 6;
pub const PF_EFFECTS: u32 = 1 << 7;
pub const PF_WEAPONFRAME: u32 = 1 << 8;
pub const PF_DEAD: u32 = 1 << 9;
pub const PF_GIB: u32 = 1 << 10;
pub const PF_NOGRAV: u32 = 1 << 11;

pub const CM_ANGLE1: u8 = 1 << 0;
pub const CM_ANGLE3: u8 = 1 << 1;
pub const CM_FORWARD: u8 = 1 << 2;
pub const CM_SIDE: u8 = 1 << 3;
pub const CM_UP: u8 = 1 << 4;
pub const CM_BUTTONS: u8 = 1 << 5;
pub const CM_IMPULSE: u8 = 1 << 6;
pub const CM_ANGLE2: u8 = 1 << 7;

pub const U_ORIGIN1: u16 = 1 << 9;
pub const U_ORIGIN2: u16 = 1 << 10;
pub const U_ORIGIN3: u16 = 1 << 11;
pub const U_ANGLE2: u16 = 1 << 12;
pub const U_FRAME: u16 = 1 << 13;
pub const U_REMOVE: u16 = 1 << 14;
pub const U_MOREBITS: u16 = 1 << 15;

pub const U_ANGLE1: u8 = 1 << 0;
pub const U_ANGLE3: u8 = 1 << 1;
pub const U_MODEL: u8 = 1 << 2;
pub const U_COLORMAP: u8 = 1 << 3;
pub const U_SKIN: u8 = 1 << 4;
pub const U_EFFECTS: u8 = 1 << 5;
pub const U_SOLID: u8 = 1 << 6;

pub const SND_VOLUME: u16 = 1 << 15;
pub const SND_ATTENUATION: u16 = 1 << 14;

pub const DEFAULT_SOUND_PACKET_VOLUME: u8 = 255;
pub const DEFAULT_SOUND_PACKET_ATTENUATION: f32 = 1.0;

pub const DEFAULT_VIEWHEIGHT: i8 = 22;

pub const SU_VIEWHEIGHT: u16 = 1 << 0;
pub const SU_IDEALPITCH: u16 = 1 << 1;
pub const SU_PUNCH1: u16 = 1 << 2;
pub const SU_PUNCH2: u16 = 1 << 3;
pub const SU_PUNCH3: u16 = 1 << 4;
pub const SU_VELOCITY1: u16 = 1 << 5;
pub const SU_VELOCITY2: u16 = 1 << 6;
pub const SU_VELOCITY3: u16 = 1 << 7;
pub const SU_ITEMS: u16 = 1 << 9;
pub const SU_ONGROUND: u16 = 1 << 10;
pub const SU_INWATER: u16 = 1 << 11;
pub const SU_WEAPONFRAME: u16 = 1 << 12;
pub const SU_ARMOR: u16 = 1 << 13;
pub const SU_WEAPON: u16 = 1 << 14;

pub const TE_SPIKE: u8 = 0;
pub const TE_SUPERSPIKE: u8 = 1;
pub const TE_GUNSHOT: u8 = 2;
pub const TE_EXPLOSION: u8 = 3;
pub const TE_TAREXPLOSION: u8 = 4;
pub const TE_LIGHTNING1: u8 = 5;
pub const TE_LIGHTNING2: u8 = 6;
pub const TE_WIZSPIKE: u8 = 7;
pub const TE_KNIGHTSPIKE: u8 = 8;
pub const TE_LIGHTNING3: u8 = 9;
pub const TE_LAVASPLASH: u8 = 10;
pub const TE_TELEPORT: u8 = 11;
pub const TE_BLOOD: u8 = 12;
pub const TE_LIGHTNINGBLOOD: u8 = 13;

pub const MAX_CLIENTS: usize = 32;
pub const UPDATE_BACKUP: usize = 64;
pub const UPDATE_MASK: usize = UPDATE_BACKUP - 1;

pub const MAX_PACKET_ENTITIES: usize = 64;
