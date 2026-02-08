use crate::config::models_dir;
use std::path::PathBuf;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("Model '{0}' not found. Available: {1}")]
    NotFound(String, String),
    #[error("Download failed: {0}")]
    DownloadFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

/// Model registry entry.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: &'static str,
    pub filename: &'static str,
    pub size_mb: u32,
    pub url: &'static str,
    pub description: &'static str,
}

/// Hardcoded model registry — same models as v1.
pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        name: "tiny.en",
        filename: "ggml-tiny.en.bin",
        size_mb: 75,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
        description: "Fastest, least accurate (English only)",
    },
    ModelInfo {
        name: "base.en",
        filename: "ggml-base.en.bin",
        size_mb: 142,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
        description: "Good balance of speed and accuracy (English only)",
    },
    ModelInfo {
        name: "small.en",
        filename: "ggml-small.en.bin",
        size_mb: 466,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin",
        description: "Higher accuracy, slower (English only)",
    },
    ModelInfo {
        name: "medium.en",
        filename: "ggml-medium.en.bin",
        size_mb: 1500,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.en.bin",
        description: "Highest accuracy, slowest (English only)",
    },
];

/// Look up model info by name.
pub fn find_model(name: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.name == name)
}

/// Get the local file path for a model.
pub fn model_path(name: &str) -> Option<PathBuf> {
    find_model(name).map(|m| models_dir().join(m.filename))
}

/// Check if a model is downloaded.
pub fn is_model_downloaded(name: &str) -> bool {
    model_path(name).is_some_and(|p| p.exists())
}

/// List all models with their download status.
pub fn list_models() -> Vec<(ModelInfo, bool)> {
    MODELS
        .iter()
        .map(|m| (m.clone(), is_model_downloaded(m.name)))
        .collect()
}

/// Download a model with progress callback.
/// `on_progress` receives (bytes_downloaded, total_bytes).
pub async fn download_model<F>(
    name: &str,
    on_progress: F,
) -> Result<PathBuf, ModelError>
where
    F: Fn(u64, u64) + Send + 'static,
{
    let model = find_model(name).ok_or_else(|| {
        let available = MODELS
            .iter()
            .map(|m| m.name)
            .collect::<Vec<_>>()
            .join(", ");
        ModelError::NotFound(name.to_string(), available)
    })?;

    let dir = models_dir();
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(model.filename);

    // Skip if already exists
    if dest.exists() {
        info!("Model {} already downloaded at {}", name, dest.display());
        return Ok(dest);
    }

    info!("Downloading {} ({} MB) from {}", name, model.size_mb, model.url);

    let response = reqwest::get(model.url).await?;

    if !response.status().is_success() {
        return Err(ModelError::DownloadFailed(format!(
            "HTTP {}",
            response.status()
        )));
    }

    let total = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    // Write to temp file first, then rename (atomic)
    let temp_dest = dir.join(format!("{}.downloading", model.filename));

    use futures::StreamExt;
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(&temp_dest).await.map_err(|e| {
        ModelError::Io(e)
    })?;

    use tokio::io::AsyncWriteExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await.map_err(ModelError::Io)?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total);
    }
    file.flush().await.map_err(ModelError::Io)?;
    drop(file);

    // Rename to final destination
    tokio::fs::rename(&temp_dest, &dest)
        .await
        .map_err(ModelError::Io)?;

    info!("Downloaded {} to {}", name, dest.display());
    Ok(dest)
}

/// Delete a downloaded model.
pub fn delete_model(name: &str) -> Result<(), ModelError> {
    if let Some(path) = model_path(name) {
        if path.exists() {
            std::fs::remove_file(&path)?;
            info!("Deleted model {} at {}", name, path.display());
        } else {
            warn!("Model {} not found at {}", name, path.display());
        }
        Ok(())
    } else {
        let available = MODELS
            .iter()
            .map(|m| m.name)
            .collect::<Vec<_>>()
            .join(", ");
        Err(ModelError::NotFound(name.to_string(), available))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_model() {
        assert!(find_model("base.en").is_some());
        assert!(find_model("nonexistent").is_none());
    }

    #[test]
    fn test_model_registry() {
        assert_eq!(MODELS.len(), 4);
        assert_eq!(MODELS[0].name, "tiny.en");
        assert_eq!(MODELS[1].name, "base.en");
    }

    #[test]
    fn test_model_path() {
        let path = model_path("base.en").unwrap();
        assert!(path.to_string_lossy().contains("ggml-base.en.bin"));
    }
}
