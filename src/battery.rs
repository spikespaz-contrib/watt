use crate::{config::types::BatteryChargeThresholds, util::error::ControlError};
use log::{debug, warn};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub type Result<T, E = ControlError> = std::result::Result<T, E>;

/// Represents a pattern of path suffixes used to control battery charge thresholds
/// for different device vendors.
#[derive(Clone)]
pub struct ThresholdPathPattern {
    pub description: &'static str,
    pub start_path: &'static str,
    pub stop_path: &'static str,
}

// Threshold patterns
const THRESHOLD_PATTERNS: &[ThresholdPathPattern] = &[
    ThresholdPathPattern {
        description: "Standard",
        start_path: "charge_control_start_threshold",
        stop_path: "charge_control_end_threshold",
    },
    ThresholdPathPattern {
        description: "ASUS",
        start_path: "charge_control_start_percentage",
        stop_path: "charge_control_end_percentage",
    },
    // Combine Huawei and ThinkPad since they use identical paths
    ThresholdPathPattern {
        description: "ThinkPad/Huawei",
        start_path: "charge_start_threshold",
        stop_path: "charge_stop_threshold",
    },
    // Framework laptop support
    ThresholdPathPattern {
        description: "Framework",
        start_path: "charge_behaviour_start_threshold",
        stop_path: "charge_behaviour_end_threshold",
    },
];

/// Represents a battery that supports charge threshold control
pub struct SupportedBattery<'a> {
    pub name: String,
    pub pattern: &'a ThresholdPathPattern,
    pub path: PathBuf,
}

/// Set battery charge thresholds to protect battery health
///
/// This sets the start and stop charging thresholds for batteries that support this feature.
/// Different laptop vendors implement battery thresholds in different ways, so this function
/// attempts to handle multiple implementations (Lenovo, ASUS, etc.).
///
/// The thresholds determine at what percentage the battery starts charging (when below `start_threshold`)
/// and at what percentage it stops (when it reaches `stop_threshold`).
///
/// # Arguments
///
/// * `start_threshold` - The battery percentage at which charging should start (typically 0-99)
/// * `stop_threshold` - The battery percentage at which charging should stop (typically 1-100)
///
/// # Errors
///
/// Returns an error if:
/// - The thresholds are invalid (start >= stop or stop > 100)
/// - No power supply path is found
/// - No batteries with threshold support are found
/// - Failed to set thresholds on any battery
pub fn set_battery_charge_thresholds(start_threshold: u8, stop_threshold: u8) -> Result<()> {
    // Validate thresholds using `BatteryChargeThresholds`
    let thresholds =
        BatteryChargeThresholds::new(start_threshold, stop_threshold).map_err(|e| match e {
            crate::config::types::ConfigError::ValidationError(msg) => {
                ControlError::InvalidValueError(msg)
            }
            _ => ControlError::InvalidValueError(format!("Invalid battery threshold values: {e}")),
        })?;

    let power_supply_path = Path::new("/sys/class/power_supply");
    if !power_supply_path.exists() {
        return Err(ControlError::NotSupported(
            "Power supply path not found, battery threshold control not supported".to_string(),
        ));
    }

    // XXX: Skip checking directory writability since /sys is a virtual filesystem
    // Individual file writability will be checked by find_battery_with_threshold_support

    let supported_batteries = find_supported_batteries(power_supply_path)?;
    if supported_batteries.is_empty() {
        return Err(ControlError::NotSupported(
            "No batteries with charge threshold control support found".to_string(),
        ));
    }

    apply_thresholds_to_batteries(&supported_batteries, thresholds.start, thresholds.stop)
}

