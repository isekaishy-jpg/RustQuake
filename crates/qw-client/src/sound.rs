use std::collections::HashMap;
use std::sync::Arc;

use qw_audio::{AudioClip, AudioSystem, PlayParams, SoundId};

use crate::state::ClientState;
use qw_common::QuakeFs;

#[derive(Debug, Default)]
pub struct SoundManager {
    active: HashMap<(u16, u8), SoundId>,
    static_started: usize,
    cache: HashMap<String, Arc<AudioClip>>,
}

impl SoundManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_events(
        &mut self,
        audio: &mut AudioSystem,
        state: &mut ClientState,
        fs: &QuakeFs,
    ) {
        self.start_static_sounds(audio, state, fs);
        self.start_dynamic_sounds(audio, state, fs);
        self.stop_sounds(audio, state);
    }

    fn start_static_sounds(&mut self, audio: &mut AudioSystem, state: &ClientState, fs: &QuakeFs) {
        if self.static_started >= state.static_sounds.len() {
            return;
        }

        for static_sound in state.static_sounds.iter().skip(self.static_started) {
            let Some(name) = sound_name_for(&state.sounds, static_sound.sound) else {
                continue;
            };
            let Some(clip) = self.load_clip(fs, name) else {
                continue;
            };
            let params = PlayParams {
                position: [
                    static_sound.origin.x,
                    static_sound.origin.y,
                    static_sound.origin.z,
                ],
                volume: static_sound.volume as f32 / 255.0,
                attenuation: static_sound.attenuation as f32 / 64.0,
                looping: true,
            };
            let _ = audio.play_clip(clip, params);
        }
        self.static_started = state.static_sounds.len();
    }

    fn start_dynamic_sounds(
        &mut self,
        audio: &mut AudioSystem,
        state: &mut ClientState,
        fs: &QuakeFs,
    ) {
        let sound_names = state.sounds.clone();
        for sound in state.sound_events.drain(..) {
            let Some(name) = sound_name_for(&sound_names, sound.sound_num) else {
                continue;
            };
            let Some(clip) = self.load_clip(fs, name) else {
                continue;
            };
            let params = PlayParams {
                position: [sound.origin.x, sound.origin.y, sound.origin.z],
                volume: sound.volume as f32 / 255.0,
                attenuation: sound.attenuation,
                looping: false,
            };
            if let Ok(id) = audio.play_clip(clip, params) {
                self.active.insert((sound.entity, sound.channel), id);
            }
        }
    }

    fn stop_sounds(&mut self, audio: &mut AudioSystem, state: &mut ClientState) {
        for stop in state.stop_sounds.drain(..) {
            if let Some(id) = self.active.remove(&(stop.entity, stop.channel)) {
                audio.stop_sound(id);
            }
        }
    }

    fn load_clip(&mut self, fs: &QuakeFs, name: &str) -> Option<Arc<AudioClip>> {
        let path = if name.starts_with("sound/") {
            name.to_string()
        } else {
            format!("sound/{}", name)
        };
        if let Some(clip) = self.cache.get(&path) {
            return Some(clip.clone());
        }
        let bytes = fs.read(&path).ok()?;
        let clip = AudioClip::from_wav(&bytes).ok()?;
        let clip = Arc::new(clip);
        self.cache.insert(path, clip.clone());
        Some(clip)
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
    use qw_common::{QuakeFs, SoundMessage, Vec3};
    use std::fs;
    use std::io::Write;
    use std::path::Path;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut path = std::env::temp_dir();
        path.push(format!("rustquake-test-{}-{}", process::id(), nanos));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_wav(path: &Path) {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&(36u32).to_le_bytes());
        data.extend_from_slice(b"WAVE");
        data.extend_from_slice(b"fmt ");
        data.extend_from_slice(&(16u32).to_le_bytes());
        data.extend_from_slice(&(1u16).to_le_bytes());
        data.extend_from_slice(&(1u16).to_le_bytes());
        data.extend_from_slice(&(44100u32).to_le_bytes());
        data.extend_from_slice(&(88200u32).to_le_bytes());
        data.extend_from_slice(&(2u16).to_le_bytes());
        data.extend_from_slice(&(16u16).to_le_bytes());
        data.extend_from_slice(b"data");
        data.extend_from_slice(&(2u32).to_le_bytes());
        data.extend_from_slice(&(0i16).to_le_bytes());
        let mut file = fs::File::create(path).unwrap();
        file.write_all(&data).unwrap();
    }

    #[test]
    fn tracks_dynamic_sound_lifecycle() {
        let dir = temp_dir();
        let sound_path = dir.join("sound").join("misc");
        fs::create_dir_all(&sound_path).unwrap();
        write_wav(&sound_path.join("foo.wav"));
        let mut fs = QuakeFs::new();
        fs.add_game_dir(&dir).unwrap();

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
        manager.handle_events(&mut audio, &mut state, &fs);
        assert!(manager.active.contains_key(&(7, 2)));

        state.stop_sounds.push(StopSoundEvent {
            entity: 7,
            channel: 2,
        });
        manager.handle_events(&mut audio, &mut state, &fs);
        assert!(!manager.active.contains_key(&(7, 2)));

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn advances_static_sound_cursor() {
        let dir = temp_dir();
        let sound_path = dir.join("sound").join("ambience");
        fs::create_dir_all(&sound_path).unwrap();
        write_wav(&sound_path.join("hum.wav"));
        let mut fs = QuakeFs::new();
        fs.add_game_dir(&dir).unwrap();

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
        manager.handle_events(&mut audio, &mut state, &fs);
        assert_eq!(manager.static_started, 1);

        state.static_sounds.push(StaticSound {
            origin: Vec3::new(1.0, 2.0, 3.0),
            sound: 0,
            volume: 200,
            attenuation: 64,
        });
        manager.handle_events(&mut audio, &mut state, &fs);
        assert_eq!(manager.static_started, 2);

        fs::remove_dir_all(dir).ok();
    }
}
