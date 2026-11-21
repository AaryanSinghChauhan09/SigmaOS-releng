//! Error types for the releng component.

/// Errors that can occur in releng tooling.
#[derive(Debug, thiserror::Error)]
pub enum RelengError {
    /// HTTP fetch failed (releases/mirrors APIs).
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON parse failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Rendering or validation failed.
    #[error("render error: {0}")]
    Render(String),

    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
