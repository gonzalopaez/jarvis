use crate::{CoreRequest, API_VERSION};
use serde_json::Value;
use std::fmt;

const MAX_MESSAGE_BYTES: usize = 8_000;
const MAX_PARAMETERS_BYTES: usize = 16_384;
const MAX_ID_BYTES: usize = 128;
const MAX_FIELD_BYTES: usize = 128;
const MAX_PARAMETER_DEPTH: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub code: &'static str,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code)
    }
}

pub fn validate_request(request: &CoreRequest) -> Result<(), ValidationError> {
    if request.api_version != API_VERSION {
        return Err(error("UNSUPPORTED_API_VERSION"));
    }
    validate_identifier(&request.request_id)?;
    validate_identifier(&request.session_id)?;

    match request.kind.as_str() {
        "conversation" => {
            if request.action.is_some() {
                return Err(error("INVALID_REQUEST_SHAPE"));
            }
            let message = request
                .message
                .as_deref()
                .ok_or_else(|| error("MESSAGE_REQUIRED"))?;
            if message.trim().is_empty() || message.len() > MAX_MESSAGE_BYTES {
                return Err(error("INVALID_MESSAGE"));
            }
        }
        "action" => {
            if request.message.is_some() {
                return Err(error("INVALID_REQUEST_SHAPE"));
            }
            let action = request
                .action
                .as_ref()
                .ok_or_else(|| error("ACTION_REQUIRED"))?;
            validate_name(&action.capability)?;
            validate_name(&action.target)?;
            let size = serde_json::to_vec(&action.parameters)
                .map_err(|_| error("INVALID_PARAMETERS"))?
                .len();
            if size > MAX_PARAMETERS_BYTES {
                return Err(error("PARAMETERS_TOO_LARGE"));
            }
            inspect_value(&Value::Object(action.parameters.clone()), 0)?;
        }
        _ => return Err(error("UNKNOWN_REQUEST_KIND")),
    }
    Ok(())
}

fn validate_identifier(value: &str) -> Result<(), ValidationError> {
    if value.is_empty()
        || value.len() > MAX_ID_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
    {
        return Err(error("INVALID_CORRELATION_ID"));
    }
    Ok(())
}

fn validate_name(value: &str) -> Result<(), ValidationError> {
    if value.is_empty()
        || value.len() > MAX_FIELD_BYTES
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_.:".contains(&byte)
        })
    {
        return Err(error("INVALID_ACTION_FIELD"));
    }
    Ok(())
}

fn inspect_value(value: &Value, depth: usize) -> Result<(), ValidationError> {
    if depth > MAX_PARAMETER_DEPTH {
        return Err(error("PARAMETERS_TOO_DEEP"));
    }
    match value {
        Value::Object(entries) => {
            for (key, value) in entries {
                if is_secret_field(key) {
                    return Err(error("SECRET_FIELD_PROHIBITED"));
                }
                inspect_value(value, depth + 1)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                inspect_value(value, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_secret_field(field: &str) -> bool {
    let normalized = field.to_ascii_lowercase().replace('-', "_");
    [
        "password",
        "passwd",
        "secret",
        "secrets",
        "token",
        "api_key",
        "access_key",
        "master_key",
        "client_secret",
        "authorization",
        "bearer",
        "cookie",
        "private_key",
        "credential",
        "credentials",
    ]
    .iter()
    .any(|prohibited| normalized == *prohibited || normalized.ends_with(&format!("_{prohibited}")))
}

fn error(code: &'static str) -> ValidationError {
    ValidationError { code }
}
