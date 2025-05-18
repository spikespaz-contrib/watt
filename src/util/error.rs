use std::io;

#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Failed to write to sysfs path: {0}")]
    WriteError(String),

    #[error("Failed to read sysfs path: {0}")]
    ReadError(String),

    #[error("Invalid value for setting: {0}")]
    InvalidValueError(String),

    #[error("Control action not supported: {0}")]
    NotSupported(String),

    #[error("Permission denied: {0}. Try running with sudo.")]
    PermissionDenied(String),

    #[error("Invalid platform control profile {0} supplied, please provide a valid one.")]
    InvalidProfile(String),

    #[error("Invalid governor: {0}")]
    InvalidGovernor(String),

    #[error("Failed to parse value: {0}")]
    ParseError(String),

    #[error("Path missing: {0}")]
    PathMissing(String),
}

#[derive(Debug, thiserror::Error)]
pub enum SysMonitorError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Failed to read sysfs path: {0}")]
    ReadError(String),

    #[error("Failed to parse value: {0}")]
    ParseError(String),

    #[error("Failed to parse /proc/stat: {0}")]
    ProcStatParseError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("CPU control error: {0}")]
    ControlError(#[from] ControlError),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

// A unified error type for the entire application
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Control(#[from] ControlError),

    #[error("{0}")]
    Monitor(#[from] SysMonitorError),

    #[error("{0}")]
    Engine(#[from] EngineError),

    #[error("{0}")]
    Config(#[from] crate::config::ConfigError),

    #[error("{0}")]
    Generic(String),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}
