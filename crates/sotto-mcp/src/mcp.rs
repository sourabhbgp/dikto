use rmcp::{
    handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::*,
    service::{Peer, RoleServer},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
    transport::stdio,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sotto_core::{
    ListenConfig, RecordingState, SottoEngine, SottoError, TranscriptionCallback,
};
use std::sync::{Arc, Mutex};
use tracing::{error, info};

/// Parameters for the `listen` tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ListenParams {
    /// Maximum recording duration in seconds (1-120, default 30).
    #[schemars(description = "Maximum recording duration in seconds")]
    max_duration: Option<u32>,
    /// Language code for transcription (default: en).
    #[schemars(description = "Language code for transcription (default: en)")]
    language: Option<String>,
}

/// MCP callback that forwards events as progress notifications.
struct McpCallback {
    peer: Peer<RoleServer>,
    progress_token: Option<ProgressToken>,
    step: Mutex<f64>,
}

impl TranscriptionCallback for McpCallback {
    fn on_partial(&self, text: &str) {
        let mut step = self.step.lock().unwrap();
        *step += 1.0;
        let s = *step;
        let peer = self.peer.clone();
        let token = self.progress_token.clone();
        let text = text.to_string();
        tokio::spawn(async move {
            if let Some(token) = token {
                let _ = peer
                    .notify_progress(ProgressNotificationParam {
                        progress_token: token,
                        progress: s,
                        total: None,
                        message: Some(text),
                    })
                    .await;
            }
        });
    }

    fn on_final_segment(&self, text: &str) {
        let mut step = self.step.lock().unwrap();
        *step += 1.0;
        let s = *step;
        let peer = self.peer.clone();
        let token = self.progress_token.clone();
        let text = format!("[final] {text}");
        tokio::spawn(async move {
            if let Some(token) = token {
                let _ = peer
                    .notify_progress(ProgressNotificationParam {
                        progress_token: token,
                        progress: s,
                        total: None,
                        message: Some(text),
                    })
                    .await;
            }
        });
    }

    fn on_silence(&self) {
        info!("Silence detected");
    }

    fn on_error(&self, error: &str) {
        error!("Transcription error: {error}");
    }

    fn on_state_change(&self, state: &RecordingState) {
        info!("State changed: {state:?}");
    }
}

/// The Sotto MCP server.
#[derive(Clone)]
pub struct SottoServer {
    engine: Arc<Mutex<SottoEngine>>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl SottoServer {
    pub fn new() -> Result<Self, SottoError> {
        let mut engine = SottoEngine::new();

        // Try to load the model at startup
        if let Err(e) = engine.load_model() {
            tracing::warn!("Model not loaded at startup: {e}. Run: sotto --setup");
        }

        Ok(Self {
            engine: Arc::new(Mutex::new(engine)),
            tool_router: Self::tool_router(),
        })
    }

    /// Record audio from the microphone and transcribe it to text in real-time
    /// using whisper.cpp. Shows live text as you speak.
    #[tool(
        name = "listen",
        description = "Record audio from the microphone and transcribe it to text in real-time using whisper.cpp. Shows live text as you speak."
    )]
    async fn listen(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        Parameters(params): Parameters<ListenParams>,
    ) -> Result<CallToolResult, McpError> {
        let max_duration = params.max_duration.unwrap_or(30).clamp(1, 120);
        let language = params.language.unwrap_or_else(|| "en".to_string());

        let listen_config = {
            let engine = self.engine.lock().unwrap();
            let base = ListenConfig::from(engine.get_config());
            ListenConfig {
                language,
                max_duration,
                ..base
            }
        };

        let callback = Arc::new(McpCallback {
            peer: peer.clone(),
            progress_token: meta.get_progress_token(),
            step: Mutex::new(0.0),
        });

        // Start listening
        let (_handle, join) = {
            let engine = self.engine.lock().unwrap();
            engine.start_listening(listen_config, callback).map_err(|e| {
                match &e {
                    SottoError::NoModel => McpError::internal_error(
                        "No whisper model found. Run: sotto --setup",
                        None,
                    ),
                    SottoError::Audio(audio_err) => {
                        let msg = format!("{audio_err}");
                        if msg.contains("No input device") {
                            McpError::internal_error(
                                "No microphone found. Grant microphone access in System Settings > Privacy & Security > Microphone.",
                                None,
                            )
                        } else {
                            McpError::internal_error(msg, None)
                        }
                    }
                    _ => McpError::internal_error(e.to_string(), None),
                }
            })?
        };

        // Wait for completion
        let result = join
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        // Send final progress
        if let Some(token) = meta.get_progress_token() {
            let _ = peer
                .notify_progress(ProgressNotificationParam {
                    progress_token: token,
                    progress: 100.0,
                    total: Some(100.0),
                    message: Some("Transcription complete".into()),
                })
                .await;
        }

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

#[tool_handler]
impl ServerHandler for SottoServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Sotto is a voice-to-text tool. Use the `listen` tool to record audio and get a transcription."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "sotto".into(),
                title: None,
                version: env!("CARGO_PKG_VERSION").into(),
                icons: None,
                website_url: None,
            },
            ..Default::default()
        }
    }
}

/// Run the MCP server over stdio.
pub async fn run_mcp_server() -> anyhow::Result<()> {
    let server = SottoServer::new().map_err(|e| anyhow::anyhow!(e))?;
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
