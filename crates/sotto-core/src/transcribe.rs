use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tracing::info;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

#[derive(Debug, Error)]
pub enum TranscribeError {
    #[error("Failed to load whisper model: {0}")]
    ModelLoad(String),
    #[error("Failed to create whisper state: {0}")]
    StateCreate(String),
    #[error("Inference failed: {0}")]
    Inference(String),
    #[error("Model not loaded")]
    NotLoaded,
}

/// Configuration for transcription.
#[derive(Debug, Clone)]
pub struct TranscribeConfig {
    /// Language code (e.g., "en").
    pub language: String,
    /// Sliding window step in ms (how often to run inference).
    pub step_ms: u32,
    /// Sliding window length in ms (audio window size for inference).
    pub length_ms: u32,
    /// Overlap to keep from previous window in ms.
    pub keep_ms: u32,
    /// Number of threads for whisper inference.
    pub n_threads: i32,
}

impl Default for TranscribeConfig {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            step_ms: 3000,
            length_ms: 5000,
            keep_ms: 200,
            n_threads: 4,
        }
    }
}

/// A segment of transcribed text.
#[derive(Debug, Clone)]
pub struct TranscriptSegment {
    pub text: String,
    pub is_final: bool,
}

/// Whisper engine that keeps the model loaded in memory.
pub struct WhisperEngine {
    ctx: Arc<WhisperContext>,
}

impl WhisperEngine {
    /// Load a whisper model from disk. This is the expensive operation
    /// that we only do once.
    pub fn load(model_path: &Path, use_gpu: bool) -> Result<Self, TranscribeError> {
        info!("Loading whisper model from {}", model_path.display());

        let mut params = WhisperContextParameters::default();
        params.use_gpu(use_gpu);

        let ctx = WhisperContext::new_with_params(
            model_path.to_str().unwrap_or(""),
            params,
        )
        .map_err(|e| TranscribeError::ModelLoad(e.to_string()))?;

        info!("Whisper model loaded successfully (gpu={})", use_gpu);

        Ok(Self {
            ctx: Arc::new(ctx),
        })
    }

    /// Create a new transcription session with the loaded model.
    pub fn create_session(&self, config: TranscribeConfig) -> Result<TranscribeSession, TranscribeError> {
        let state = self
            .ctx
            .create_state()
            .map_err(|e| TranscribeError::StateCreate(e.to_string()))?;

        Ok(TranscribeSession {
            state,
            config,
            audio_buffer: Vec::new(),
            final_segments: Vec::new(),
            prompt_tokens: Vec::new(),
            samples_since_last_step: 0,
        })
    }
}

/// A transcription session that processes streaming audio.
/// Implements the same sliding window algorithm as v1's whisper-stream.
pub struct TranscribeSession {
    state: WhisperState,
    config: TranscribeConfig,
    /// Accumulated audio buffer.
    audio_buffer: Vec<f32>,
    /// Final (committed) transcript segments.
    final_segments: Vec<String>,
    /// Prompt tokens for context carryover.
    prompt_tokens: Vec<i32>,
    /// Samples accumulated since last inference step.
    samples_since_last_step: usize,
}

// WhisperState isn't Send by default but we need it for async.
// Safety: we only access it from one thread at a time via spawn_blocking.
unsafe impl Send for TranscribeSession {}

impl TranscribeSession {
    /// Feed audio samples (16kHz mono f32) and return any new segments.
    /// This implements the sliding window: it only runs inference
    /// when enough new audio has accumulated (step_ms worth).
    pub fn feed_samples(&mut self, samples: &[f32]) -> Result<Vec<TranscriptSegment>, TranscribeError> {
        self.audio_buffer.extend_from_slice(samples);
        self.samples_since_last_step += samples.len();

        let step_samples = self.config.step_ms as usize * 16; // 16 samples per ms at 16kHz
        if self.samples_since_last_step < step_samples {
            return Ok(Vec::new());
        }

        self.samples_since_last_step = 0;
        self.run_inference()
    }

    /// Run inference on the current audio buffer using the sliding window.
    fn run_inference(&mut self) -> Result<Vec<TranscriptSegment>, TranscribeError> {
        let length_samples = self.config.length_ms as usize * 16;
        let keep_samples = self.config.keep_ms as usize * 16;

        // Extract the window: last `length_ms` of audio
        let window = if self.audio_buffer.len() > length_samples {
            &self.audio_buffer[self.audio_buffer.len() - length_samples..]
        } else {
            &self.audio_buffer
        };

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(&self.config.language));
        params.set_n_threads(self.config.n_threads);
        params.set_no_context(false);
        params.set_single_segment(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Set prompt tokens for context carryover
        if !self.prompt_tokens.is_empty() {
            params.set_tokens(&self.prompt_tokens);
        }

        self.state
            .full(params, window)
            .map_err(|e: whisper_rs::WhisperError| TranscribeError::Inference(e.to_string()))?;

        let n_segments = self.state.full_n_segments();

        let mut segments = Vec::new();
        for i in 0..n_segments {
            if let Some(segment) = self.state.get_segment(i) {
                let text = segment.to_str_lossy()
                    .map_err(|e| TranscribeError::Inference(e.to_string()))?;
                let text = text.trim().to_string();
                if !text.is_empty() {
                    segments.push(TranscriptSegment {
                        text,
                        is_final: false,
                    });
                }
            }
        }

        // Save prompt tokens for context carryover
        if n_segments > 0 {
            let last = n_segments - 1;
            if let Some(segment) = self.state.get_segment(last) {
                let n_tokens = segment.n_tokens();
                self.prompt_tokens.clear();
                for j in 0..n_tokens {
                    if let Some(token) = segment.get_token(j) {
                        self.prompt_tokens.push(token.token_id());
                    }
                }
            }
        }

        // Trim buffer: keep only `keep_ms` overlap
        if self.audio_buffer.len() > keep_samples {
            let trim_to = self.audio_buffer.len() - keep_samples;
            self.audio_buffer.drain(..trim_to);
        }

        Ok(segments)
    }

    /// Flush remaining audio and return final transcript.
    pub fn flush(&mut self) -> Result<Vec<TranscriptSegment>, TranscribeError> {
        if self.audio_buffer.is_empty() {
            return Ok(Vec::new());
        }

        let segments = self.run_inference()?;
        let final_segments: Vec<TranscriptSegment> = segments
            .into_iter()
            .map(|mut s| {
                s.is_final = true;
                s
            })
            .collect();

        // Add to committed segments
        for seg in &final_segments {
            self.final_segments.push(seg.text.clone());
        }

        self.audio_buffer.clear();
        self.samples_since_last_step = 0;

        Ok(final_segments)
    }

    /// Get the full transcript so far (all final segments joined).
    pub fn transcript(&self) -> String {
        self.final_segments.join(" ")
    }

    /// Get accumulated audio buffer length in seconds.
    pub fn buffer_duration_secs(&self) -> f32 {
        self.audio_buffer.len() as f32 / 16000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcribe_config_defaults() {
        let config = TranscribeConfig::default();
        assert_eq!(config.step_ms, 3000);
        assert_eq!(config.length_ms, 5000);
        assert_eq!(config.keep_ms, 200);
        assert_eq!(config.language, "en");
    }
}
