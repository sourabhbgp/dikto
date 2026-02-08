pub mod audio;
pub mod clipboard;
pub mod config;
pub mod models;
pub mod transcribe;
pub mod vad;

use audio::{AudioCapture, AudioCaptureConfig, AudioError};
use config::SottoConfig;
use models::ModelError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info};
use transcribe::{TranscribeConfig, TranscribeError, WhisperEngine};
use vad::{VadConfig, VadError, VadEvent, VadProcessor};

/// Errors from the Sotto engine.
#[derive(Debug, Error)]
pub enum SottoError {
    #[error("Audio error: {0}")]
    Audio(#[from] AudioError),
    #[error("VAD error: {0}")]
    Vad(#[from] VadError),
    #[error("Transcription error: {0}")]
    Transcribe(#[from] TranscribeError),
    #[error("Model error: {0}")]
    Model(#[from] ModelError),
    #[error("No model loaded. Run: sotto --setup")]
    NoModel,
    #[error("Already recording")]
    AlreadyRecording,
    #[error("Config error: {0}")]
    Config(String),
}

/// Recording state enum.
#[derive(Debug, Clone, PartialEq)]
pub enum RecordingState {
    Idle,
    Listening,
    Processing,
    Done { text: String },
    Error { message: String },
}

/// Callbacks for transcription events.
pub trait TranscriptionCallback: Send + Sync {
    fn on_partial(&self, text: &str);
    fn on_final_segment(&self, text: &str);
    fn on_silence(&self);
    fn on_error(&self, error: &str);
    fn on_state_change(&self, state: &RecordingState);
}

/// Configuration for a listening session.
#[derive(Debug, Clone)]
pub struct ListenConfig {
    pub language: String,
    pub max_duration: u32,
    pub silence_duration_ms: u32,
    pub speech_threshold: f32,
    pub step_ms: u32,
    pub length_ms: u32,
    pub keep_ms: u32,
}

impl Default for ListenConfig {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            max_duration: 30,
            silence_duration_ms: 1500,
            speech_threshold: 0.5,
            step_ms: 3000,
            length_ms: 5000,
            keep_ms: 200,
        }
    }
}

impl From<&SottoConfig> for ListenConfig {
    fn from(cfg: &SottoConfig) -> Self {
        Self {
            language: cfg.language.clone(),
            max_duration: cfg.max_duration,
            silence_duration_ms: cfg.silence_duration_ms,
            speech_threshold: cfg.speech_threshold,
            ..Default::default()
        }
    }
}

/// Handle to stop a running recording session.
pub struct SessionHandle {
    stop_flag: Arc<AtomicBool>,
}

impl SessionHandle {
    /// Stop the recording session.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }

    /// Check if the session is still active.
    pub fn is_active(&self) -> bool {
        !self.stop_flag.load(Ordering::Relaxed)
    }
}

/// The main Sotto engine. Keeps the whisper model loaded in memory.
pub struct SottoEngine {
    engine: Option<WhisperEngine>,
    config: SottoConfig,
    recording: Arc<AtomicBool>,
}

impl SottoEngine {
    /// Create a new SottoEngine. Does NOT load the model yet.
    pub fn new() -> Self {
        let config = config::load_config();
        Self {
            engine: None,
            config,
            recording: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Load the configured model. Call this once at startup.
    pub fn load_model(&mut self) -> Result<(), SottoError> {
        let model_name = &self.config.model_name;
        let path = models::model_path(model_name).ok_or(SottoError::NoModel)?;

        if !path.exists() {
            return Err(SottoError::NoModel);
        }

        let engine = WhisperEngine::load(&path, cfg!(feature = "metal"))?;
        self.engine = Some(engine);
        info!("Model '{}' loaded and ready", model_name);
        Ok(())
    }

    /// Switch to a different model (hot-swap).
    pub fn switch_model(&mut self, model_name: &str) -> Result<(), SottoError> {
        let path = models::model_path(model_name).ok_or(SottoError::NoModel)?;
        if !path.exists() {
            return Err(SottoError::NoModel);
        }

        let engine = WhisperEngine::load(&path, cfg!(feature = "metal"))?;
        self.engine = Some(engine);
        self.config.model_name = model_name.to_string();
        config::save_config(&self.config).map_err(|e| SottoError::Config(e.to_string()))?;
        info!("Switched to model '{}'", model_name);
        Ok(())
    }

    /// Start listening and transcribing. Returns a handle to stop the session,
    /// and a future that resolves to the final transcript.
    pub fn start_listening(
        &self,
        listen_config: ListenConfig,
        callback: Arc<dyn TranscriptionCallback>,
    ) -> Result<(SessionHandle, tokio::task::JoinHandle<Result<String, SottoError>>), SottoError>
    {
        if self.recording.load(Ordering::Relaxed) {
            return Err(SottoError::AlreadyRecording);
        }

        let engine = self.engine.as_ref().ok_or(SottoError::NoModel)?;

        let transcribe_config = TranscribeConfig {
            language: listen_config.language.clone(),
            step_ms: listen_config.step_ms,
            length_ms: listen_config.length_ms,
            keep_ms: listen_config.keep_ms,
            ..Default::default()
        };

        let mut session = engine.create_session(transcribe_config)?;

        let stop_flag = Arc::new(AtomicBool::new(false));
        let handle = SessionHandle {
            stop_flag: stop_flag.clone(),
        };

        let recording = self.recording.clone();
        recording.store(true, Ordering::Relaxed);

        let max_duration = listen_config.max_duration;
        let silence_duration_ms = listen_config.silence_duration_ms;
        let speech_threshold = listen_config.speech_threshold;

        let join_handle = tokio::task::spawn_blocking(move || {
            let result = run_pipeline(
                &mut session,
                stop_flag,
                recording.clone(),
                callback.clone(),
                max_duration,
                silence_duration_ms,
                speech_threshold,
            );

            recording.store(false, Ordering::Relaxed);

            match &result {
                Ok(text) => {
                    callback.on_state_change(&RecordingState::Done {
                        text: text.clone(),
                    });
                }
                Err(e) => {
                    callback.on_state_change(&RecordingState::Error {
                        message: e.to_string(),
                    });
                }
            }

            result
        });

        Ok((handle, join_handle))
    }

    /// Get current config.
    pub fn get_config(&self) -> &SottoConfig {
        &self.config
    }

    /// Update config and save.
    pub fn update_config(&mut self, config: SottoConfig) -> Result<(), SottoError> {
        config::save_config(&config).map_err(|e| SottoError::Config(e.to_string()))?;
        self.config = config;
        Ok(())
    }

    /// List available models.
    pub fn list_models(&self) -> Vec<(models::ModelInfo, bool)> {
        models::list_models()
    }

    /// Check if currently recording.
    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::Relaxed)
    }
}

/// The main recording + transcription pipeline, runs on a blocking thread.
fn run_pipeline(
    session: &mut transcribe::TranscribeSession,
    stop_flag: Arc<AtomicBool>,
    _recording: Arc<AtomicBool>,
    callback: Arc<dyn TranscriptionCallback>,
    max_duration: u32,
    silence_duration_ms: u32,
    speech_threshold: f32,
) -> Result<String, SottoError> {
    callback.on_state_change(&RecordingState::Listening);

    // Start audio capture
    let mut capture = AudioCapture::start(AudioCaptureConfig::default())?;

    // Initialize VAD
    let vad_config = VadConfig {
        speech_threshold,
        silence_duration_ms,
        ..Default::default()
    };
    let mut vad = VadProcessor::new(vad_config)?;
    let chunk_size = vad.chunk_size();

    let start_time = std::time::Instant::now();
    let max_dur = std::time::Duration::from_secs(max_duration as u64);

    let mut vad_buffer: Vec<f32> = Vec::new();
    let mut speech_detected = false;

    loop {
        // Check stop conditions
        if stop_flag.load(Ordering::Relaxed) {
            info!("Stop requested");
            break;
        }
        if start_time.elapsed() >= max_dur {
            info!("Max duration reached");
            break;
        }

        // Read samples from mic
        let samples = capture.read_samples();
        if samples.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        }

        // Feed to VAD in chunks
        vad_buffer.extend_from_slice(&samples);

        while vad_buffer.len() >= chunk_size {
            let chunk: Vec<f32> = vad_buffer.drain(..chunk_size).collect();

            match vad.process_chunk(&chunk)? {
                VadEvent::SpeechStart => {
                    speech_detected = true;
                    debug!("Speech detected, starting transcription");
                }
                VadEvent::SpeechEnd => {
                    if speech_detected {
                        callback.on_silence();
                        info!("Speech ended (silence detected)");

                        // Flush remaining audio
                        callback.on_state_change(&RecordingState::Processing);
                        let final_segments = session.flush()?;
                        for seg in &final_segments {
                            callback.on_final_segment(&seg.text);
                        }

                        capture.stop();
                        return Ok(session.transcript());
                    }
                }
                VadEvent::SpeechContinue | VadEvent::Silence => {}
            }
        }

        // Feed audio to transcription engine
        if speech_detected {
            let segments = session.feed_samples(&samples)?;
            for seg in &segments {
                callback.on_partial(&seg.text);
            }
        }
    }

    // Flush on stop
    callback.on_state_change(&RecordingState::Processing);
    let final_segments = session.flush()?;
    for seg in &final_segments {
        callback.on_final_segment(&seg.text);
    }

    capture.stop();
    Ok(session.transcript())
}
