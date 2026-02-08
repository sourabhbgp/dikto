//! CLI example that captures mic, runs VAD + whisper, and prints transcript.
//! Usage: cargo run --example listen

use sotto_core::{
    ListenConfig, RecordingState, SottoEngine, TranscriptionCallback,
};
use std::sync::Arc;

struct PrintCallback;

impl TranscriptionCallback for PrintCallback {
    fn on_partial(&self, text: &str) {
        eprint!("\r\x1b[K[partial] {text}");
    }

    fn on_final_segment(&self, text: &str) {
        eprintln!("\r\x1b[K[final] {text}");
    }

    fn on_silence(&self) {
        eprintln!("\r\x1b[K[silence detected]");
    }

    fn on_error(&self, error: &str) {
        eprintln!("\r\x1b[K[error] {error}");
    }

    fn on_state_change(&self, state: &RecordingState) {
        match state {
            RecordingState::Listening => eprintln!("[state] Listening..."),
            RecordingState::Processing => eprintln!("[state] Processing..."),
            RecordingState::Done { text } => {
                eprintln!("[state] Done!");
                println!("{text}");
            }
            RecordingState::Error { message } => eprintln!("[state] Error: {message}"),
            RecordingState::Idle => eprintln!("[state] Idle"),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sotto_core=debug".parse().unwrap()),
        )
        .init();

    let mut engine = SottoEngine::new();

    eprintln!("Loading model...");
    engine.load_model()?;
    eprintln!("Model loaded! Speak into your microphone (max 30s, or silence to stop).");

    let config = ListenConfig::default();
    let callback = Arc::new(PrintCallback);

    let (_handle, join) = engine.start_listening(config, callback)?;
    let result = join.await??;

    eprintln!("\nFinal transcript: {result}");
    Ok(())
}
