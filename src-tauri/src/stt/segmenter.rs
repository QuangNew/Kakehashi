//! Cut capture audio once, using sample counts rather than inference feedback.
use super::provider::DualPassConfig;

pub const SAMPLE_RATE: usize = 16_000;
const FRAME_SAMPLES: usize = 320; // 20 ms
const PRE_ROLL_SAMPLES: usize = 1_600;
const SPEECH_RMS: f64 = 200.0;

#[derive(Debug)]
pub(super) struct AudioSegment {
    pub samples: Vec<i16>,
    pub start_ms: u64,
}

impl AudioSegment {
    pub fn duration_ms(&self) -> u64 {
        self.samples.len() as u64 / 16
    }
    pub fn end_ms(&self) -> u64 {
        self.start_ms + self.duration_ms()
    }
}

#[derive(Default)]
pub(super) struct Segmenter {
    audio: Vec<i16>,
    start_ms: u64,
    silence_samples: usize,
    pre_roll: Vec<i16>,
}

impl Segmenter {
    pub fn push(
        &mut self,
        samples: &[i16],
        start_ms: u64,
        config: &DualPassConfig,
    ) -> Vec<AudioSegment> {
        let config = config.normalized();
        let min_samples = (config.short_chunk_secs * SAMPLE_RATE as f32) as usize;
        let max_samples = (config.long_chunk_secs * SAMPLE_RATE as f32) as usize;
        let pause_samples = (config.pause_secs * SAMPLE_RATE as f32) as usize;
        let mut segments = Vec::new();
        let mut offset = 0;
        while offset < samples.len() {
            // Respect a cap even when the capture callback supplies a large buffer.
            let room = if self.audio.is_empty() {
                max_samples
            } else {
                max_samples.saturating_sub(self.audio.len())
            };
            if room == 0 {
                if let Some(segment) = self.flush() {
                    segments.push(segment);
                }
                continue;
            }
            let end = (offset + FRAME_SAMPLES.min(room)).min(samples.len());
            let frame = &samples[offset..end];
            let energy =
                frame.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / frame.len() as f64;
            let speech = energy > SPEECH_RMS * SPEECH_RMS;
            if self.audio.is_empty() {
                if !speech {
                    self.pre_roll.extend_from_slice(frame);
                    if self.pre_roll.len() > PRE_ROLL_SAMPLES {
                        self.pre_roll
                            .drain(..self.pre_roll.len() - PRE_ROLL_SAMPLES);
                    }
                    offset = end;
                    continue;
                }
                self.start_ms =
                    (start_ms + offset as u64 / 16).saturating_sub(self.pre_roll.len() as u64 / 16);
                self.audio = std::mem::take(&mut self.pre_roll);
            }
            self.audio.extend_from_slice(frame);
            self.silence_samples = if speech {
                0
            } else {
                self.silence_samples + frame.len()
            };
            if self.audio.len() >= max_samples
                || (self.audio.len() >= min_samples && self.silence_samples >= pause_samples)
            {
                if let Some(segment) = self.flush() {
                    segments.push(segment);
                }
            }
            offset = end;
        }
        segments
    }

    pub fn flush(&mut self) -> Option<AudioSegment> {
        self.silence_samples = 0;
        if self.audio.is_empty() {
            return None;
        }
        Some(AudioSegment {
            samples: std::mem::take(&mut self.audio),
            start_ms: self.start_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uninterrupted_speech_is_finalizable_without_any_inference_feedback() {
        let mut segmenter = Segmenter::default();
        let audio = vec![1000; 16_000 * 30];
        let segments = segmenter.push(&audio, 500, &DualPassConfig::default());
        assert_eq!(segments.len(), 10);
        assert_eq!(
            segments.iter().map(|s| s.samples.len()).sum::<usize>(),
            audio.len()
        );
        for (i, segment) in segments.iter().enumerate() {
            assert_eq!(segment.start_ms, 500 + i as u64 * 3000);
            assert_eq!(segment.duration_ms(), 3000);
        }
    }

    #[test]
    fn real_silence_closes_a_segment_without_waiting_for_recognition() {
        let mut segmenter = Segmenter::default();
        assert!(segmenter
            .push(&vec![1200; 16_000], 0, &DualPassConfig::default())
            .is_empty());
        let segments = segmenter.push(&vec![0; 8000], 1000, &DualPassConfig::default());
        assert_eq!(segments.len(), 1);
        assert!((1400..=1500).contains(&segments[0].duration_ms()));
        assert!(segmenter.flush().is_none());
    }

    #[test]
    fn silence_never_queues_inference_and_short_last_utterance_can_be_flushed() {
        let mut segmenter = Segmenter::default();
        assert!(segmenter
            .push(&vec![0; 160_000], 0, &DualPassConfig::default())
            .is_empty());
        assert!(segmenter
            .push(&vec![900; 4800], 10_000, &DualPassConfig::default())
            .is_empty());
        let final_segment = segmenter.flush().unwrap();
        assert_eq!(final_segment.start_ms, 9900);
        assert_eq!(final_segment.duration_ms(), 400);
    }

    #[test]
    fn runtime_config_changes_cannot_create_a_zero_length_loop() {
        let mut segmenter = Segmenter::default();
        let config = DualPassConfig {
            short_chunk_secs: f32::NAN,
            long_chunk_secs: 0.0,
            pause_secs: f32::INFINITY,
        };
        let result = segmenter.push(&vec![1000; 48_000], 0, &config);
        assert!(!result.is_empty());
        assert!(result.iter().all(|s| !s.samples.is_empty()));
    }
}
