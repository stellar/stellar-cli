use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    #[error("Template error: {0}")]
    Template(String),

    #[error("Render error: {0}")]
    Render(#[from] handlebars::RenderError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Smart wallet error: {0}")]
    SmartWalletError(String),

    #[error("Policy generation error: {0}")]
    PolicyGenerationError(String),
} 