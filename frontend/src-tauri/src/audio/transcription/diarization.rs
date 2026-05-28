use crate::audio::RecordingDeviceType;

const MAX_SPEAKERS_PER_SOURCE: usize = 6;
const MIN_CLUSTER_SECONDS: f64 = 0.75;
const NEW_SPEAKER_DISTANCE: f32 = 0.2;
const UPDATE_WEIGHT: f32 = 0.18;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeakerAssignment {
    pub speaker: String,
    pub speaker_id: String,
}

#[derive(Debug, Clone)]
struct SpeakerProfile {
    id: String,
    label: String,
    fingerprint: AudioFingerprint,
    segments: usize,
    last_seen_at: f64,
}

#[derive(Debug, Clone, Copy)]
struct AudioFingerprint {
    values: [f32; 6],
}

impl AudioFingerprint {
    fn from_samples(samples: &[f32], sample_rate: u32) -> Option<Self> {
        if samples.len() < (sample_rate as usize / 4).max(1) {
            return None;
        }

        let mut sum_sq = 0.0f32;
        let mut sum_abs = 0.0f32;
        let mut peak = 0.0f32;
        let mut zero_crossings = 0usize;
        let mut high_frequency_motion = 0.0f32;
        let mut prev = samples[0];

        for &sample in samples {
            let sample = sanitize_sample(sample);
            sum_sq += sample * sample;
            sum_abs += sample.abs();
            peak = peak.max(sample.abs());

            if (prev >= 0.0 && sample < 0.0) || (prev < 0.0 && sample >= 0.0) {
                zero_crossings += 1;
            }

            high_frequency_motion += (sample - prev).abs();
            prev = sample;
        }

        let len = samples.len() as f32;
        let rms = (sum_sq / len).sqrt();
        if rms < 0.002 {
            return None;
        }

        let mean_abs = sum_abs / len;
        let zcr = zero_crossings as f32 / len;
        let motion = high_frequency_motion / len / mean_abs.max(0.001);
        let dynamics = (peak / rms.max(0.001)).min(12.0) / 12.0;
        let envelope_variance = envelope_variance(samples);
        let pitch_proxy = pitch_proxy(samples, sample_rate);

        Some(Self {
            values: [
                (rms.log10() + 3.0).clamp(0.0, 1.0),
                zcr.clamp(0.0, 0.35) / 0.35,
                motion.clamp(0.0, 4.0) / 4.0,
                dynamics.clamp(0.0, 1.0),
                envelope_variance.clamp(0.0, 1.0),
                pitch_proxy,
            ],
        })
    }

    fn distance(&self, other: &Self) -> f32 {
        let weights = [0.45, 1.25, 1.25, 0.7, 0.65, 1.35];
        let weighted_sum = self
            .values
            .iter()
            .zip(other.values.iter())
            .zip(weights.iter())
            .map(|((a, b), weight)| {
                let delta = a - b;
                delta * delta * weight
            })
            .sum::<f32>();

        (weighted_sum / weights.iter().sum::<f32>()).sqrt()
    }

    fn blend(&mut self, other: &Self, weight: f32) {
        for (current, next) in self.values.iter_mut().zip(other.values.iter()) {
            *current = (*current * (1.0 - weight)) + (*next * weight);
        }
    }
}

#[derive(Debug, Default)]
struct SourceProfiles {
    speakers: Vec<SpeakerProfile>,
    last_speaker_id: Option<String>,
}

#[derive(Debug, Default)]
pub struct SpeakerDiarizer {
    microphone: SourceProfiles,
    system: SourceProfiles,
}

