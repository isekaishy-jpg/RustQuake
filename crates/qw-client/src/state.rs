use crate::prediction::{PhysEnt, PlayerMove};
use qw_common::{
    BspCollision, ClientDataMessage, EntityState, Frame, InfoString, MAX_CL_STATS, MAX_CLIENTS,
    MAX_EDICTS, MAX_INFO_STRING, MAX_LIGHTSTYLES, MAX_PACKET_ENTITIES, MAX_SERVERINFO_STRING,
    NailProjectile, PacketEntities, PacketEntitiesUpdate, PlayerState, STAT_ACTIVEWEAPON,
    STAT_AMMO, STAT_ARMOR, STAT_CELLS, STAT_HEALTH, STAT_ITEMS, STAT_MONSTERS, STAT_NAILS,
    STAT_ROCKETS, STAT_SECRETS, STAT_SHELLS, STAT_WEAPON, ServerData, StringListChunk, SvcMessage,
    UPDATE_BACKUP, UPDATE_MASK, UserCmd, Vec3,
};

#[derive(Debug, Clone)]
pub struct PlayerInfo {
    pub user_id: i32,
    pub userinfo: InfoString,
    pub origin: Vec3,
    pub frame: u8,
    pub msec: u8,
    pub cmd: UserCmd,
    pub velocity: [i16; 3],
    pub model_index: u8,
    pub skin_num: u8,
    pub effects: u8,
    pub weapon_frame: u8,
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
            msec: 0,
            cmd: UserCmd::default(),
            velocity: [0; 3],
            model_index: 0,
            skin_num: 0,
            effects: 0,
            weapon_frame: 0,
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

#[derive(Debug, Clone, PartialEq)]
pub struct StopSoundEvent {
    pub entity: u16,
    pub channel: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DamageEvent {
    pub armor: u8,
    pub blood: u8,
    pub origin: Vec3,
}

#[derive(Debug)]
pub struct ClientState {
    pub serverinfo: InfoString,
    pub serverdata: Option<ServerData>,
    pub collision: Option<BspCollision>,
    pub collision_map: Option<String>,
    pub players: Vec<PlayerInfo>,
    pub sounds: Vec<String>,
    pub models: Vec<String>,
    pub client_data: Option<ClientDataMessage>,
    pub sound_events: Vec<qw_common::SoundMessage>,
    pub stop_sounds: Vec<StopSoundEvent>,
    pub muzzle_flashes: Vec<u16>,
    pub damage_events: Vec<DamageEvent>,
    pub prints: Vec<(u8, String)>,
    pub center_prints: Vec<String>,
    pub next_sound: Option<u8>,
    pub next_model: Option<u8>,
    pub view_entity: Option<u16>,
    pub view_angles: Vec3,
    pub sim_origin: Vec3,
    pub sim_velocity: Vec3,
    pub sim_angles: Vec3,
    pub server_version: Option<i32>,
    pub stats: [i32; MAX_CL_STATS],
    pub lightstyles: Vec<String>,
    pub baselines: Vec<EntityState>,
    pub frames: Vec<Frame>,
    pub valid_sequence: i32,
    pub server_time: f32,
    pub signon_num: Option<u8>,
    pub particle_effects: Vec<qw_common::ParticleEffect>,
    pub temp_entities: Vec<qw_common::TempEntityMessage>,
    pub nails: Vec<NailProjectile>,
    pub static_entities: Vec<EntityState>,
    pub static_sounds: Vec<StaticSound>,
    pub intermission: Option<(Vec3, Vec3)>,
    pub finale: Option<String>,
    pub show_sellscreen: bool,
    pub kick_angle: f32,
    pub paused: bool,
    pub disconnected: bool,
    pub cd_track: Option<u8>,
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
            collision: None,
            collision_map: None,
            players,
            sounds: Vec::new(),
            models: Vec::new(),
            client_data: None,
            sound_events: Vec::new(),
            stop_sounds: Vec::new(),
            muzzle_flashes: Vec::new(),
            damage_events: Vec::new(),
            prints: Vec::new(),
            center_prints: Vec::new(),
            next_sound: None,
            next_model: None,
            view_entity: None,
            view_angles: Vec3::default(),
            sim_origin: Vec3::default(),
            sim_velocity: Vec3::default(),
            sim_angles: Vec3::default(),
            server_version: None,
            stats: [0; MAX_CL_STATS],
            lightstyles: vec![String::new(); MAX_LIGHTSTYLES],
            baselines,
            frames,
            valid_sequence: 0,
            server_time: 0.0,
            signon_num: None,
            particle_effects: Vec::new(),
            temp_entities: Vec::new(),
            nails: Vec::new(),
            static_entities: Vec::new(),
            static_sounds: Vec::new(),
            intermission: None,
            finale: None,
            show_sellscreen: false,
            kick_angle: 0.0,
            paused: false,
            disconnected: false,
            cd_track: None,
        }
    }

