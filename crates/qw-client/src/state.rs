use qw_common::{
    EntityState, Frame, InfoString, MAX_CL_STATS, MAX_CLIENTS, MAX_EDICTS, MAX_INFO_STRING,
    MAX_LIGHTSTYLES, MAX_PACKET_ENTITIES, MAX_SERVERINFO_STRING, PacketEntities,
    PacketEntitiesUpdate, ServerData, StringListChunk, SvcMessage, STAT_MONSTERS, STAT_SECRETS,
    UPDATE_BACKUP, UPDATE_MASK, UserCmd, Vec3,
};

#[derive(Debug, Clone)]
pub struct PlayerInfo {
    pub user_id: i32,
    pub userinfo: InfoString,
    pub origin: Vec3,
    pub frame: u8,
    pub velocity: [i16; 3],
    pub model_index: u8,
    pub frags: i16,
    pub ping: i16,
    pub packet_loss: u8,
    pub enter_time: f32,
}

impl PlayerInfo {
    pub fn new() -> Self {
        Self {
            user_id: 0,
            userinfo: InfoString::new(MAX_INFO_STRING),
            origin: Vec3::default(),
            frame: 0,
            velocity: [0; 3],
            model_index: 0,
            frags: 0,
            ping: 0,
            packet_loss: 0,
            enter_time: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticSound {
    pub origin: Vec3,
    pub sound: u8,
    pub volume: u8,
    pub attenuation: u8,
}

#[derive(Debug)]
pub struct ClientState {
    pub serverinfo: InfoString,
    pub serverdata: Option<ServerData>,
    pub players: Vec<PlayerInfo>,
    pub sounds: Vec<String>,
    pub models: Vec<String>,
    pub next_sound: Option<u8>,
    pub next_model: Option<u8>,
    pub view_entity: Option<u16>,
    pub view_angles: Vec3,
    pub stats: [i32; MAX_CL_STATS],
    pub lightstyles: Vec<String>,
    pub baselines: Vec<EntityState>,
    pub frames: Vec<Frame>,
    pub valid_sequence: i32,
    pub server_time: f32,
    pub signon_num: Option<u8>,
    pub particle_effects: Vec<qw_common::ParticleEffect>,
    pub temp_entities: Vec<qw_common::TempEntityMessage>,
    pub static_entities: Vec<EntityState>,
    pub static_sounds: Vec<StaticSound>,
    pub intermission: Option<(Vec3, Vec3)>,
    pub finale: Option<String>,
    pub show_sellscreen: bool,
    pub kick_angle: f32,
    pub paused: bool,
}

impl ClientState {
    pub fn new() -> Self {
        let mut players = Vec::with_capacity(MAX_CLIENTS);
        for _ in 0..MAX_CLIENTS {
            players.push(PlayerInfo::new());
        }
        let baselines = vec![EntityState::default(); MAX_EDICTS];
        let frames = vec![Frame::default(); UPDATE_BACKUP];
        Self {
            serverinfo: InfoString::new(MAX_SERVERINFO_STRING),
            serverdata: None,
            players,
            sounds: Vec::new(),
            models: Vec::new(),
            next_sound: None,
            next_model: None,
            view_entity: None,
            view_angles: Vec3::default(),
            stats: [0; MAX_CL_STATS],
            lightstyles: vec![String::new(); MAX_LIGHTSTYLES],
            baselines,
            frames,
            valid_sequence: 0,
            server_time: 0.0,
            signon_num: None,
            particle_effects: Vec::new(),
            temp_entities: Vec::new(),
            static_entities: Vec::new(),
            static_sounds: Vec::new(),
            intermission: None,
            finale: None,
            show_sellscreen: false,
            kick_angle: 0.0,
            paused: false,
        }
    }

    pub fn apply_message(&mut self, msg: &SvcMessage, incoming_sequence: u32) {
        match msg {
            SvcMessage::ServerData(data) => {
                self.serverdata = Some(data.clone());
                self.sounds.clear();
                self.models.clear();
                self.next_sound = None;
                self.next_model = None;
                self.signon_num = None;
                self.particle_effects.clear();
                self.temp_entities.clear();
                self.static_entities.clear();
                self.static_sounds.clear();
                self.intermission = None;
                self.finale = None;
                self.show_sellscreen = false;
                self.kick_angle = 0.0;
            }
            SvcMessage::Time(value) => {
                self.server_time = *value;
            }
            SvcMessage::UpdateUserInfo {
                slot,
                user_id,
                userinfo,
            } => {
                if let Some(player) = self.players.get_mut(*slot as usize) {
                    player.user_id = *user_id;
                    player.userinfo.set_raw(userinfo);
                }
            }
            SvcMessage::SetInfo { slot, key, value } => {
                if let Some(player) = self.players.get_mut(*slot as usize) {
                    let _ = player.userinfo.set(key, value);
                }
            }
            SvcMessage::UpdateName { slot, name } => {
                if let Some(player) = self.players.get_mut(*slot as usize) {
                    let _ = player.userinfo.set("name", name);
                }
            }
            SvcMessage::UpdateColors { slot, colors } => {
                if let Some(player) = self.players.get_mut(*slot as usize) {
                    let top = colors >> 4;
                    let bottom = colors & 0x0f;
                    let _ = player.userinfo.set("topcolor", &top.to_string());
                    let _ = player.userinfo.set("bottomcolor", &bottom.to_string());
                }
            }
            SvcMessage::ServerInfo { key, value } => {
                let _ = self.serverinfo.set(key, value);
            }
            SvcMessage::UpdateFrags { slot, frags } => {
                if let Some(player) = self.players.get_mut(*slot as usize) {
                    player.frags = *frags;
                }
            }
            SvcMessage::UpdatePing { slot, ping } => {
                if let Some(player) = self.players.get_mut(*slot as usize) {
                    player.ping = *ping;
                }
            }
            SvcMessage::UpdatePl { slot, packet_loss } => {
                if let Some(player) = self.players.get_mut(*slot as usize) {
                    player.packet_loss = *packet_loss;
                }
            }
            SvcMessage::UpdateEnterTime { slot, seconds_ago } => {
                if let Some(player) = self.players.get_mut(*slot as usize) {
                    player.enter_time = *seconds_ago;
                }
            }
            SvcMessage::PlayerInfo(info) => {
                if let Some(player) = self.players.get_mut(info.num as usize) {
                    player.origin = info.origin;
                    player.frame = info.frame;
                    player.velocity = info.velocity;
                    if let Some(model_index) = info.model_index {
                        player.model_index = model_index;
                    }
                }
            }
            SvcMessage::SoundList(chunk) => {
                apply_string_list(&mut self.sounds, chunk);
                self.next_sound = if chunk.next == 0 { None } else { Some(chunk.next) };
            }
            SvcMessage::ModelList(chunk) => {
                apply_string_list(&mut self.models, chunk);
                self.next_model = if chunk.next == 0 { None } else { Some(chunk.next) };
            }
            SvcMessage::SetView { entity } => {
                self.view_entity = Some(*entity);
            }
            SvcMessage::SetAngle(angles) => {
                self.view_angles = *angles;
            }
            SvcMessage::SignonNum(value) => {
                self.signon_num = Some(*value);
            }
            SvcMessage::SetPause(paused) => {
                self.paused = *paused;
            }
            SvcMessage::SpawnStatic(entity) => {
                self.static_entities.push(*entity);
            }
            SvcMessage::SpawnStaticSound {
                origin,
                sound,
                volume,
                attenuation,
            } => {
                self.static_sounds.push(StaticSound {
                    origin: *origin,
                    sound: *sound,
                    volume: *volume,
                    attenuation: *attenuation,
                });
            }
            SvcMessage::Intermission { origin, angles } => {
                self.intermission = Some((*origin, *angles));
            }
            SvcMessage::Finale(text) => {
                self.finale = Some(text.clone());
            }
            SvcMessage::SellScreen => {
                self.show_sellscreen = true;
            }
            SvcMessage::SmallKick => {
                self.kick_angle = -2.0;
            }
            SvcMessage::BigKick => {
                self.kick_angle = -4.0;
            }
            SvcMessage::LightStyle { style, value } => {
                let index = *style as usize;
                if index < self.lightstyles.len() {
                    self.lightstyles[index] = value.clone();
                }
            }
            SvcMessage::UpdateStat { index, value } => {
                let idx = *index as usize;
                if idx < self.stats.len() {
                    self.stats[idx] = *value as i32;
                }
            }
            SvcMessage::UpdateStatLong { index, value } => {
                let idx = *index as usize;
                if idx < self.stats.len() {
                    self.stats[idx] = *value;
                }
            }
            SvcMessage::KilledMonster => {
                self.stats[STAT_MONSTERS] += 1;
            }
            SvcMessage::FoundSecret => {
                self.stats[STAT_SECRETS] += 1;
            }
            SvcMessage::MaxSpeed(value) => {
                if let Some(data) = &mut self.serverdata {
                    data.movevars.maxspeed = *value;
                }
            }
            SvcMessage::EntGravity(value) => {
                if let Some(data) = &mut self.serverdata {
                    data.movevars.entgravity = *value;
                }
            }
            SvcMessage::Particle(effect) => {
                self.particle_effects.push(effect.clone());
            }
            SvcMessage::TempEntity(temp) => {
                self.temp_entities.push(temp.clone());
            }
            SvcMessage::SpawnBaseline { entity, baseline } => {
                let index = *entity as usize;
                if index < self.baselines.len() {
                    let mut stored = *baseline;
                    stored.number = *entity as i32;
                    self.baselines[index] = stored;
                }
            }
            SvcMessage::PacketEntities(update) => {
                self.apply_packet_entities(update, incoming_sequence);
            }
            _ => {}
        }
    }

    pub fn store_outgoing_cmd(&mut self, sequence: u32, cmd: UserCmd) {
        let index = (sequence as usize) & UPDATE_MASK;
        self.frames[index].cmd = cmd;
    }

    pub fn outgoing_cmd(&self, sequence: u32) -> UserCmd {
        let index = (sequence as usize) & UPDATE_MASK;
        self.frames[index].cmd
    }

    pub fn set_outgoing_delta_sequence(&mut self, sequence: u32, delta_sequence: i32) {
        let index = (sequence as usize) & UPDATE_MASK;
        self.frames[index].delta_sequence = delta_sequence;
    }

    fn apply_packet_entities(&mut self, update: &PacketEntitiesUpdate, incoming_sequence: u32) {
        let newpacket = (incoming_sequence as usize) & UPDATE_MASK;
        let full = update.delta_from.is_none();

        let oldp = if let Some(from) = update.delta_from {
            let oldpacket = resolve_delta_sequence(incoming_sequence, from);
            if incoming_sequence.wrapping_sub(oldpacket) >= (UPDATE_BACKUP as u32 - 1) {
                self.valid_sequence = 0;
                self.frames[newpacket].invalid = true;
                return;
            }
            self.valid_sequence = incoming_sequence as i32;
            self.frames[(oldpacket as usize) & UPDATE_MASK]
                .packet_entities
                .clone()
        } else {
            self.valid_sequence = incoming_sequence as i32;
            PacketEntities::default()
        };

        let mut newp = PacketEntities::default();
        let mut oldindex = 0usize;
        let mut newindex = 0usize;

        for delta in &update.entities {
            let newnum = delta.number as i32;
            let mut oldnum = if oldindex >= oldp.num_entities {
                9999
            } else {
                oldp.entities[oldindex].number
            };

            while newnum > oldnum {
                if full {
                    self.valid_sequence = 0;
                    self.frames[newpacket].invalid = true;
                    return;
                }
                if newindex >= MAX_PACKET_ENTITIES {
                    self.frames[newpacket].invalid = true;
                    return;
                }
                newp.entities[newindex] = oldp.entities[oldindex];
                newindex += 1;
                oldindex += 1;
                oldnum = if oldindex >= oldp.num_entities {
                    9999
                } else {
                    oldp.entities[oldindex].number
                };
            }

            if newnum < oldnum {
                if delta.remove {
                    if full {
                        self.valid_sequence = 0;
                        self.frames[newpacket].invalid = true;
                        return;
                    }
                    continue;
                }
                if newindex >= MAX_PACKET_ENTITIES {
                    self.frames[newpacket].invalid = true;
                    return;
                }
                let baseline = self
                    .baselines
                    .get(newnum as usize)
                    .copied()
                    .unwrap_or_default();
                newp.entities[newindex] = delta.apply_to(&baseline);
                newindex += 1;
                continue;
            }

            if newnum == oldnum {
                if delta.remove {
                    oldindex += 1;
                    continue;
                }
                if newindex >= MAX_PACKET_ENTITIES {
                    self.frames[newpacket].invalid = true;
                    return;
                }
                newp.entities[newindex] = delta.apply_to(&oldp.entities[oldindex]);
                newindex += 1;
                oldindex += 1;
            }
        }

        while oldindex < oldp.num_entities {
            if newindex >= MAX_PACKET_ENTITIES {
                self.frames[newpacket].invalid = true;
                return;
            }
            newp.entities[newindex] = oldp.entities[oldindex];
            newindex += 1;
            oldindex += 1;
        }

        newp.num_entities = newindex;
        self.frames[newpacket].packet_entities = newp;
        self.frames[newpacket].invalid = false;
    }
}

fn apply_string_list(target: &mut Vec<String>, chunk: &StringListChunk) {
    let start = chunk.start as usize;
    if target.len() < start {
        target.resize(start, String::new());
    }
    for (i, item) in chunk.items.iter().enumerate() {
        let index = start + i;
        if index < target.len() {
            target[index] = item.clone();
        } else {
            target.push(item.clone());
        }
    }
}

fn resolve_delta_sequence(incoming_sequence: u32, from: u8) -> u32 {
    let mask = UPDATE_MASK as u32;
    let mut seq = (incoming_sequence & !mask) | (from as u32 & mask);
    if seq > incoming_sequence {
        seq = seq.wrapping_sub(UPDATE_BACKUP as u32);
    }
    seq
}

#[cfg(test)]
mod tests {
    use super::*;
    use qw_common::SvcMessage;

    #[test]
    fn applies_userinfo_updates() {
        let mut state = ClientState::new();
        state.apply_message(
            &SvcMessage::UpdateUserInfo {
                slot: 1,
                user_id: 99,
                userinfo: "\\name\\player".to_string(),
            },
            0,
        );
        assert_eq!(state.players[1].user_id, 99);
        assert!(state.players[1].userinfo.as_str().contains("\\name\\player"));
    }

    #[test]
    fn applies_setinfo() {
        let mut state = ClientState::new();
        state.apply_message(
            &SvcMessage::SetInfo {
                slot: 2,
                key: "team".to_string(),
                value: "red".to_string(),
            },
            0,
        );
        assert!(state.players[2].userinfo.as_str().contains("\\team\\red"));
    }

    #[test]
    fn applies_update_name_and_colors() {
        let mut state = ClientState::new();
        state.apply_message(
            &SvcMessage::UpdateName {
                slot: 0,
                name: "unit".to_string(),
            },
            0,
        );
        state.apply_message(
            &SvcMessage::UpdateColors {
                slot: 0,
                colors: 0x3f,
            },
            0,
        );
        let info = state.players[0].userinfo.as_str();
        assert!(info.contains("\\name\\unit"));
        assert!(info.contains("\\topcolor\\3"));
        assert!(info.contains("\\bottomcolor\\15"));
    }

    #[test]
    fn applies_serverinfo() {
        let mut state = ClientState::new();
        state.apply_message(
            &SvcMessage::ServerInfo {
                key: "hostname".to_string(),
                value: "server".to_string(),
            },
            0,
        );
        assert!(state.serverinfo.as_str().contains("\\hostname\\server"));
    }

    #[test]
    fn applies_playerinfo() {
        let mut state = ClientState::new();
        state.apply_message(
            &SvcMessage::PlayerInfo(qw_common::PlayerInfoMessage {
                num: 0,
                flags: 0,
                origin: Vec3::new(1.0, 2.0, 3.0),
                frame: 4,
                msec: None,
                command: None,
                velocity: [10, 20, 30],
                model_index: Some(2),
                skin_num: None,
                effects: None,
                weapon_frame: None,
            }),
            0,
        );
        assert_eq!(state.players[0].origin, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(state.players[0].frame, 4);
        assert_eq!(state.players[0].velocity, [10, 20, 30]);
        assert_eq!(state.players[0].model_index, 2);
    }

    #[test]
    fn applies_packetentities_from_baseline() {
        let mut state = ClientState::new();
        let baseline = EntityState {
            number: 2,
            flags: 0,
            origin: Vec3::new(1.0, 2.0, 3.0),
            angles: Vec3::new(10.0, 20.0, 30.0),
            modelindex: 1,
            frame: 0,
            colormap: 0,
            skinnum: 0,
            effects: 0,
        };
        state.apply_message(
            &SvcMessage::SpawnBaseline {
                entity: 2,
                baseline,
            },
            0,
        );

        let delta = qw_common::EntityDelta {
            number: 2,
            remove: false,
            flags: 0,
            model_index: Some(3),
            frame: None,
            colormap: None,
            skin_num: None,
            effects: None,
            origin: [Some(4.0), None, None],
            angles: [None, None, None],
            solid: false,
        };
        let update = PacketEntitiesUpdate {
            delta_from: None,
            entities: vec![qw_common::EntityDelta {
                flags: delta.compute_flags(),
                ..delta
            }],
        };
        state.apply_message(&SvcMessage::PacketEntities(update), 1);

        let frame = &state.frames[1 & UPDATE_MASK];
        assert_eq!(frame.packet_entities.num_entities, 1);
        assert_eq!(frame.packet_entities.entities[0].modelindex, 3);
        assert_eq!(frame.packet_entities.entities[0].origin.x, 4.0);
    }

    #[test]
    fn applies_packetentities_delta_remove() {
        let mut state = ClientState::new();
        let baseline1 = EntityState {
            number: 1,
            flags: 0,
            origin: Vec3::new(1.0, 1.0, 1.0),
            angles: Vec3::default(),
            modelindex: 1,
            frame: 0,
            colormap: 0,
            skinnum: 0,
            effects: 0,
        };
        let baseline2 = EntityState {
            number: 2,
            flags: 0,
            origin: Vec3::new(2.0, 2.0, 2.0),
            angles: Vec3::default(),
            modelindex: 2,
            frame: 0,
            colormap: 0,
            skinnum: 0,
            effects: 0,
        };
        state.apply_message(
            &SvcMessage::SpawnBaseline {
                entity: 1,
                baseline: baseline1,
            },
            0,
        );
        state.apply_message(
            &SvcMessage::SpawnBaseline {
                entity: 2,
                baseline: baseline2,
            },
            0,
        );

        let full = PacketEntitiesUpdate {
            delta_from: None,
            entities: vec![
                qw_common::EntityDelta {
                    number: 1,
                    remove: false,
                    flags: 0,
                    model_index: Some(1),
                    frame: None,
                    colormap: None,
                    skin_num: None,
                    effects: None,
                    origin: [Some(1.0), None, None],
                    angles: [None, None, None],
                    solid: false,
                },
                qw_common::EntityDelta {
                    number: 2,
                    remove: false,
                    flags: 0,
                    model_index: Some(2),
                    frame: None,
                    colormap: None,
                    skin_num: None,
                    effects: None,
                    origin: [Some(2.0), None, None],
                    angles: [None, None, None],
                    solid: false,
                },
            ],
        };
        state.apply_message(&SvcMessage::PacketEntities(full), 1);

        let delta = PacketEntitiesUpdate {
            delta_from: Some(1),
            entities: vec![
                qw_common::EntityDelta {
                    number: 1,
                    remove: true,
                    flags: 0,
                    model_index: None,
                    frame: None,
                    colormap: None,
                    skin_num: None,
                    effects: None,
                    origin: [None, None, None],
                    angles: [None, None, None],
                    solid: false,
                },
                qw_common::EntityDelta {
                    number: 2,
                    remove: false,
                    flags: 0,
                    model_index: Some(4),
                    frame: None,
                    colormap: None,
                    skin_num: None,
                    effects: None,
                    origin: [None, None, None],
                    angles: [None, None, None],
                    solid: false,
                },
            ],
        };
        state.apply_message(&SvcMessage::PacketEntities(delta), 2);

        let frame = &state.frames[2 & UPDATE_MASK];
        assert_eq!(frame.packet_entities.num_entities, 1);
        assert_eq!(frame.packet_entities.entities[0].number, 2);
        assert_eq!(frame.packet_entities.entities[0].modelindex, 4);
    }

    #[test]
    fn applies_scoreboard_updates() {
        let mut state = ClientState::new();
        state.apply_message(
            &SvcMessage::UpdateFrags { slot: 0, frags: 5 },
            0,
        );
        state.apply_message(
            &SvcMessage::UpdatePing { slot: 0, ping: 50 },
            0,
        );
        state.apply_message(
            &SvcMessage::UpdatePl {
                slot: 0,
                packet_loss: 2,
            },
            0,
        );
        state.apply_message(
            &SvcMessage::UpdateEnterTime {
                slot: 0,
                seconds_ago: 3.5,
            },
            0,
        );

        let player = &state.players[0];
        assert_eq!(player.frags, 5);
        assert_eq!(player.ping, 50);
        assert_eq!(player.packet_loss, 2);
        assert_eq!(player.enter_time, 3.5);
    }

    #[test]
    fn applies_serverdata_and_lists() {
        let mut state = ClientState::new();
        let data = qw_common::ServerData {
            protocol: qw_common::PROTOCOL_VERSION,
            server_count: 9,
            game_dir: "id1".to_string(),
            player_num: 1,
            spectator: false,
            level_name: "dm2".to_string(),
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
        state.apply_message(&SvcMessage::ServerData(data.clone()), 0);
        assert_eq!(state.serverdata, Some(data));

        let sound_chunk = qw_common::StringListChunk {
            start: 0,
            items: vec!["sound1".to_string(), "sound2".to_string()],
            next: 2,
        };
        state.apply_message(&SvcMessage::SoundList(sound_chunk), 0);
        assert_eq!(state.sounds.len(), 2);
        assert_eq!(state.next_sound, Some(2));

        let model_chunk = qw_common::StringListChunk {
            start: 2,
            items: vec!["model3".to_string()],
            next: 0,
        };
        state.apply_message(&SvcMessage::ModelList(model_chunk), 0);
        assert_eq!(state.models.len(), 3);
        assert_eq!(state.models[2], "model3");
        assert_eq!(state.next_model, None);
    }

    #[test]
    fn applies_view_and_stats() {
        let mut state = ClientState::new();
        state.apply_message(&SvcMessage::SetView { entity: 12 }, 0);
        state.apply_message(&SvcMessage::SetAngle(Vec3::new(1.0, 2.0, 3.0)), 0);
        state.apply_message(
            &SvcMessage::LightStyle {
                style: 2,
                value: "abc".to_string(),
            },
            0,
        );
        state.apply_message(
            &SvcMessage::UpdateStat {
                index: 4,
                value: 11,
            },
            0,
        );
        state.apply_message(
            &SvcMessage::UpdateStatLong {
                index: 5,
                value: 1024,
            },
            0,
        );
        state.apply_message(&SvcMessage::KilledMonster, 0);
        state.apply_message(&SvcMessage::FoundSecret, 0);
        state.apply_message(&SvcMessage::SignonNum(2), 0);
        state.apply_message(&SvcMessage::Time(12.5), 0);
        state.apply_message(&SvcMessage::SetPause(true), 0);

        assert_eq!(state.view_entity, Some(12));
        assert_eq!(state.view_angles, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(state.lightstyles[2], "abc");
        assert_eq!(state.stats[4], 11);
        assert_eq!(state.stats[5], 1024);
        assert_eq!(state.stats[STAT_MONSTERS], 1);
        assert_eq!(state.stats[STAT_SECRETS], 1);
        assert_eq!(state.signon_num, Some(2));
        assert_eq!(state.server_time, 12.5);
        assert!(state.paused);
    }

    #[test]
    fn queues_temp_entities_and_particles() {
        let mut state = ClientState::new();
        let temp = qw_common::TempEntityMessage {
            kind: qw_common::TE_SPIKE,
            origin: Some(Vec3::new(1.0, 2.0, 3.0)),
            start: None,
            end: None,
            count: None,
            entity: None,
        };
        let particle = qw_common::ParticleEffect {
            origin: Vec3::new(0.0, 1.0, 2.0),
            direction: Vec3::new(0.0, 0.0, 1.0),
            count: 8,
            color: 5,
        };
        state.apply_message(&SvcMessage::TempEntity(temp.clone()), 0);
        state.apply_message(&SvcMessage::Particle(particle.clone()), 0);

        assert_eq!(state.temp_entities, vec![temp]);
        assert_eq!(state.particle_effects, vec![particle]);
    }

    #[test]
    fn applies_static_and_end_state_messages() {
        let mut state = ClientState::new();
        let entity = EntityState {
            number: 0,
            flags: 0,
            origin: Vec3::new(1.0, 2.0, 3.0),
            angles: Vec3::new(4.0, 5.0, 6.0),
            modelindex: 1,
            frame: 0,
            colormap: 0,
            skinnum: 0,
            effects: 0,
        };
        state.apply_message(&SvcMessage::SpawnStatic(entity), 0);
        state.apply_message(
            &SvcMessage::SpawnStaticSound {
                origin: Vec3::new(7.0, 8.0, 9.0),
                sound: 3,
                volume: 200,
                attenuation: 64,
            },
            0,
        );
        state.apply_message(
            &SvcMessage::Intermission {
                origin: Vec3::new(10.0, 11.0, 12.0),
                angles: Vec3::new(13.0, 14.0, 15.0),
            },
            0,
        );
        state.apply_message(&SvcMessage::Finale("end".to_string()), 0);
        state.apply_message(&SvcMessage::SellScreen, 0);
        state.apply_message(&SvcMessage::SmallKick, 0);

        assert_eq!(state.static_entities, vec![entity]);
        assert_eq!(
            state.static_sounds,
            vec![StaticSound {
                origin: Vec3::new(7.0, 8.0, 9.0),
                sound: 3,
                volume: 200,
                attenuation: 64,
            }]
        );
        assert_eq!(
            state.intermission,
            Some((Vec3::new(10.0, 11.0, 12.0), Vec3::new(13.0, 14.0, 15.0)))
        );
        assert_eq!(state.finale, Some("end".to_string()));
        assert!(state.show_sellscreen);
        assert_eq!(state.kick_angle, -2.0);
    }

    #[test]
    fn applies_movevar_updates() {
        let mut state = ClientState::new();
        let data = qw_common::ServerData {
            protocol: qw_common::PROTOCOL_VERSION,
            server_count: 1,
            game_dir: "id1".to_string(),
            player_num: 0,
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
        state.apply_message(&SvcMessage::ServerData(data), 0);
        state.apply_message(&SvcMessage::MaxSpeed(450.0), 0);
        state.apply_message(&SvcMessage::EntGravity(0.8), 0);

        let data = state.serverdata.unwrap();
        assert_eq!(data.movevars.maxspeed, 450.0);
        assert_eq!(data.movevars.entgravity, 0.8);
    }

    #[test]
    fn resolves_delta_sequence_across_wrap() {
        let mut state = ClientState::new();
        let mut oldp = PacketEntities::default();
        oldp.num_entities = 1;
        oldp.entities[0].number = 7;
        oldp.entities[0].modelindex = 1;
        state.frames[1].packet_entities = oldp;

        let update = PacketEntitiesUpdate {
            delta_from: Some(65),
            entities: Vec::new(),
        };
        state.apply_message(&SvcMessage::PacketEntities(update), 130);

        let frame = &state.frames[130 & UPDATE_MASK];
        assert_eq!(frame.packet_entities.num_entities, 1);
        assert_eq!(frame.packet_entities.entities[0].number, 7);
    }
}