impl SpeakerDiarizer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn assign(
        &mut self,
        source: &RecordingDeviceType,
        samples: &[f32],
        sample_rate: u32,
        timestamp: f64,
    ) -> SpeakerAssignment {
        let duration = if sample_rate == 0 {
            0.0
        } else {
            samples.len() as f64 / sample_rate as f64
        };

        let source_profiles = match source {
            RecordingDeviceType::Microphone => &mut self.microphone,
            RecordingDeviceType::System => &mut self.system,
        };

        if duration < MIN_CLUSTER_SECONDS {
            if let Some(existing) = source_profiles.last_assignment() {
                return existing;
            }
        }

        let Some(fingerprint) = AudioFingerprint::from_samples(samples, sample_rate) else {
            return source_profiles.default_assignment(source);
        };

        if source_profiles.speakers.is_empty() {
            return source_profiles.create_speaker(source, fingerprint, timestamp);
        }

        let (best_index, best_distance) = source_profiles
            .speakers
            .iter()
            .enumerate()
            .map(|(index, speaker)| (index, speaker.fingerprint.distance(&fingerprint)))
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap();

        if best_distance > NEW_SPEAKER_DISTANCE
            && source_profiles.speakers.len() < MAX_SPEAKERS_PER_SOURCE
            && duration >= MIN_CLUSTER_SECONDS
        {
            return source_profiles.create_speaker(source, fingerprint, timestamp);
        }

        let speaker = &mut source_profiles.speakers[best_index];
        let blend_weight = if speaker.segments < 3 {
            0.35
        } else {
            UPDATE_WEIGHT
        };
        speaker.fingerprint.blend(&fingerprint, blend_weight);
        speaker.segments += 1;
        speaker.last_seen_at = timestamp;
        source_profiles.last_speaker_id = Some(speaker.id.clone());

        SpeakerAssignment {
            speaker: speaker.label.clone(),
            speaker_id: speaker.id.clone(),
        }
    }
}

impl SourceProfiles {
    fn create_speaker(
        &mut self,
        source: &RecordingDeviceType,
        fingerprint: AudioFingerprint,
        timestamp: f64,
    ) -> SpeakerAssignment {
        let speaker_number = self.speakers.len() + 1;
        let id = match source {
            RecordingDeviceType::Microphone => format!("local-speaker-{}", speaker_number),
            RecordingDeviceType::System => format!("remote-speaker-{}", speaker_number),
        };
        let label = match source {
            RecordingDeviceType::Microphone if speaker_number == 1 => "Local speaker".to_string(),
            RecordingDeviceType::Microphone => format!("Local speaker {}", speaker_number),
            RecordingDeviceType::System => format!("Remote speaker {}", speaker_number),
        };

        self.speakers.push(SpeakerProfile {
            id: id.clone(),
            label: label.clone(),
            fingerprint,
            segments: 1,
            last_seen_at: timestamp,
        });
        self.last_speaker_id = Some(id.clone());

        SpeakerAssignment {
            speaker: label,
            speaker_id: id,
        }
    }

    fn last_assignment(&self) -> Option<SpeakerAssignment> {
        let last_speaker_id = self.last_speaker_id.as_ref()?;
        let speaker = self
            .speakers
            .iter()
            .find(|speaker| &speaker.id == last_speaker_id)?;

        Some(SpeakerAssignment {
            speaker: speaker.label.clone(),
            speaker_id: speaker.id.clone(),
        })
    }

    fn default_assignment(&mut self, source: &RecordingDeviceType) -> SpeakerAssignment {
        if let Some(existing) = self.last_assignment() {
            return existing;
        }

        let id = match source {
            RecordingDeviceType::Microphone => "local-speaker-1",
            RecordingDeviceType::System => "remote-speaker-1",
        };
        let speaker = match source {
            RecordingDeviceType::Microphone => "Local speaker",
            RecordingDeviceType::System => "Remote speaker 1",
        };

        SpeakerAssignment {
            speaker: speaker.to_string(),
            speaker_id: id.to_string(),
        }
    }
}

