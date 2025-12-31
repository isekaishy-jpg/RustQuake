use std::collections::HashMap;
use std::sync::Arc;

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

#[derive(Debug, Clone)]
pub struct AudioClip {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
}

#[derive(Debug)]
pub enum AudioError {
    InvalidHeader,
    UnsupportedFormat,
    UnsupportedBitsPerSample(u16),
    UnsupportedSampleRate(u32),
    UnexpectedEof,
}

#[derive(Debug, Clone, Copy)]
pub struct PlayParams {
    pub position: [f32; 3],
    pub volume: f32,
    pub attenuation: f32,
    pub looping: bool,
}

impl Default for PlayParams {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            volume: 1.0,
            attenuation: 1.0,
            looping: false,
        }
    }
}

#[derive(Debug)]
struct Voice {
    clip: Arc<AudioClip>,
    cursor: usize,
    params: PlayParams,
}

#[derive(Debug)]
pub struct AudioSystem {
    config: AudioConfig,
    running: bool,
    next_id: u32,
    listener_pos: [f32; 3],
    listener_right: [f32; 3],
    voices: HashMap<SoundId, Voice>,
}

impl AudioSystem {
    pub fn new(config: AudioConfig) -> Self {
        Self {
            config,
            running: true,
            next_id: 1,
            listener_pos: [0.0; 3],
            listener_right: [1.0, 0.0, 0.0],
            voices: HashMap::new(),
        }
    }

    pub fn config(&self) -> AudioConfig {
        self.config
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn play_clip(
        &mut self,
        clip: Arc<AudioClip>,
        params: PlayParams,
    ) -> Result<SoundId, AudioError> {
        if clip.sample_rate != self.config.sample_rate {
            return Err(AudioError::UnsupportedSampleRate(clip.sample_rate));
        }
        let id = SoundId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.voices.insert(
            id,
            Voice {
                clip,
                cursor: 0,
                params,
            },
        );
        Ok(id)
    }

    pub fn stop_sound(&mut self, id: SoundId) {
        self.voices.remove(&id);
    }

    pub fn set_listener(&mut self, position: [f32; 3], right: [f32; 3]) {
        self.listener_pos = position;
        self.listener_right = normalize(right).unwrap_or([1.0, 0.0, 0.0]);
    }

    pub fn mix(&mut self, frames: usize) -> Vec<i16> {
        let channels = self.config.channels as usize;
        let mut output = vec![0f32; frames * channels];
        let mut finished = Vec::new();

        for (id, voice) in self.voices.iter_mut() {
            let clip_channels = voice.clip.channels as usize;
            if clip_channels == 0 {
                finished.push(*id);
                continue;
            }
            let total_frames = voice.clip.samples.len() / clip_channels;
            if total_frames == 0 {
                finished.push(*id);
                continue;
            }
            let (left_gain, right_gain) =
                compute_gains(voice.params, self.listener_pos, self.listener_right);

            let mut out_index = 0;
            for _ in 0..frames {
                if voice.cursor >= total_frames {
                    if voice.params.looping {
                        voice.cursor = 0;
                    } else {
                        finished.push(*id);
                        break;
                    }
                }
                let base = voice.cursor * clip_channels;
                let sample_l = voice.clip.samples[base] as f32;
                let sample_r = if clip_channels > 1 {
                    voice.clip.samples[base + 1] as f32
                } else {
                    sample_l
                };

                output[out_index] += sample_l * left_gain;
                if channels > 1 {
                    output[out_index + 1] += sample_r * right_gain;
                }

                voice.cursor += 1;
                out_index += channels;
            }
        }

        for id in finished {
            self.voices.remove(&id);
        }

        output
            .into_iter()
            .map(|sample| sample.round().clamp(-32768.0, 32767.0) as i16)
            .collect()
    }

    pub fn stop(&mut self) {
        self.running = false;
    }
}

impl AudioClip {
    pub fn from_wav(data: &[u8]) -> Result<Self, AudioError> {
        if data.len() < 12 {
            return Err(AudioError::InvalidHeader);
        }
        if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
            return Err(AudioError::InvalidHeader);
        }

        let mut offset = 12;
        let mut channels = None;
        let mut sample_rate = None;
        let mut bits_per_sample = None;
        let mut samples = None;

        while offset + 8 <= data.len() {
            let chunk_id = &data[offset..offset + 4];
            let chunk_size =
                u32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap()) as usize;
            offset += 8;
            if offset + chunk_size > data.len() {
                return Err(AudioError::UnexpectedEof);
            }

