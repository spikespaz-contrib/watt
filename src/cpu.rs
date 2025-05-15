use crate::core::{GovernorOverrideMode, TurboSetting};
use crate::util::error::ControlError;
use core::str;
use log::debug;
use std::{
    fs, io,
    path::{Path, PathBuf},
    string::ToString,
};

pub type Result<T, E = ControlError> = std::result::Result<T, E>;

// Write a value to a sysfs file
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

pub fn get_logical_core_count() -> Result<u32> {
    // Using num_cpus::get() for a reliable count of logical cores accessible.
    // The monitor module's get_logical_core_count might be more specific to cpufreq-capable cores,
    // but for applying settings, we might want to iterate over all reported by OS.
    // However, settings usually apply to cores with cpufreq.
    // Let's use a similar discovery to monitor's get_logical_core_count
    let mut num_cores: u32 = 0;
    let path = Path::new("/sys/devices/system/cpu");
    if !path.exists() {
        return Err(ControlError::NotSupported(format!(
            "No logical cores found at {}.",
            path.display()
        )));
    }

    let entries = fs::read_dir(path)
        .map_err(|_| {
            ControlError::PermissionDenied(format!("Cannot read contents of {}.", path.display()))
        })?
        .flatten();

    for entry in entries {
        let entry_file_name = entry.file_name();
        let Some(name) = entry_file_name.to_str() else {
            continue;
        };

        // Skip non-CPU directories (e.g., cpuidle, cpufreq)
        if !name.starts_with("cpu") || name.len() <= 3 || !name[3..].chars().all(char::is_numeric) {
            continue;
        }

        if !entry.path().join("cpufreq").exists() {
            continue;
        }

        if name[3..].parse::<u32>().is_ok() {
            num_cores += 1;
        }
    }
    if num_cores == 0 {
        // Fallback if sysfs iteration above fails to find any cpufreq cores
        num_cores = num_cpus::get() as u32;
    }

    Ok(num_cores)
}

fn for_each_cpu_core<F>(mut action: F) -> Result<()>
where
    F: FnMut(u32) -> Result<()>,
{
    let num_cores: u32 = get_logical_core_count()?;

    for core_id in 0u32..num_cores {
        action(core_id)?;
    }
    Ok(())
}

pub fn set_governor(governor: &str, core_id: Option<u32>) -> Result<()> {
    let action = |id: u32| {
        let path = format!("/sys/devices/system/cpu/cpu{id}/cpufreq/scaling_governor");
        if Path::new(&path).exists() {
            write_sysfs_value(&path, governor)
        } else {
            // Silently ignore if the path doesn't exist for a specific core,
            // as not all cores might have cpufreq (e.g. offline cores)
            Ok(())
        }
    };

    core_id.map_or_else(|| for_each_cpu_core(action), action)
}

pub fn set_turbo(setting: TurboSetting) -> Result<()> {
    let value_pstate = match setting {
        TurboSetting::Always => "0", // no_turbo = 0 means turbo is enabled
        TurboSetting::Never => "1",  // no_turbo = 1 means turbo is disabled
        TurboSetting::Auto => return Err(ControlError::InvalidValueError("Turbo Auto cannot be directly set via intel_pstate/no_turbo or cpufreq/boost. System default.".to_string())),
    };
    let value_boost = match setting {
        TurboSetting::Always => "1", // boost = 1 means turbo is enabled
        TurboSetting::Never => "0",  // boost = 0 means turbo is disabled
        TurboSetting::Auto => return Err(ControlError::InvalidValueError("Turbo Auto cannot be directly set via intel_pstate/no_turbo or cpufreq/boost. System default.".to_string())),
    };

    // AMD specific paths
    let amd_pstate_path = "/sys/devices/system/cpu/amd_pstate/cpufreq/boost";
    let msr_boost_path = "/sys/devices/system/cpu/cpufreq/amd_pstate_enable_boost";

    // Path priority (from most to least specific)
    let pstate_path = "/sys/devices/system/cpu/intel_pstate/no_turbo";
    let boost_path = "/sys/devices/system/cpu/cpufreq/boost";

    // Try each boost control path in order of specificity
    if Path::new(pstate_path).exists() {
        write_sysfs_value(pstate_path, value_pstate)
    } else if Path::new(amd_pstate_path).exists() {
        write_sysfs_value(amd_pstate_path, value_boost)
    } else if Path::new(msr_boost_path).exists() {
        write_sysfs_value(msr_boost_path, value_boost)
    } else if Path::new(boost_path).exists() {
        write_sysfs_value(boost_path, value_boost)
    } else {
        // Also try per-core cpufreq boost for some AMD systems
        let result = try_set_per_core_boost(value_boost)?;
        if result {
            Ok(())
        } else {
            Err(ControlError::NotSupported(
                "No supported CPU boost control mechanism found.".to_string(),
            ))
        }
    }
}

