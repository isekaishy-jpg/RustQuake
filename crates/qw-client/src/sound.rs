use std::collections::HashMap;

use qw_audio::{AudioSystem, SoundId};

use crate::state::ClientState;

#[derive(Debug, Default)]
pub struct SoundManager {
    active: HashMap<(u16, u8), SoundId>,
    static_started: usize,
}

impl SoundManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_events(&mut self, audio: &mut AudioSystem, state: &mut ClientState) {
        self.start_static_sounds(audio, state);
        self.start_dynamic_sounds(audio, state);
        self.stop_sounds(state);
    }

    fn start_static_sounds(&mut self, audio: &mut AudioSystem, state: &ClientState) {
        if self.static_started >= state.static_sounds.len() {
            return;
        }

        for static_sound in state.static_sounds.iter().skip(self.static_started) {
            if sound_name_for(&state.sounds, static_sound.sound).is_some() {
                audio.play_sound();
            }
        }
        self.static_started = state.static_sounds.len();
    }

    fn start_dynamic_sounds(&mut self, audio: &mut AudioSystem, state: &mut ClientState) {
        let sound_names = state.sounds.clone();
        for sound in state.sound_events.drain(..) {
            if sound_name_for(&sound_names, sound.sound_num).is_none() {
                continue;
            }
            let id = audio.play_sound();
            self.active.insert((sound.entity, sound.channel), id);
        }
    }

    fn stop_sounds(&mut self, state: &mut ClientState) {
        for stop in state.stop_sounds.drain(..) {
            self.active.remove(&(stop.entity, stop.channel));
        }
    }
}

fn sound_name_for(sound_names: &[String], sound_num: u8) -> Option<&str> {
    let index = sound_num as usize;
    sound_names
        .get(index)
        .map(String::as_str)
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{StaticSound, StopSoundEvent};
    use qw_audio::AudioConfig;
    use qw_common::{SoundMessage, Vec3};

    #[test]
    fn tracks_dynamic_sound_lifecycle() {
        let mut state = ClientState::new();
        state.sounds.push("misc/foo.wav".to_string());
        state.sound_events.push(SoundMessage {
            entity: 7,
            channel: 2,
            sound_num: 0,
            volume: 255,
            attenuation: 1.0,
            origin: Vec3::default(),
        });

        let mut audio = AudioSystem::new(AudioConfig::default());
        let mut manager = SoundManager::new();
        manager.handle_events(&mut audio, &mut state);
        assert!(manager.active.contains_key(&(7, 2)));

        state.stop_sounds.push(StopSoundEvent {
            entity: 7,
            channel: 2,
        });
        manager.handle_events(&mut audio, &mut state);
        assert!(!manager.active.contains_key(&(7, 2)));
    }

    #[test]
    fn advances_static_sound_cursor() {
        let mut state = ClientState::new();
        state.sounds.push("ambience/hum.wav".to_string());
        state.static_sounds.push(StaticSound {
            origin: Vec3::default(),
            sound: 0,
            volume: 255,
            attenuation: 0,
        });

        let mut audio = AudioSystem::new(AudioConfig::default());
        let mut manager = SoundManager::new();
        manager.handle_events(&mut audio, &mut state);
        assert_eq!(manager.static_started, 1);

        state.static_sounds.push(StaticSound {
            origin: Vec3::new(1.0, 2.0, 3.0),
            sound: 0,
            volume: 200,
            attenuation: 64,
        });
        manager.handle_events(&mut audio, &mut state);
        assert_eq!(manager.static_started, 2);
    }
}
