use std::fmt;

use crate::value::NiValue;

#[derive(Debug, Clone)]
pub struct NiRuntimeError {
    pub message: String,
    pub value: Option<NiValue>,
}

impl NiRuntimeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            value: None,
        }
    }

    pub fn from_value(value: NiValue) -> Self {
        let message = value.to_display_string();
        Self {
            message,
            value: Some(value),
        }
    }

    pub fn type_error(expected: &str, got: &NiValue) -> Self {
        Self::new(format!(
            "TypeError: expected {}, got {}",
            expected,
            got.type_name()
        ))
    }
}

impl fmt::Display for NiRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RuntimeError: {}", self.message)
    }
}

impl std::error::Error for NiRuntimeError {}

pub type NiResult<T> = Result<T, NiRuntimeError>;