/// Try to set boost on a per-core basis for systems that support it
fn try_set_per_core_boost(value: &str) -> Result<bool> {
    let mut success = false;
    let num_cores = get_logical_core_count()?;

    for core_id in 0..num_cores {
        let boost_path = format!("/sys/devices/system/cpu/cpu{core_id}/cpufreq/boost");

        if Path::new(&boost_path).exists() {
            write_sysfs_value(&boost_path, value)?;
            success = true;
        }
    }

    Ok(success)
}

pub fn set_epp(epp: &str, core_id: Option<u32>) -> Result<()> {
    let action = |id: u32| {
        let path = format!("/sys/devices/system/cpu/cpu{id}/cpufreq/energy_performance_preference");
        if Path::new(&path).exists() {
            write_sysfs_value(&path, epp)
        } else {
            Ok(())
        }
    };
    core_id.map_or_else(|| for_each_cpu_core(action), action)
}

pub fn set_epb(epb: &str, core_id: Option<u32>) -> Result<()> {
    // EPB is often an integer 0-15. Ensure `epb` string is valid if parsing.
    // For now, writing it directly as a string.
    let action = |id: u32| {
        let path = format!("/sys/devices/system/cpu/cpu{id}/cpufreq/energy_performance_bias");
        if Path::new(&path).exists() {
            write_sysfs_value(&path, epb)
        } else {
            Ok(())
        }
    };
    core_id.map_or_else(|| for_each_cpu_core(action), action)
}

pub fn set_min_frequency(freq_mhz: u32, core_id: Option<u32>) -> Result<()> {
    let freq_khz_str = (freq_mhz * 1000).to_string();
    let action = |id: u32| {
        let path = format!("/sys/devices/system/cpu/cpu{id}/cpufreq/scaling_min_freq");
        if Path::new(&path).exists() {
            write_sysfs_value(&path, &freq_khz_str)
        } else {
            Ok(())
        }
    };
    core_id.map_or_else(|| for_each_cpu_core(action), action)
}

pub fn set_max_frequency(freq_mhz: u32, core_id: Option<u32>) -> Result<()> {
    let freq_khz_str = (freq_mhz * 1000).to_string();
    let action = |id: u32| {
        let path = format!("/sys/devices/system/cpu/cpu{id}/cpufreq/scaling_max_freq");
        if Path::new(&path).exists() {
            write_sysfs_value(&path, &freq_khz_str)
        } else {
            Ok(())
        }
    };
    core_id.map_or_else(|| for_each_cpu_core(action), action)
}

/// Sets the platform profile.
/// This changes the system performance, temperature, fan, and other hardware replated characteristics.
///
/// Also see [`The Kernel docs`] for this.
///
/// [`The Kernel docs`]: <https://docs.kernel.org/userspace-api/sysfs-platform_profile.html>
///
/// # Examples
///
/// ```
/// set_platform_profile("balanced");
/// ```
///
pub fn set_platform_profile(profile: &str) -> Result<()> {
    let path = "/sys/firmware/acpi/platform_profile";
    if !Path::new(path).exists() {
        return Err(ControlError::NotSupported(format!(
            "Platform profile control not found at {path}.",
        )));
    }

    let available_profiles = get_platform_profiles()?;

    if !available_profiles.contains(&profile.to_string()) {
        return Err(ControlError::InvalidProfile(format!(
            "Invalid platform control profile provided.\n\
             Provided profile: {} \n\
             Available profiles:\n\
             {}",
            profile,
            available_profiles.join(", ")
        )));
    }
    write_sysfs_value(path, profile)
}

