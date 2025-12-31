#[derive(Debug, Clone, Copy)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: usize,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44_100,
            channels: 2,
            buffer_size: 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoundId(u32);

#[derive(Debug)]
pub struct AudioSystem {
    config: AudioConfig,
    running: bool,
    next_id: u32,
}

impl AudioSystem {
    pub fn new(config: AudioConfig) -> Self {
        Self {
            config,
            running: true,
            next_id: 1,
        }
    }

    pub fn config(&self) -> AudioConfig {
        self.config
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn play_sound(&mut self) -> SoundId {
        let id = SoundId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    pub fn stop(&mut self) {
        self.running = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_stereo() {
        let cfg = AudioConfig::default();
        assert_eq!(cfg.channels, 2);
        assert_eq!(cfg.sample_rate, 44_100);
    }

    #[test]
    fn play_sound_returns_unique_ids() {
        let mut audio = AudioSystem::new(AudioConfig::default());
        let first = audio.play_sound();
        let second = audio.play_sound();
        assert_ne!(first, second);
    }
}
