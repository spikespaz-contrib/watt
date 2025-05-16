use crate::util::error::ControlError;
use std::{fs, io, path::Path};

/// Write a value to a sysfs file with consistent error handling
///
/// # Arguments
///
/// * `path` - The file path to write to
/// * `value` - The string value to write
///
/// # Errors
///
/// Returns a `ControlError` variant based on the specific error:
/// - `ControlError::PermissionDenied` if permission is denied
/// - `ControlError::PathMissing` if the path doesn't exist
/// - `ControlError::WriteError` for other I/O errors
pub fn write_sysfs_value(path: impl AsRef<Path>, value: &str) -> Result<(), ControlError> {
    let p = path.as_ref();

    fs::write(p, value).map_err(|e| {
        let error_msg = format!("Path: {:?}, Value: '{}', Error: {}", p.display(), value, e);
        match e.kind() {
            io::ErrorKind::PermissionDenied => ControlError::PermissionDenied(error_msg),
            io::ErrorKind::NotFound => {
                ControlError::PathMissing(format!("Path '{}' does not exist", p.display()))
            }
            _ => ControlError::WriteError(error_msg),
        }
    })
}

/// Read a value from a sysfs file with consistent error handling
///
/// # Arguments
///
/// * `path` - The file path to read from
///
/// # Returns
///
/// Returns the trimmed contents of the file as a String
///
/// # Errors
///
/// Returns a `ControlError` variant based on the specific error:
/// - `ControlError::PermissionDenied` if permission is denied
/// - `ControlError::PathMissing` if the path doesn't exist
/// - `ControlError::ReadError` for other I/O errors
pub fn read_sysfs_value(path: impl AsRef<Path>) -> Result<String, ControlError> {
    let p = path.as_ref();
    fs::read_to_string(p)
        .map_err(|e| {
            let error_msg = format!("Path: {:?}, Error: {}", p.display(), e);
            match e.kind() {
                io::ErrorKind::PermissionDenied => ControlError::PermissionDenied(error_msg),
                io::ErrorKind::NotFound => {
                    ControlError::PathMissing(format!("Path '{}' does not exist", p.display()))
                }
                _ => ControlError::ReadError(error_msg),
            }
        })
        .map(|s| s.trim().to_string())
}

/// Safely check if a path exists and is writable
///
/// # Arguments
///
/// * `path` - The file path to check
///
/// # Returns
///
/// Returns true if the path exists and is writable, false otherwise
pub fn path_exists_and_writable(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }

    // Try to open the file with write access to verify write permission
    fs::OpenOptions::new().write(true).open(path).is_ok()
}
