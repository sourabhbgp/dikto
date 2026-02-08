use std::process::Command;
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("Failed to copy to clipboard: {0}")]
    CopyFailed(String),
    #[error("Failed to simulate paste: {0}")]
    PasteFailed(String),
}

/// Copy text to the macOS clipboard via pbcopy.
pub fn copy_to_clipboard(text: &str) -> Result<(), ClipboardError> {
    use std::io::Write;

    let mut child = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| ClipboardError::CopyFailed(e.to_string()))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| ClipboardError::CopyFailed(e.to_string()))?;
    }

    child
        .wait()
        .map_err(|e| ClipboardError::CopyFailed(e.to_string()))?;

    debug!("Copied {} chars to clipboard", text.len());
    Ok(())
}

/// Simulate Cmd+V paste using osascript (AppleScript).
pub fn simulate_paste() -> Result<(), ClipboardError> {
    let status = Command::new("osascript")
        .args([
            "-e",
            r#"tell application "System Events" to keystroke "v" using command down"#,
        ])
        .status()
        .map_err(|e| ClipboardError::PasteFailed(e.to_string()))?;

    if !status.success() {
        return Err(ClipboardError::PasteFailed(
            "osascript exited with non-zero status".to_string(),
        ));
    }

    debug!("Simulated paste (Cmd+V)");
    Ok(())
}

/// Copy text to clipboard, wait briefly, then simulate paste.
pub async fn copy_and_paste(text: &str) -> Result<(), ClipboardError> {
    copy_to_clipboard(text)?;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    simulate_paste()
}
