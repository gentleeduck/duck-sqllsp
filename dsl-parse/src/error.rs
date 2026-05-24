//! Parser error envelope. One per failed top-level statement.

use serde::Serialize;
use text_size::TextRange;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Error)]
#[error("parse error at {range:?}: {message}")]
pub struct ParseError {
  pub range: TextRange,
  pub message: String,
}