/// Returns the list of available platform profiles.
///
/// # Errors
///
/// Returns [`ControlError::NotSupported`] if:
/// - The file `/sys/firmware/acpi/platform_profile_choices` does not exist.
/// - The file `/sys/firmware/acpi/platform_profile_choices` is empty.
///
/// Returns [`ControlError::PermissionDenied`] if the file `/sys/firmware/acpi/platform_profile_choices` cannot be read.
///
pub fn get_platform_profiles() -> Result<Vec<String>> {
    let path = "/sys/firmware/acpi/platform_profile_choices";

    if !Path::new(path).exists() {
        return Err(ControlError::NotSupported(format!(
            "Platform profile choices not found at {path}."
        )));
    }

    let content = fs::read_to_string(path)
        .map_err(|_| ControlError::PermissionDenied(format!("Cannot read contents of {path}.")))?;

    Ok(content
        .split_whitespace()
        .map(ToString::to_string)
        .collect())
}

/// Path for storing the governor override state
const GOVERNOR_OVERRIDE_PATH: &str = "/etc/superfreq/governor_override";

/// Force a specific CPU governor or reset to automatic mode
pub fn force_governor(mode: GovernorOverrideMode) -> Result<()> {
    // Create directory if it doesn't exist
    let dir_path = Path::new("/etc/superfreq");
    if !dir_path.exists() {
        fs::create_dir_all(dir_path).map_err(|e| {
            if e.kind() == io::ErrorKind::PermissionDenied {
                ControlError::PermissionDenied(format!(
                    "Permission denied creating directory: {}. Try running with sudo.",
                    dir_path.display()
                ))
            } else {
                ControlError::Io(e)
            }
        })?;
    }

    match mode {
        GovernorOverrideMode::Reset => {
            // Remove the override file if it exists
            if Path::new(GOVERNOR_OVERRIDE_PATH).exists() {
                fs::remove_file(GOVERNOR_OVERRIDE_PATH).map_err(|e| {
                    if e.kind() == io::ErrorKind::PermissionDenied {
                        ControlError::PermissionDenied(format!(
                            "Permission denied removing override file: {GOVERNOR_OVERRIDE_PATH}. Try running with sudo."
                        ))
                    } else {
                        ControlError::Io(e)
                    }
                })?;
                println!(
                    "Governor override has been reset. Normal profile-based settings will be used."
                );
            } else {
                println!("No governor override was set.");
            }
            Ok(())
        }
        GovernorOverrideMode::Performance | GovernorOverrideMode::Powersave => {
            // Create the override file with the selected governor
            let governor = mode.to_string().to_lowercase();
            fs::write(GOVERNOR_OVERRIDE_PATH, &governor).map_err(|e| {
                if e.kind() == io::ErrorKind::PermissionDenied {
                    ControlError::PermissionDenied(format!(
                        "Permission denied writing to override file: {GOVERNOR_OVERRIDE_PATH}. Try running with sudo."
                    ))
                } else {
                    ControlError::Io(e)
                }
            })?;

            // Also apply the governor immediately
            set_governor(&governor, None)?;

            println!(
                "Governor override set to '{governor}'. This setting will persist across reboots."
            );
            println!("To reset, use: superfreq force-governor reset");
            Ok(())
        }
    }
}

/// Get the current governor override if set
pub fn get_governor_override() -> Option<String> {
    if Path::new(GOVERNOR_OVERRIDE_PATH).exists() {
        fs::read_to_string(GOVERNOR_OVERRIDE_PATH).ok()
    } else {
        None
    }
}

