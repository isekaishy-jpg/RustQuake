// Shared protocol-facing types.

use crate::protocol::{MAX_CLIENTS, MAX_PACKET_ENTITIES};

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

impl Default for Vec3 {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct UserCmd {
    pub msec: u8,
    pub angles: Vec3,
    pub forwardmove: i16,
    pub sidemove: i16,
    pub upmove: i16,
    pub buttons: u8,
    pub impulse: u8,
}

impl Default for UserCmd {
    fn default() -> Self {
        Self {
            msec: 0,
            angles: Vec3::default(),
            forwardmove: 0,
            sidemove: 0,
            upmove: 0,
            buttons: 0,
            impulse: 0,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct EntityState {
    pub number: i32,
    pub flags: i32,
    pub origin: Vec3,
    pub angles: Vec3,
    pub modelindex: i32,
    pub frame: i32,
    pub colormap: i32,
    pub skinnum: i32,
    pub effects: i32,
}

impl Default for EntityState {
    fn default() -> Self {
        Self {
            number: 0,
            flags: 0,
            origin: Vec3::default(),
            angles: Vec3::default(),
            modelindex: 0,
            frame: 0,
            colormap: 0,
            skinnum: 0,
            effects: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PacketEntities {
    pub num_entities: usize,
    pub entities: [EntityState; MAX_PACKET_ENTITIES],
}

impl Default for PacketEntities {
    fn default() -> Self {
        Self {
            num_entities: 0,
            entities: [EntityState::default(); MAX_PACKET_ENTITIES],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub cmd: UserCmd,
    pub senttime: f64,
    pub delta_sequence: i32,
    pub receivedtime: f64,
    pub playerstate: [PlayerState; MAX_CLIENTS],
    pub packet_entities: PacketEntities,
    pub invalid: bool,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            cmd: UserCmd::default(),
            senttime: 0.0,
            delta_sequence: -1,
            receivedtime: -1.0,
            playerstate: [PlayerState::default(); MAX_CLIENTS],
            packet_entities: PacketEntities::default(),
            invalid: false,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PlayerState {
    pub messagenum: i32,
    pub state_time: f64,
    pub command: UserCmd,
    pub origin: Vec3,
    pub viewangles: Vec3,
    pub velocity: Vec3,
    pub weaponframe: i32,
    pub modelindex: i32,
    pub frame: i32,
    pub skinnum: i32,
    pub effects: i32,
    pub flags: i32,
    pub waterjumptime: f32,
    pub onground: i32,
    pub oldbuttons: i32,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            messagenum: 0,
            state_time: 0.0,
            command: UserCmd::default(),
            origin: Vec3::default(),
            viewangles: Vec3::default(),
            velocity: Vec3::default(),
            weaponframe: 0,
            modelindex: 0,
            frame: 0,
            skinnum: 0,
            effects: 0,
            flags: 0,
            waterjumptime: 0.0,
            onground: 0,
            oldbuttons: 0,
        }
    }
}