    pub fn apply_message(&mut self, msg: &SvcMessage, incoming_sequence: u32) {
        match msg {
            SvcMessage::ServerData(data) => {
                self.serverdata = Some(data.clone());
                self.collision = None;
                self.collision_map = None;
                self.sounds.clear();
                self.models.clear();
                self.client_data = None;
                self.server_version = None;
                self.view_entity = None;
                self.view_angles = Vec3::default();
                self.sim_origin = Vec3::default();
                self.sim_velocity = Vec3::default();
                self.sim_angles = Vec3::default();
                self.server_time = 0.0;
                self.paused = false;
                self.next_sound = None;
                self.next_model = None;
                self.signon_num = None;
                self.particle_effects.clear();
                self.temp_entities.clear();
                self.nails.clear();
                self.sound_events.clear();
                self.stop_sounds.clear();
                self.muzzle_flashes.clear();
                self.damage_events.clear();
                self.prints.clear();
                self.center_prints.clear();
                self.static_entities.clear();
                self.static_sounds.clear();
                self.intermission = None;
                self.finale = None;
                self.show_sellscreen = false;
                self.kick_angle = 0.0;
                self.disconnected = false;
                self.cd_track = None;
            }
            SvcMessage::Disconnect => {
                self.disconnected = true;
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
                    if key.starts_with('*') {
                        let _ = player.userinfo.set_star(key, value);
                    } else {
                        let _ = player.userinfo.set(key, value);
                    }
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
                if key.starts_with('*') {
                    let _ = self.serverinfo.set_star(key, value);
                } else {
                    let _ = self.serverinfo.set(key, value);
                }
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
                    if let Some(msec) = info.msec {
                        player.msec = msec;
                    }
                    if let Some(cmd) = info.command {
                        player.cmd = cmd;
                    }
                    if let Some(model_index) = info.model_index {
                        player.model_index = model_index;
                    }
                    if let Some(skin) = info.skin_num {
                        player.skin_num = skin;
                    }
                    if let Some(effects) = info.effects {
                        player.effects = effects;
                    }
                    if let Some(weapon_frame) = info.weapon_frame {
                        player.weapon_frame = weapon_frame;
                    }
                }

                let frame_index = (incoming_sequence as usize) & UPDATE_MASK;
                if let Some(state) = self.frames.get_mut(frame_index) {
                    let slot = info.num as usize;
                    if let Some(player_state) = state.playerstate.get_mut(slot) {
                        player_state.messagenum = incoming_sequence as i32;
                        player_state.state_time = self.server_time as f64;
                        player_state.flags = info.flags as i32;
                        player_state.origin = info.origin;
                        player_state.velocity = Vec3::new(
                            info.velocity[0] as f32,
                            info.velocity[1] as f32,
                            info.velocity[2] as f32,
                        );
                        player_state.frame = info.frame as i32;
                        if let Some(cmd) = info.command {
                            player_state.command = cmd;
                            player_state.viewangles = cmd.angles;
                        }
                        if let Some(model_index) = info.model_index {
                            player_state.modelindex = model_index as i32;
                        }
                        if let Some(skin_num) = info.skin_num {
                            player_state.skinnum = skin_num as i32;
                        }
                        if let Some(effects) = info.effects {
                            player_state.effects = effects as i32;
                        }
                        if let Some(weapon_frame) = info.weapon_frame {
                            player_state.weaponframe = weapon_frame as i32;
                        }
                    }
                }
            }
            SvcMessage::SoundList(chunk) => {
                apply_string_list(&mut self.sounds, chunk);
                self.next_sound = if chunk.next == 0 {
                    None
                } else {
                    Some(chunk.next)
                };
            }
            SvcMessage::ModelList(chunk) => {
                apply_string_list(&mut self.models, chunk);
                self.next_model = if chunk.next == 0 {
                    None
                } else {
                    Some(chunk.next)
                };
            }
            SvcMessage::SetView { entity } => {
                self.view_entity = Some(*entity);
            }
            SvcMessage::SetAngle(angles) => {
                self.view_angles = *angles;
            }
            SvcMessage::Version(version) => {
                self.server_version = Some(*version);
            }
            SvcMessage::ClientData(data) => {
                self.client_data = Some(data.clone());
                self.stats[STAT_HEALTH] = data.health as i32;
                self.stats[STAT_WEAPON] = data.weapon as i32;
                self.stats[STAT_ARMOR] = data.armor as i32;
                self.stats[STAT_AMMO] = data.ammo as i32;
                self.stats[STAT_SHELLS] = data.ammo_counts[0] as i32;
                self.stats[STAT_NAILS] = data.ammo_counts[1] as i32;
                self.stats[STAT_ROCKETS] = data.ammo_counts[2] as i32;
                self.stats[STAT_CELLS] = data.ammo_counts[3] as i32;
                self.stats[STAT_ACTIVEWEAPON] = data.active_weapon as i32;
                self.stats[STAT_ITEMS] = data.items;
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
            SvcMessage::CdTrack(track) => {
                self.cd_track = Some(*track);
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
            SvcMessage::Nails { projectiles } => {
                self.nails = projectiles.clone();
            }
            SvcMessage::Sound(sound) => {
                self.sound_events.push(sound.clone());
            }
            SvcMessage::StopSound { entity, channel } => {
                self.stop_sounds.push(StopSoundEvent {
                    entity: *entity,
                    channel: *channel,
                });
            }
            SvcMessage::MuzzleFlash { entity } => {
                self.muzzle_flashes.push(*entity);
            }
            SvcMessage::Damage {
                armor,
                blood,
                origin,
            } => {
                self.damage_events.push(DamageEvent {
                    armor: *armor,
                    blood: *blood,
                    origin: *origin,
                });
            }
            SvcMessage::Print { level, message } => {
                self.prints.push((*level, message.clone()));
            }
            SvcMessage::CenterPrint(text) => {
                self.center_prints.push(text.clone());
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

    pub fn predict_usercmd(
        &self,
        from: &PlayerState,
        cmd: UserCmd,
        spectator: bool,
    ) -> Option<PlayerState> {
        self.predict_usercmd_with_physents(from, cmd, spectator, None)
    }

    fn predict_usercmd_with_physents(
        &self,
        from: &PlayerState,
        cmd: UserCmd,
        spectator: bool,
        physents: Option<&[PhysEnt]>,
    ) -> Option<PlayerState> {
        if cmd.msec > 50 {
            let mut split = cmd;
            split.msec /= 2;
            let mid = self.predict_usercmd_with_physents(from, split, spectator, physents)?;
            return self.predict_usercmd_with_physents(&mid, split, spectator, physents);
        }
        self.predict_usercmd_internal(from, cmd, spectator, physents)
    }

    fn predict_usercmd_internal(
        &self,
        from: &PlayerState,
        cmd: UserCmd,
        spectator: bool,
        physents: Option<&[PhysEnt]>,
    ) -> Option<PlayerState> {
        let collision = self.collision.as_ref()?;
        let movevars = self.serverdata.as_ref()?.movevars;
        let mut pmove = PlayerMove::new(cmd);
        pmove.origin = from.origin;
        pmove.angles = cmd.angles;
        pmove.velocity = from.velocity;
        pmove.oldbuttons = from.oldbuttons;
        pmove.waterjumptime = from.waterjumptime;
        pmove.dead = self.stats[STAT_HEALTH] <= 0;
        pmove.spectator = spectator;
        pmove.add_world(0);
        if let Some(physents) = physents {
            for ent in physents {
                if ent.model == Some(0) {
                    continue;
                }
                pmove.add_physent(ent.clone());
            }
        }
        pmove.simulate(collision, movevars);

        let mut out = *from;
        out.command = cmd;
        out.waterjumptime = pmove.waterjumptime;
        out.oldbuttons = pmove.cmd.buttons as i32;
        out.origin = pmove.origin;
        out.viewangles = pmove.angles;
        out.velocity = pmove.velocity;
        out.onground = pmove.onground;
        Some(out)
    }

    pub fn predict_move(&mut self, incoming_sequence: u32, outgoing_sequence: u32, now: f64) {
        if self.paused || self.intermission.is_some() {
            return;
        }
        let Some(data) = &self.serverdata else {
            return;
        };
        if self.valid_sequence == 0 {
            return;
        }
        if outgoing_sequence.wrapping_sub(incoming_sequence) >= (UPDATE_BACKUP as u32 - 1) {
            return;
        }

        let player_num = data.player_num as usize;
        let spectator = data.spectator;
        let frame_index = (incoming_sequence as usize) & UPDATE_MASK;
        let physents = self.build_physents(&self.frames[frame_index], player_num);
        let mut from_seq = incoming_sequence;
        let mut from_state = self.frames[frame_index].playerstate[player_num];
        let mut to_state = None;
        let mut to_seq = None;

        for i in 1..(UPDATE_BACKUP - 1) {
            let seq = incoming_sequence.wrapping_add(i as u32);
            if seq >= outgoing_sequence {
                break;
            }
            let index = (seq as usize) & UPDATE_MASK;
            let cmd = self.frames[index].cmd;
            let Some(predicted) =
                self.predict_usercmd_with_physents(&from_state, cmd, spectator, Some(&physents))
            else {
                break;
            };
            self.frames[index].playerstate[player_num] = predicted;
            to_state = Some(predicted);
            to_seq = Some(seq);
            if self.frames[index].senttime >= now {
                break;
            }
            from_state = predicted;
            from_seq = seq;
        }

        let Some(to_state) = to_state else {
            self.sim_origin = from_state.origin;
            self.sim_velocity = from_state.velocity;
            self.sim_angles = from_state.viewangles;
            return;
        };

        let from_time = self.frames[(from_seq as usize) & UPDATE_MASK].senttime;
        let to_time = self.frames[(to_seq.unwrap() as usize) & UPDATE_MASK].senttime;
        let mut frac = if to_time == from_time {
            0.0
        } else {
            (now - from_time) / (to_time - from_time)
        };
        if frac < 0.0 {
            frac = 0.0;
        }
        if frac > 1.0 {
            frac = 1.0;
        }
        let frac = frac as f32;

        let delta = Vec3::new(
            (from_state.origin.x - to_state.origin.x).abs(),
            (from_state.origin.y - to_state.origin.y).abs(),
            (from_state.origin.z - to_state.origin.z).abs(),
        );
        if delta.x > 128.0 || delta.y > 128.0 || delta.z > 128.0 {
            self.sim_origin = to_state.origin;
            self.sim_velocity = to_state.velocity;
            self.sim_angles = to_state.viewangles;
            return;
        }

        self.sim_origin = Vec3::new(
            from_state.origin.x + frac * (to_state.origin.x - from_state.origin.x),
            from_state.origin.y + frac * (to_state.origin.y - from_state.origin.y),
            from_state.origin.z + frac * (to_state.origin.z - from_state.origin.z),
        );
        self.sim_velocity = Vec3::new(
            from_state.velocity.x + frac * (to_state.velocity.x - from_state.velocity.x),
            from_state.velocity.y + frac * (to_state.velocity.y - from_state.velocity.y),
            from_state.velocity.z + frac * (to_state.velocity.z - from_state.velocity.z),
        );
        self.sim_angles = from_state.viewangles;
    }

    fn build_physents(&self, frame: &Frame, player_num: usize) -> Vec<PhysEnt> {
        let mut physents = Vec::new();
        physents.push(PhysEnt {
            origin: Vec3::default(),
            model: Some(0),
            mins: Vec3::default(),
            maxs: Vec3::default(),
            info: 0,
        });

        let player_mins = Vec3::new(-16.0, -16.0, -24.0);
        let player_maxs = Vec3::new(16.0, 16.0, 32.0);

        for (index, state) in frame.playerstate.iter().enumerate() {
            if index == player_num || state.messagenum == 0 {
                continue;
            }
            physents.push(PhysEnt {
                origin: state.origin,
                model: None,
                mins: player_mins,
                maxs: player_maxs,
                info: index as i32,
            });
        }

        let entity_count = frame.packet_entities.num_entities;
        for ent in frame.packet_entities.entities.iter().take(entity_count) {
            if ent.modelindex <= 0 {
                continue;
            }
            let model_index = ent.modelindex as usize;
            let Some(name) = self.models.get(model_index) else {
                continue;
            };
            if !name.starts_with('*') {
                continue;
            }
            let Ok(submodel_index) = name[1..].parse::<usize>() else {
                continue;
            };
            physents.push(PhysEnt {
                origin: ent.origin,
                model: Some(submodel_index),
                mins: Vec3::default(),
                maxs: Vec3::default(),
                info: ent.number,
            });
        }

        physents
    }

    pub fn clear_frame_events(&mut self) {
        self.particle_effects.clear();
        self.temp_entities.clear();
        self.nails.clear();
        self.sound_events.clear();
        self.stop_sounds.clear();
        self.muzzle_flashes.clear();
        self.damage_events.clear();
        self.prints.clear();
        self.center_prints.clear();
    }

    pub fn mark_choked(&mut self, count: u8, acknowledged: u32) {
        for offset in 0..count {
            let index = (acknowledged.wrapping_sub(1 + offset as u32) as usize) & UPDATE_MASK;
            self.frames[index].receivedtime = -2.0;
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
        assert!(
            state.players[1]
                .userinfo
                .as_str()
                .contains("\\name\\player")
        );
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
        state.apply_message(
            &SvcMessage::SetInfo {
                slot: 2,
                key: "*spectator".to_string(),
                value: "1".to_string(),
            },
            0,
        );
        let info = state.players[2].userinfo.as_str();
        assert!(info.contains("\\team\\red"));
        assert!(info.contains("\\*spectator\\1"));
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
        state.apply_message(
            &SvcMessage::ServerInfo {
                key: "*version".to_string(),
                value: "1.0".to_string(),
            },
            0,
        );
        let info = state.serverinfo.as_str();
        assert!(info.contains("\\hostname\\server"));
        assert!(info.contains("\\*version\\1.0"));
    }

    #[test]
    fn applies_playerinfo() {
        let mut state = ClientState::new();
        state.apply_message(
            &SvcMessage::PlayerInfo(qw_common::PlayerInfoMessage {
                num: 0,
                flags: qw_common::PF_MSEC as u16
                    | qw_common::PF_COMMAND as u16
                    | qw_common::PF_MODEL as u16
                    | qw_common::PF_SKINNUM as u16
                    | qw_common::PF_EFFECTS as u16
                    | qw_common::PF_WEAPONFRAME as u16,
                origin: Vec3::new(1.0, 2.0, 3.0),
                frame: 4,
                msec: Some(12),
                command: Some(UserCmd {
                    msec: 5,
                    angles: Vec3::new(0.0, 1.0, 2.0),
                    forwardmove: 10,
                    sidemove: 20,
                    upmove: 30,
                    buttons: 1,
                    impulse: 0,
                }),
                velocity: [10, 20, 30],
                model_index: Some(2),
                skin_num: Some(3),
                effects: Some(4),
                weapon_frame: Some(5),
            }),
            0,
        );
        assert_eq!(state.players[0].origin, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(state.players[0].frame, 4);
        assert_eq!(state.players[0].velocity, [10, 20, 30]);
        assert_eq!(state.players[0].model_index, 2);
        assert_eq!(state.players[0].msec, 12);
        assert_eq!(state.players[0].cmd.forwardmove, 10);
        assert_eq!(state.players[0].skin_num, 3);
        assert_eq!(state.players[0].effects, 4);
        assert_eq!(state.players[0].weapon_frame, 5);

        let frame = &state.frames[0];
        let player_state = &frame.playerstate[0];
        assert_eq!(player_state.origin, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(player_state.velocity, Vec3::new(10.0, 20.0, 30.0));
        assert_eq!(player_state.modelindex, 2);
        assert_eq!(player_state.skinnum, 3);
        assert_eq!(player_state.effects, 4);
        assert_eq!(player_state.weaponframe, 5);
        assert_eq!(player_state.viewangles, Vec3::new(0.0, 1.0, 2.0));
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
        state.apply_message(&SvcMessage::UpdateFrags { slot: 0, frags: 5 }, 0);
        state.apply_message(&SvcMessage::UpdatePing { slot: 0, ping: 50 }, 0);
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
        state.view_entity = Some(9);
        state.view_angles = Vec3::new(1.0, 2.0, 3.0);
        state.server_time = 7.0;
        state.paused = true;
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
        assert_eq!(state.view_entity, None);
        assert_eq!(state.view_angles, Vec3::default());
        assert_eq!(state.server_time, 0.0);
        assert!(!state.paused);

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
        state.apply_message(&SvcMessage::Version(28), 0);
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
        assert_eq!(state.server_version, Some(28));
        assert_eq!(state.server_time, 12.5);
        assert!(state.paused);
    }

    #[test]
    fn applies_disconnect() {
        let mut state = ClientState::new();
        state.apply_message(&SvcMessage::Disconnect, 0);
        assert!(state.disconnected);
    }

    #[test]
    fn stores_clientdata_message() {
        let mut state = ClientState::new();
        let data = ClientDataMessage {
            bits: qw_common::SU_ONGROUND,
            view_height: 22,
            ideal_pitch: 0,
            punch_angle: Vec3::new(1.0, 0.0, 0.0),
            velocity: Vec3::new(16.0, 0.0, 0.0),
            items: 5,
            onground: true,
            inwater: false,
            weapon_frame: 0,
            armor: 0,
            weapon: 0,
            health: 100,
            ammo: 10,
            ammo_counts: [1, 2, 3, 4],
            active_weapon: 1,
        };

        state.apply_message(&SvcMessage::ClientData(data.clone()), 0);

        assert_eq!(state.client_data, Some(data));
        assert_eq!(state.stats[STAT_HEALTH], 100);
        assert_eq!(state.stats[STAT_WEAPON], 0);
        assert_eq!(state.stats[STAT_ARMOR], 0);
        assert_eq!(state.stats[STAT_AMMO], 10);
        assert_eq!(state.stats[STAT_SHELLS], 1);
        assert_eq!(state.stats[STAT_NAILS], 2);
        assert_eq!(state.stats[STAT_ROCKETS], 3);
        assert_eq!(state.stats[STAT_CELLS], 4);
        assert_eq!(state.stats[STAT_ACTIVEWEAPON], 1);
        assert_eq!(state.stats[STAT_ITEMS], 5);
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
    fn queues_sound_and_damage_events() {
        let mut state = ClientState::new();
        let sound = qw_common::SoundMessage {
            entity: 4,
            channel: 2,
            sound_num: 7,
            volume: 200,
            attenuation: 0.8,
            origin: Vec3::new(1.0, 2.0, 3.0),
        };
        state.apply_message(&SvcMessage::Sound(sound.clone()), 0);
        state.apply_message(
            &SvcMessage::StopSound {
                entity: 5,
                channel: 1,
            },
            0,
        );
        state.apply_message(&SvcMessage::MuzzleFlash { entity: 9 }, 0);
        state.apply_message(
            &SvcMessage::Damage {
                armor: 3,
                blood: 5,
                origin: Vec3::new(4.0, 5.0, 6.0),
            },
            0,
        );

        assert_eq!(state.sound_events, vec![sound]);
        assert_eq!(
            state.stop_sounds,
            vec![StopSoundEvent {
                entity: 5,
                channel: 1
            }]
        );
        assert_eq!(state.muzzle_flashes, vec![9]);
        assert_eq!(
            state.damage_events,
            vec![DamageEvent {
                armor: 3,
                blood: 5,
                origin: Vec3::new(4.0, 5.0, 6.0),
            }]
        );
    }

    #[test]
    fn queues_print_messages() {
        let mut state = ClientState::new();
        state.apply_message(
            &SvcMessage::Print {
                level: 1,
                message: "hello".to_string(),
            },
            0,
        );
        state.apply_message(&SvcMessage::CenterPrint("center".to_string()), 0);

        assert_eq!(state.prints, vec![(1, "hello".to_string())]);
        assert_eq!(state.center_prints, vec!["center".to_string()]);
    }

    #[test]
    fn clears_frame_events() {
        let mut state = ClientState::new();
        state.particle_effects.push(qw_common::ParticleEffect {
            origin: Vec3::new(1.0, 2.0, 3.0),
            direction: Vec3::new(0.0, 0.0, 1.0),
            count: 1,
            color: 0,
        });
        state.temp_entities.push(qw_common::TempEntityMessage {
            kind: qw_common::TE_SPIKE,
            origin: Some(Vec3::new(1.0, 2.0, 3.0)),
            start: None,
            end: None,
            count: None,
            entity: None,
        });
        state.nails.push(qw_common::NailProjectile {
            origin: Vec3::new(1.0, 2.0, 3.0),
            angles: Vec3::new(0.0, 0.0, 0.0),
        });
        state.sound_events.push(qw_common::SoundMessage {
            entity: 1,
            channel: 0,
            sound_num: 2,
            volume: 255,
            attenuation: 1.0,
            origin: Vec3::new(0.0, 0.0, 0.0),
        });
        state.stop_sounds.push(StopSoundEvent {
            entity: 2,
            channel: 1,
        });
        state.muzzle_flashes.push(3);
        state.damage_events.push(DamageEvent {
            armor: 1,
            blood: 2,
            origin: Vec3::new(4.0, 5.0, 6.0),
        });
        state.prints.push((1, "test".to_string()));
        state.center_prints.push("center".to_string());

        state.clear_frame_events();

        assert!(state.particle_effects.is_empty());
        assert!(state.temp_entities.is_empty());
        assert!(state.nails.is_empty());
        assert!(state.sound_events.is_empty());
        assert!(state.stop_sounds.is_empty());
        assert!(state.muzzle_flashes.is_empty());
        assert!(state.damage_events.is_empty());
        assert!(state.prints.is_empty());
        assert!(state.center_prints.is_empty());
    }

    #[test]
    fn marks_choked_frames() {
        let mut state = ClientState::new();
        state.frames[2].receivedtime = 1.0;
        state.frames[3].receivedtime = 1.0;

        state.mark_choked(2, 4);

        assert_eq!(state.frames[3].receivedtime, -2.0);
        assert_eq!(state.frames[2].receivedtime, -2.0);
    }

    #[test]
    fn stores_nail_projectiles() {
        let mut state = ClientState::new();
        let projectile = qw_common::NailProjectile {
            origin: Vec3::new(1.0, 2.0, 3.0),
            angles: Vec3::new(45.0, 90.0, 0.0),
        };
        state.apply_message(
            &SvcMessage::Nails {
                projectiles: vec![projectile.clone()],
            },
            0,
        );

        assert_eq!(state.nails, vec![projectile]);
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
        state.apply_message(&SvcMessage::CdTrack(3), 0);
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
        assert_eq!(state.cd_track, Some(3));
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

    #[test]
    fn predict_usercmd_requires_collision_and_serverdata() {
        let state = ClientState::new();
        let from = PlayerState::default();
        let cmd = UserCmd::default();
        assert!(state.predict_usercmd(&from, cmd, false).is_none());
    }

    #[test]
    fn predict_usercmd_updates_buttons() {
        let mut state = ClientState::new();
        state.serverdata = Some(qw_common::ServerData {
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
        });
        state.collision = Some(BspCollision {
            planes: Vec::new(),
            clipnodes: Vec::new(),
            hull0_clipnodes: Vec::new(),
            models: Vec::new(),
        });

        let from = PlayerState::default();
        let mut cmd = UserCmd::default();
        cmd.msec = 20;
        cmd.buttons = 7;
        let predicted = state.predict_usercmd(&from, cmd, false).unwrap();
        assert_eq!(predicted.oldbuttons, 7);
        assert_eq!(predicted.command, cmd);
    }
}