/// Set battery charge thresholds to protect battery health
///
/// This sets the start and stop charging thresholds for batteries that support this feature.
/// Different laptop vendors implement battery thresholds in different ways, so this function
/// attempts to handle multiple implementations (Lenovo, ASUS, etc.).
///
/// The thresholds determine at what percentage the battery starts charging (when below start_threshold)
/// and at what percentage it stops (when it reaches stop_threshold).
///
/// # Arguments
///
/// * `start_threshold` - The battery percentage at which charging should start (typically 0-99)
/// * `stop_threshold` - The battery percentage at which charging should stop (typically 1-100)
///
pub fn set_battery_charge_thresholds(start_threshold: u8, stop_threshold: u8) -> Result<()> {
    // Validate threshold values
    if start_threshold >= stop_threshold {
        return Err(ControlError::InvalidValueError(format!(
            "Start threshold ({}) must be less than stop threshold ({})",
            start_threshold, stop_threshold
        )));
    }

    if stop_threshold > 100 {
        return Err(ControlError::InvalidValueError(format!(
            "Stop threshold ({}) cannot exceed 100%",
            stop_threshold
        )));
    }

    // Known sysfs paths for battery threshold control by vendor
    let threshold_paths = vec![
        // Standard sysfs paths (used by Lenovo and some others)
        ThresholdPathPattern {
            description: "Standard",
            start_path: "charge_control_start_threshold",
            stop_path: "charge_control_end_threshold",
        },
        // ASUS-specific paths
        ThresholdPathPattern {
            description: "ASUS",
            start_path: "charge_control_start_percentage",
            stop_path: "charge_control_end_percentage",
        },
        // Huawei-specific paths
        ThresholdPathPattern {
            description: "Huawei",
            start_path: "charge_start_threshold",
            stop_path: "charge_stop_threshold",
        },
    ];

    let power_supply_path = Path::new("/sys/class/power_supply");
    if !power_supply_path.exists() {
        return Err(ControlError::NotSupported(
            "Power supply path not found, battery threshold control not supported".to_string(),
        ));
    }

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

    // Scan all power supplies for battery threshold support
    for entry in entries.flatten() {
        let ps_path = entry.path();
        let name = entry.file_name().into_string().unwrap_or_default();

        // Skip non-battery devices
        if !is_battery(&ps_path)? {
            continue;
        }

        // Try each threshold path pattern for this battery
        for pattern in &threshold_paths {
            let start_threshold_path = ps_path.join(&pattern.start_path);
            let stop_threshold_path = ps_path.join(&pattern.stop_path);

            if start_threshold_path.exists() && stop_threshold_path.exists() {
                // Found a battery with threshold support
                supported_batteries.push(SupportedBattery {
                    name: name.clone(),
                    pattern: pattern.clone(),
                    path: ps_path.clone(),
                });

                // Found a supported pattern, no need to check others for this battery
                break;
            }
        }
    }

    if supported_batteries.is_empty() {
        return Err(ControlError::NotSupported(
            "No batteries with charge threshold control support found".to_string(),
        ));
    }

    // Apply thresholds to all supported batteries
    let mut errors = Vec::new();
    let mut success_count = 0;

    for battery in supported_batteries {
        let start_path = battery.path.join(&battery.pattern.start_path);
        let stop_path = battery.path.join(&battery.pattern.stop_path);

        // Attempt to set both thresholds
        match (
            write_sysfs_value(&start_path, &start_threshold.to_string()),
            write_sysfs_value(&stop_path, &stop_threshold.to_string()),
        ) {
            (Ok(_), Ok(_)) => {
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
                    error_msg.push_str(&format!(": start threshold error: {}", e));
                }
                if let Err(e) = stop_result {
                    error_msg.push_str(&format!(": stop threshold error: {}", e));
                }

                errors.push(error_msg);
            }
        }
    }

    if success_count > 0 {
        // As long as we successfully set thresholds on at least one battery, consider it a success
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

/// Helper struct for battery charge threshold path patterns
#[derive(Clone)]
struct ThresholdPathPattern {
    description: &'static str,
    start_path: &'static str,
    stop_path: &'static str,
}

/// Helper struct for batteries with threshold support
struct SupportedBattery {
    name: String,
    pattern: ThresholdPathPattern,
    path: PathBuf,
}

/// Check if a power supply entry is a battery
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
