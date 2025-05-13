use std::io;

#[derive(Debug)]
pub enum ControlError {
    Io(io::Error),
    WriteError(String),
    InvalidValueError(String),
    NotSupported(String),
    PermissionDenied(String),
    InvalidProfile(String),
}

impl From<io::Error> for ControlError {
    fn from(err: io::Error) -> Self {
        match err.kind() {
            io::ErrorKind::PermissionDenied => Self::PermissionDenied(err.to_string()),
            _ => Self::Io(err),
        }
    }
}

impl std::fmt::Display for ControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::WriteError(s) => write!(f, "Failed to write to sysfs path: {s}"),
            Self::InvalidValueError(s) => write!(f, "Invalid value for setting: {s}"),
            Self::NotSupported(s) => write!(f, "Control action not supported: {s}"),
            Self::PermissionDenied(s) => {
                write!(f, "Permission denied: {s}. Try running with sudo.")
            }
            Self::InvalidProfile(s) => {
                write!(
                    f,
                    "Invalid platform control profile {s} supplied, please provide a valid one."
                )
            }
        }
    }
}

impl std::error::Error for ControlError {}

#[derive(Debug)]
pub enum SysMonitorError {
    Io(io::Error),
    ReadError(String),
    ParseError(String),
    ProcStatParseError(String),
    NotAvailable(String),
}

impl From<io::Error> for SysMonitorError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl std::fmt::Display for SysMonitorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::ReadError(s) => write!(f, "Failed to read sysfs path: {s}"),
            Self::ParseError(s) => write!(f, "Failed to parse value: {s}"),
            Self::ProcStatParseError(s) => {
                write!(f, "Failed to parse /proc/stat: {s}")
            }
            Self::NotAvailable(s) => write!(f, "Information not available: {s}"),
        }
    }
}

impl std::error::Error for SysMonitorError {}

#[derive(Debug)]
pub enum EngineError {
    ControlError(ControlError),
    ConfigurationError(String),
}

impl From<ControlError> for EngineError {
    fn from(err: ControlError) -> Self {
        Self::ControlError(err)
    }
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ControlError(e) => write!(f, "CPU control error: {e}"),
            Self::ConfigurationError(s) => write!(f, "Configuration error: {s}"),
        }
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ControlError(e) => Some(e),
            Self::ConfigurationError(_) => None,
        }
    }
}