/// Finds all batteries in the system that support threshold control
fn find_supported_batteries(power_supply_path: &Path) -> Result<Vec<SupportedBattery<'static>>> {
    let entries = fs::read_dir(power_supply_path).map_err(|e| {
        if e.kind() == io::ErrorKind::PermissionDenied {
            ControlError::PermissionDenied(format!(
                "Permission denied accessing power supply directory: {}",
                power_supply_path.display()
            ))
        } else {
            ControlError::Io(e)
        }
    })?;

    let mut supported_batteries = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to read power-supply entry: {e}");
                continue;
            }
        };
        let ps_path = entry.path();
        if is_battery(&ps_path)? {
            if let Some(battery) = find_battery_with_threshold_support(&ps_path) {
                supported_batteries.push(battery);
            }
        }
    }

    if supported_batteries.is_empty() {
        warn!("No batteries with charge threshold support found");
    } else {
        debug!(
            "Found {} batteries with threshold support",
            supported_batteries.len()
        );
        for battery in &supported_batteries {
            debug!(
                "Battery '{}' supports {} threshold control",
                battery.name, battery.pattern.description
            );
        }
    }

    Ok(supported_batteries)
}

/// Write a value to a sysfs file
fn write_sysfs_value(path: impl AsRef<Path>, value: &str) -> Result<()> {
    let p = path.as_ref();
    fs::write(p, value).map_err(|e| {
        let error_msg = format!("Path: {:?}, Value: '{}', Error: {}", p.display(), value, e);
        if e.kind() == io::ErrorKind::PermissionDenied {
            ControlError::PermissionDenied(error_msg)
        } else {
            ControlError::WriteError(error_msg)
        }
    })
}

/// Safely check if a path exists and is writable
fn path_exists_and_writable(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }

    // Try to open the file with write access to verify write permission
    fs::OpenOptions::new().write(true).open(path).is_ok()
}

/// Identifies if a battery supports threshold control and which pattern it uses
fn find_battery_with_threshold_support(ps_path: &Path) -> Option<SupportedBattery<'static>> {
    for pattern in THRESHOLD_PATTERNS {
        let start_threshold_path = ps_path.join(pattern.start_path);
        let stop_threshold_path = ps_path.join(pattern.stop_path);

        // Ensure both paths exist and are writable before considering this battery supported
        if path_exists_and_writable(&start_threshold_path)
            && path_exists_and_writable(&stop_threshold_path)
        {
            return Some(SupportedBattery {
                name: ps_path.file_name()?.to_string_lossy().to_string(),
                pattern,
                path: ps_path.to_path_buf(),
            });
        }
    }
    None
}

/// Applies the threshold settings to all supported batteries
fn apply_thresholds_to_batteries(
    batteries: &[SupportedBattery<'_>],
    start_threshold: u8,
    stop_threshold: u8,
) -> Result<()> {
    let mut errors = Vec::new();
    let mut success_count = 0;

    for battery in batteries {
        let start_path = battery.path.join(battery.pattern.start_path);
        let stop_path = battery.path.join(battery.pattern.stop_path);

        match (
            write_sysfs_value(&start_path, &start_threshold.to_string()),
            write_sysfs_value(&stop_path, &stop_threshold.to_string()),
        ) {
            (Ok(()), Ok(())) => {
                debug!(
                    "Set {}-{}% charge thresholds for {} battery '{}'",
                    start_threshold, stop_threshold, battery.pattern.description, battery.name
                );
                success_count += 1;
            }
            (start_result, stop_result) => {
                let mut error_msg = format!(
                    "Failed to set thresholds for {} battery '{}'",
                    battery.pattern.description, battery.name
                );
                if let Err(e) = start_result {
                    error_msg.push_str(&format!(": start threshold error: {e}"));
                }
                if let Err(e) = stop_result {
                    error_msg.push_str(&format!(": stop threshold error: {e}"));
                }
                errors.push(error_msg);
            }
        }
    }

    if success_count > 0 {
        if !errors.is_empty() {
            debug!(
                "Partial success setting battery thresholds: {}",
                errors.join("; ")
            );
        }
        Ok(())
    } else {
        Err(ControlError::WriteError(format!(
            "Failed to set charge thresholds on any battery: {}",
            errors.join("; ")
        )))
    }
}

/// Determines if a power supply entry is a battery
fn is_battery(path: &Path) -> Result<bool> {
    let type_path = path.join("type");

    if !type_path.exists() {
        return Ok(false);
    }

    let ps_type = fs::read_to_string(&type_path)
        .map_err(|_| ControlError::ReadError(format!("Failed to read {}", type_path.display())))?
        .trim()
        .to_string();

    Ok(ps_type == "Battery")
}