            match chunk_id {
                b"fmt " => {
                    if chunk_size < 16 {
                        return Err(AudioError::InvalidHeader);
                    }
                    let audio_format =
                        u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap());
                    if audio_format != 1 {
                        return Err(AudioError::UnsupportedFormat);
                    }
                    let parsed_channels =
                        u16::from_le_bytes(data[offset + 2..offset + 4].try_into().unwrap());
                    let parsed_sample_rate =
                        u32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap());
                    let parsed_bits =
                        u16::from_le_bytes(data[offset + 14..offset + 16].try_into().unwrap());
                    channels = Some(parsed_channels);
                    sample_rate = Some(parsed_sample_rate);
                    bits_per_sample = Some(parsed_bits);
                }
                b"data" => {
                    samples = Some(data[offset..offset + chunk_size].to_vec());
                }
                _ => {}
            }

            offset += chunk_size;
            if chunk_size % 2 == 1 {
                offset += 1;
            }
        }

        let channels = channels.ok_or(AudioError::InvalidHeader)?;
        let sample_rate = sample_rate.ok_or(AudioError::InvalidHeader)?;
        let bits_per_sample = bits_per_sample.ok_or(AudioError::InvalidHeader)?;
        let data_bytes = samples.ok_or(AudioError::InvalidHeader)?;

        let samples = match bits_per_sample {
            8 => data_bytes
                .into_iter()
                .map(|value| ((value as i16) - 128) << 8)
                .collect::<Vec<_>>(),
            16 => {
                if data_bytes.len() % 2 != 0 {
                    return Err(AudioError::UnexpectedEof);
                }
                data_bytes
                    .chunks_exact(2)
                    .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect::<Vec<_>>()
            }
            other => return Err(AudioError::UnsupportedBitsPerSample(other)),
        };

        Ok(AudioClip {
            sample_rate,
            channels,
            samples,
        })
    }
}

fn compute_gains(
    params: PlayParams,
    listener_pos: [f32; 3],
    listener_right: [f32; 3],
) -> (f32, f32) {
    let dx = params.position[0] - listener_pos[0];
    let dy = params.position[1] - listener_pos[1];
    let dz = params.position[2] - listener_pos[2];
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();

    let attenuation = if params.attenuation <= 0.0 {
        1.0
    } else {
        1.0 / (1.0 + distance * params.attenuation)
    };
    let gain = params.volume.max(0.0) * attenuation;

    let pan = if distance > 0.0 {
        let inv = 1.0 / distance;
        let dir = [dx * inv, dy * inv, dz * inv];
        (dir[0] * listener_right[0] + dir[1] * listener_right[1] + dir[2] * listener_right[2])
            .clamp(-1.0, 1.0)
    } else {
        0.0
    };

    let left = gain * (1.0 - pan) * 0.5;
    let right = gain * (1.0 + pan) * 0.5;
    (left, right)
}

fn normalize(vec: [f32; 3]) -> Option<[f32; 3]> {
    let len = (vec[0] * vec[0] + vec[1] * vec[1] + vec[2] * vec[2]).sqrt();
    if len == 0.0 {
        None
    } else {
        Some([vec[0] / len, vec[1] / len, vec[2] / len])
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
    fn play_clip_returns_unique_ids() {
        let mut audio = AudioSystem::new(AudioConfig::default());
        let clip = Arc::new(AudioClip {
            sample_rate: audio.config().sample_rate,
            channels: 1,
            samples: vec![0, 1, 2, 3],
        });
        let first = audio
            .play_clip(clip.clone(), PlayParams::default())
            .unwrap();
        let second = audio.play_clip(clip, PlayParams::default()).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn decodes_pcm16_wav() {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&(36u32).to_le_bytes());
        data.extend_from_slice(b"WAVE");
        data.extend_from_slice(b"fmt ");
        data.extend_from_slice(&(16u32).to_le_bytes());
        data.extend_from_slice(&(1u16).to_le_bytes());
        data.extend_from_slice(&(1u16).to_le_bytes());
        data.extend_from_slice(&(8000u32).to_le_bytes());
        data.extend_from_slice(&(16000u32).to_le_bytes());
        data.extend_from_slice(&(2u16).to_le_bytes());
        data.extend_from_slice(&(16u16).to_le_bytes());
        data.extend_from_slice(b"data");
        data.extend_from_slice(&(4u32).to_le_bytes());
        data.extend_from_slice(&(0i16).to_le_bytes());
        data.extend_from_slice(&(32767i16).to_le_bytes());

        let clip = AudioClip::from_wav(&data).unwrap();
        assert_eq!(clip.sample_rate, 8000);
        assert_eq!(clip.channels, 1);
        assert_eq!(clip.samples, vec![0, 32767]);
    }
}