fn sanitize_sample(sample: f32) -> f32 {
    if sample.is_finite() {
        sample.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

fn envelope_variance(samples: &[f32]) -> f32 {
    const WINDOWS: usize = 8;
    if samples.len() < WINDOWS {
        return 0.0;
    }

    let window_size = samples.len() / WINDOWS;
    let mut energies = Vec::with_capacity(WINDOWS);

    for index in 0..WINDOWS {
        let start = index * window_size;
        let end = if index == WINDOWS - 1 {
            samples.len()
        } else {
            (index + 1) * window_size
        };
        let window = &samples[start..end];
        let rms = (window
            .iter()
            .map(|sample| sanitize_sample(*sample).powi(2))
            .sum::<f32>()
            / window.len().max(1) as f32)
            .sqrt();
        energies.push(rms);
    }

    let mean = energies.iter().sum::<f32>() / energies.len() as f32;
    if mean <= 0.0001 {
        return 0.0;
    }

    let variance = energies
        .iter()
        .map(|energy| {
            let delta = energy - mean;
            delta * delta
        })
        .sum::<f32>()
        / energies.len() as f32;

    (variance.sqrt() / mean).clamp(0.0, 1.0)
}

fn pitch_proxy(samples: &[f32], sample_rate: u32) -> f32 {
    if sample_rate == 0 || samples.len() < 1024 {
        return 0.0;
    }

    let step = (sample_rate / 8_000).max(1) as usize;
    let downsampled: Vec<f32> = samples
        .iter()
        .step_by(step)
        .take(8_000)
        .map(|sample| sanitize_sample(*sample))
        .collect();

    if downsampled.len() < 512 {
        return 0.0;
    }

    let effective_rate = sample_rate as usize / step;
    let min_lag = (effective_rate / 350).max(1);
    let max_lag = (effective_rate / 75)
        .min(downsampled.len() / 2)
        .max(min_lag + 1);

    let mut best_lag = min_lag;
    let mut best_score = 0.0f32;

    for lag in min_lag..=max_lag {
        let mut correlation = 0.0f32;
        let mut energy_a = 0.0f32;
        let mut energy_b = 0.0f32;

        for index in lag..downsampled.len() {
            let a = downsampled[index];
            let b = downsampled[index - lag];
            correlation += a * b;
            energy_a += a * a;
            energy_b += b * b;
        }

        let score = correlation / (energy_a.sqrt() * energy_b.sqrt()).max(0.0001);
        if score > best_score {
            best_score = score;
            best_lag = lag;
        }
    }

    let frequency = effective_rate as f32 / best_lag as f32;
    ((frequency - 75.0) / (350.0 - 75.0)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_wave(frequency: f32, seconds: f32) -> Vec<f32> {
        let sample_rate = 16_000.0;
        let samples = (seconds * sample_rate) as usize;
        (0..samples)
            .map(|index| {
                let t = index as f32 / sample_rate;
                (t * frequency * std::f32::consts::TAU).sin() * 0.2
            })
            .collect()
    }

    #[test]
    fn keeps_same_remote_speaker_for_similar_audio() {
        let mut diarizer = SpeakerDiarizer::new();
        let first = diarizer.assign(
            &RecordingDeviceType::System,
            &sine_wave(170.0, 1.0),
            16_000,
            0.0,
        );
        let second = diarizer.assign(
            &RecordingDeviceType::System,
            &sine_wave(172.0, 1.0),
            16_000,
            1.2,
        );

        assert_eq!(first.speaker_id, second.speaker_id);
        assert_eq!("Remote speaker 1", second.speaker);
    }

    #[test]
    fn creates_new_remote_speaker_for_different_audio() {
        let mut diarizer = SpeakerDiarizer::new();
        let first = diarizer.assign(
            &RecordingDeviceType::System,
            &sine_wave(120.0, 1.0),
            16_000,
            0.0,
        );
        let second = diarizer.assign(
            &RecordingDeviceType::System,
            &sine_wave(260.0, 1.0),
            16_000,
            1.2,
        );

        assert_ne!(first.speaker_id, second.speaker_id);
        assert_eq!("Remote speaker 2", second.speaker);
    }

    #[test]
    fn short_audio_uses_previous_assignment() {
        let mut diarizer = SpeakerDiarizer::new();
        let first = diarizer.assign(
            &RecordingDeviceType::Microphone,
            &sine_wave(160.0, 1.0),
            16_000,
            0.0,
        );
        let short = diarizer.assign(
            &RecordingDeviceType::Microphone,
            &sine_wave(310.0, 0.2),
            16_000,
            1.2,
        );

        assert_eq!(first.speaker_id, short.speaker_id);
        assert_eq!("Local speaker", short.speaker);
    }
}
