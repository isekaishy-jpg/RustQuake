// Shared constants and types from QuakeWorld headers.

pub type Byte = u8;
pub type QBool = bool;

pub const GLQUAKE_VERSION: f32 = 1.00;
pub const VERSION: f32 = 2.40;
pub const LINUX_VERSION: f32 = 0.98;

pub const MAX_SCOREBOARD: usize = 16;
pub const SOUND_CHANNELS: usize = 8;

pub const MAX_QPATH: usize = 64;
pub const MAX_OSPATH: usize = 128;

pub const MAX_INFO_STRING: usize = 196;
pub const MAX_SERVERINFO_STRING: usize = 512;
pub const MAX_LOCALINFO_STRING: usize = 32768;

pub const MAX_MSGLEN: usize = 1450;
pub const MAX_DATAGRAM: usize = 1450;

pub const MAX_EDICTS: usize = 768;
pub const MAX_LIGHTSTYLES: usize = 64;
pub const MAX_MODELS: usize = 256;
pub const MAX_SOUNDS: usize = 256;

pub const MAX_CL_STATS: usize = 32;
pub const STAT_HEALTH: usize = 0;
pub const STAT_WEAPON: usize = 2;
pub const STAT_AMMO: usize = 3;
pub const STAT_ARMOR: usize = 4;
pub const STAT_SHELLS: usize = 6;
pub const STAT_NAILS: usize = 7;
pub const STAT_ROCKETS: usize = 8;
pub const STAT_CELLS: usize = 9;
pub const STAT_ACTIVEWEAPON: usize = 10;
pub const STAT_TOTALSECRETS: usize = 11;
pub const STAT_TOTALMONSTERS: usize = 12;
pub const STAT_SECRETS: usize = 13;
pub const STAT_MONSTERS: usize = 14;
pub const STAT_ITEMS: usize = 15;

pub const IT_SHOTGUN: u32 = 1;
pub const IT_SUPER_SHOTGUN: u32 = 2;
pub const IT_NAILGUN: u32 = 4;
pub const IT_SUPER_NAILGUN: u32 = 8;
pub const IT_GRENADE_LAUNCHER: u32 = 16;
pub const IT_ROCKET_LAUNCHER: u32 = 32;
pub const IT_LIGHTNING: u32 = 64;
pub const IT_SUPER_LIGHTNING: u32 = 128;
pub const IT_SHELLS: u32 = 256;
pub const IT_NAILS: u32 = 512;
pub const IT_ROCKETS: u32 = 1024;
pub const IT_CELLS: u32 = 2048;
pub const IT_AXE: u32 = 4096;
pub const IT_ARMOR1: u32 = 8192;
pub const IT_ARMOR2: u32 = 16384;
pub const IT_ARMOR3: u32 = 32768;
pub const IT_SUPERHEALTH: u32 = 65536;
pub const IT_KEY1: u32 = 131072;
pub const IT_KEY2: u32 = 262144;
pub const IT_INVISIBILITY: u32 = 524288;
pub const IT_INVULNERABILITY: u32 = 1_048_576;
pub const IT_SUIT: u32 = 2_097_152;
pub const IT_QUAD: u32 = 4_194_304;
pub const IT_SIGIL1: u32 = 1 << 28;
pub const IT_SIGIL2: u32 = 1 << 29;
pub const IT_SIGIL3: u32 = 1 << 30;
pub const IT_SIGIL4: u32 = 1 << 31;

pub const CONTENTS_EMPTY: i32 = -1;
pub const CONTENTS_SOLID: i32 = -2;
pub const CONTENTS_WATER: i32 = -3;
pub const CONTENTS_SLIME: i32 = -4;
pub const CONTENTS_LAVA: i32 = -5;
pub const CONTENTS_SKY: i32 = -6;

pub const PRINT_LOW: u8 = 0;
pub const PRINT_MEDIUM: u8 = 1;
pub const PRINT_HIGH: u8 = 2;
pub const PRINT_CHAT: u8 = 3;
