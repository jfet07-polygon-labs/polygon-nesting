use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::{ArchiveIneligibilityReason, ProtocolVersion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    MalformedInput {
        message: String,
    },
    UnsupportedVersion {
        expected: ProtocolVersion,
        received: u32,
    },
    Validation {
        field: String,
        message: String,
    },
    InvalidDecimalString {
        field: String,
        value: String,
    },
    Encoding {
        message: String,
    },
}

impl ProtocolError {
    pub(crate) fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
        }
    }
}

impl Display for ProtocolError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedInput { message } => {
                write!(formatter, "malformed protocol input: {message}")
            }
            Self::UnsupportedVersion { expected, received } => write!(
                formatter,
                "unsupported protocol version {received}; expected {}",
                expected.get()
            ),
            Self::Validation { field, message } => {
                write!(formatter, "invalid protocol field {field}: {message}")
            }
            Self::InvalidDecimalString { field, value } => write!(
                formatter,
                "protocol field {field} must be a canonical decimal string, received {value:?}"
            ),
            Self::Encoding { message } => write!(formatter, "protocol encoding failed: {message}"),
        }
    }
}

impl Error for ProtocolError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineErrorCode {
    MalformedInput,
    ProtocolVersionMismatch,
    ArchiveIneligible,
    InvalidGeometry,
    Cancelled,
    DeadlineExceeded,
    EngineFailure,
    InternalFailure,
    IoFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineError {
    pub category: EngineErrorCode,
    pub operation: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub context: BTreeMap<String, String>,
}

impl EngineError {
    pub fn new(
        category: EngineErrorCode,
        operation: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            category,
            operation: operation.into(),
            message: message.into(),
            context: BTreeMap::new(),
        }
    }

    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    pub fn archive_ineligible(reason: ArchiveIneligibilityReason) -> Self {
        Self::new(
            EngineErrorCode::ArchiveIneligible,
            "archive-ineligible",
            format!(
                "request is not eligible for the supported polygon nesting archive path: {}",
                reason.as_str()
            ),
        )
        .with_context("reason", reason.as_str())
    }
}
