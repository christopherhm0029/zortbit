// On-device OCR via the Apple Vision sidecar (src-tauri/ocr/zb_ocr.swift).
// A subprocess call, so it's safe from any thread and decoupled from our runtime.

use std::path::Path;
use std::process::Command;

/// Extract text from an image. Returns None on failure or empty result
/// (e.g. icons/photos with no text) — callers fall back to type-based filing.
pub fn ocr_image(bin: &Path, image: &Path) -> Option<String> {
    let out = Command::new(bin).arg(image).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}
