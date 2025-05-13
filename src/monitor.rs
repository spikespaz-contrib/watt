use crate::core::{SystemInfo, CpuCoreInfo, CpuGlobalInfo, BatteryInfo, SystemLoad, SystemReport};
use crate::config::AppConfig;
use std::{fs, io, path::{Path, PathBuf}, str::FromStr, time::SystemTime};

#[derive(Debug)]
pub enum SysMonitorError {
    Io(io::Error),
    ReadError(String),
    ParseError(String),
    NotAvailable(String),
}

impl From<io::Error> for SysMonitorError {
    fn from(err: io::Error) -> SysMonitorError {
        SysMonitorError::Io(err)
    }
}

impl std::fmt::Display for SysMonitorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SysMonitorError::Io(e) => write!(f, "I/O error: {}", e),
            SysMonitorError::ReadError(s) => write!(f, "Failed to read sysfs path: {}", s),
            SysMonitorError::ParseError(s) => write!(f, "Failed to parse value: {}", s),
            SysMonitorError::NotAvailable(s) => write!(f, "Information not available: {}", s),
        }
    }
}

impl std::error::Error for SysMonitorError {}

pub type Result<T, E = SysMonitorError> = std::result::Result<T, E>;

// Read a sysfs file to a string, trimming whitespace
fn read_sysfs_file_trimmed(path: impl AsRef<Path>) -> Result<String> {
    fs::read_to_string(path.as_ref())
        .map(|s| s.trim().to_string())
        .map_err(|e| {
            SysMonitorError::ReadError(format!("Path: {:?}, Error: {}", path.as_ref().display(), e))
        })
}

// Read a sysfs file and parse it to a specific type
fn read_sysfs_value<T: FromStr>(path: impl AsRef<Path>) -> Result<T> {
    let content = read_sysfs_file_trimmed(path.as_ref())?;
    content.parse::<T>().map_err(|_| {
        SysMonitorError::ParseError(format!(
            "Could not parse '{}' from {:?}",
            content,
            path.as_ref().display()
        ))
    })
}

pub fn get_system_info() -> Result<SystemInfo> {
    let mut cpu_model = "Unknown".to_string();
    if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
        for line in cpuinfo.lines() {
            if line.starts_with("model name") {
                if let Some(val) = line.split(':').nth(1) {
                    cpu_model = val.trim().to_string();
                    break;
                }
            }
        }
    }

    let architecture = std::env::consts::ARCH.to_string();

    let mut linux_distribution = "Unknown".to_string();
    if let Ok(os_release) = fs::read_to_string("/etc/os-release") {
        for line in os_release.lines() {
            if line.starts_with("PRETTY_NAME=") {
                if let Some(val) = line.split('=').nth(1) {
                    linux_distribution = val.trim_matches('"').to_string();
                    break;
                }
            }
        }
    } else if let Ok(lsb_release) = fs::read_to_string("/etc/lsb-release") { // fallback for some systems
        for line in lsb_release.lines() {
            if line.starts_with("DISTRIB_DESCRIPTION=") {
                 if let Some(val) = line.split('=').nth(1) {
                    linux_distribution = val.trim_matches('"').to_string();
                    break;
                }
            }
        }
    }


    Ok(SystemInfo {
        cpu_model,
        architecture,
        linux_distribution,
    })
}

fn get_logical_core_count() -> Result<u32> {
    let mut count = 0;
    let path = Path::new("/sys/devices/system/cpu");
    if path.exists() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let name = entry.file_name();
            if let Some(name_str) = name.to_str() {
                if name_str.starts_with("cpu") &&
                   name_str.len() > 3 &&
                   name_str[3..].chars().all(char::is_numeric) {
                    // Check if it's a directory representing a core that can have cpufreq
                    if entry.path().join("cpufreq").exists() {
                        count += 1;
                    } else if Path::new(&format!("/sys/devices/system/cpu/{}/online", name_str)).exists() {
                        // Fallback for cores that might not have cpufreq but are online (e.g. E-cores on some setups before driver loads)
                        // This is a simplification; true cpufreq capability is key.
                        // If cpufreq dir doesn't exist, it might not be controllable by this tool.
                        // For counting purposes, we count it if it's an online CPU.
                        count +=1;
                    }
                }
            }
        }
    }
    if count == 0 {
        // Fallback to num_cpus crate if sysfs parsing fails or yields 0
        Ok(num_cpus::get() as u32)
    } else {
        Ok(count)
    }
}


pub fn get_cpu_core_info(core_id: u32) -> Result<CpuCoreInfo> {
    let cpufreq_path = PathBuf::from(format!("/sys/devices/system/cpu/cpu{}/cpufreq/", core_id));

    let current_frequency_mhz = read_sysfs_value::<u32>(cpufreq_path.join("scaling_cur_freq"))
        .map(|khz| khz / 1000)
        .ok();
    let min_frequency_mhz = read_sysfs_value::<u32>(cpufreq_path.join("scaling_min_freq"))
        .map(|khz| khz / 1000)
        .ok();
    let max_frequency_mhz = read_sysfs_value::<u32>(cpufreq_path.join("scaling_max_freq"))
        .map(|khz| khz / 1000)
        .ok();

    // Temperature: Iterate through hwmon to find core-specific temperatures
    // This is a common but not universal approach.
    let mut temperature_celsius: Option<f32> = None;
    if let Ok(hwmon_dir) = fs::read_dir("/sys/class/hwmon") {
        for hw_entry in hwmon_dir.flatten() {
            let hw_path = hw_entry.path();
            // Try to find a label that indicates it's for this core or package
            // e.g. /sys/class/hwmon/hwmonX/name might be "coretemp" or similar
            // and /sys/class/hwmon/hwmonX/tempY_label might be "Core Z" or "Physical id 0"
            // This is highly system-dependent, and not all systems will have this. For now,
            // we'll try a common pattern for "coretemp" driver because it works:tm: on my system.
            if let Ok(name) = read_sysfs_file_trimmed(hw_path.join("name")) {
                if name == "coretemp" { // Common driver for Intel core temperatures
                    for i in 1..=16 { // Check a reasonable number of temp inputs
                        let label_path = hw_path.join(format!("temp{}_label", i));
                        let input_path = hw_path.join(format!("temp{}_input", i));
                        if label_path.exists() && input_path.exists() {
                            if let Ok(label) = read_sysfs_file_trimmed(&label_path) {
                                // Example: "Core 0", "Core 1", etc. or "Physical id 0" for package
                                if label.eq_ignore_ascii_case(&format!("Core {}", core_id)) ||
                                   label.eq_ignore_ascii_case(&format!("Package id {}", core_id)) { //core_id might map to package for some sensors
                                    if let Ok(temp_mc) = read_sysfs_value::<i32>(&input_path) {
                                        temperature_celsius = Some(temp_mc as f32 / 1000.0);
                                        break; // found temp for this core
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if temperature_celsius.is_some() { break; }
        }
    }

    // FIXME: This is a placeholder so that I can actually run the code. It is a little
    //complex to calculate from raw sysfs/procfs data. It typically involves reading /proc/stat
    // and calculating deltas over time. This is out of scope for simple sysfs reads here.
    // We will be returning here to this later.
    let usage_percent: Option<f32> = None;

    Ok(CpuCoreInfo {
        core_id,
        current_frequency_mhz,
        min_frequency_mhz,
        max_frequency_mhz,
        usage_percent,
        temperature_celsius,
    })
}

pub fn get_all_cpu_core_info() -> Result<Vec<CpuCoreInfo>> {
    let num_cores = get_logical_core_count()?;
    (0..num_cores).map(get_cpu_core_info).collect()
}

pub fn get_cpu_global_info() -> Result<CpuGlobalInfo> {
    // FIXME: Assume global settings can be read from cpu0 or are consistent.
    // This might not work properly for heterogeneous systems (e.g. big.LITTLE)
    let cpufreq_base = Path::new("/sys/devices/system/cpu/cpu0/cpufreq/");

    let current_governor = if cpufreq_base.join("scaling_governor").exists() {
        read_sysfs_file_trimmed(cpufreq_base.join("scaling_governor")).ok()
    } else { None };

    let available_governors = if cpufreq_base.join("scaling_available_governors").exists() {
        read_sysfs_file_trimmed(cpufreq_base.join("scaling_available_governors"))
            .map(|s| s.split_whitespace().map(String::from).collect())
            .unwrap_or_else(|_| vec![])
    } else { vec![] };

    let turbo_status = if Path::new("/sys/devices/system/cpu/intel_pstate/no_turbo").exists() {
        // 0 means turbo enabled, 1 means disabled for intel_pstate
        read_sysfs_value::<u8>("/sys/devices/system/cpu/intel_pstate/no_turbo").map(|val| val == 0).ok()
    } else if Path::new("/sys/devices/system/cpu/cpufreq/boost").exists() {
        // 1 means turbo enabled, 0 means disabled for generic cpufreq boost
        read_sysfs_value::<u8>("/sys/devices/system/cpu/cpufreq/boost").map(|val| val == 1).ok()
    } else {
        None
    };

    let epp = read_sysfs_file_trimmed(cpufreq_base.join("energy_performance_preference")).ok();

    // EPB is often an integer 0-15. Reading as string for now.
    let epb = read_sysfs_file_trimmed(cpufreq_base.join("energy_performance_bias")).ok();

    let platform_profile = read_sysfs_file_trimmed("/sys/firmware/acpi/platform_profile").ok();
    let _platform_profile_choices = read_sysfs_file_trimmed("/sys/firmware/acpi/platform_profile_choices").ok();


    Ok(CpuGlobalInfo {
        current_governor,
        available_governors,
        turbo_status,
        epp,
        epb,
        platform_profile,
    })
}

pub fn get_battery_info(config: &AppConfig) -> Result<Vec<BatteryInfo>> {
    let mut batteries = Vec::new();
    let power_supply_path = Path::new("/sys/class/power_supply");

    if !power_supply_path.exists() {
        return Ok(batteries); // no power supply directory
    }

    let ignored_supplies = config.ignored_power_supplies.as_ref().cloned().unwrap_or_default();

    // Determine overall AC connection status
    let mut overall_ac_connected = false;
    for entry in fs::read_dir(power_supply_path)? {
        let entry = entry?;
        let ps_path = entry.path();
        let name = entry.file_name().into_string().unwrap_or_default();

        // Check for AC adapter type (common names: AC, ACAD, ADP)
        if let Ok(ps_type) = read_sysfs_file_trimmed(ps_path.join("type")) {
            if ps_type == "Mains" || ps_type == "USB_PD_DRP" || ps_type == "USB_PD" || ps_type == "USB_DCP" || ps_type == "USB_CDP" || ps_type == "USB_ACA" { // USB types can also provide power
                if let Ok(online) = read_sysfs_value::<u8>(ps_path.join("online")) {
                    if online == 1 {
                        overall_ac_connected = true;
                        break;
                    }
                }
            }
        } else if name.starts_with("AC") || name.contains("ACAD") || name.contains("ADP") { // fallback for type file missing
             if let Ok(online) = read_sysfs_value::<u8>(ps_path.join("online")) {
                if online == 1 {
                    overall_ac_connected = true;
                    break;
                }
            }
        }
    }

    for entry in fs::read_dir(power_supply_path)? {
        let entry = entry?;
        let ps_path = entry.path();
        let name = entry.file_name().into_string().unwrap_or_default();

        if ignored_supplies.contains(&name) {
            continue;
        }

        if let Ok(ps_type) = read_sysfs_file_trimmed(ps_path.join("type")) {
            if ps_type == "Battery" {
                let status_str = read_sysfs_file_trimmed(ps_path.join("status")).ok();
                let capacity_percent = read_sysfs_value::<u8>(ps_path.join("capacity")).ok();

                let power_rate_watts = if ps_path.join("power_now").exists() {
                    read_sysfs_value::<i32>(ps_path.join("power_now")) // uW
                        .map(|uw| uw as f32 / 1_000_000.0)
                        .ok()
                } else if ps_path.join("current_now").exists() && ps_path.join("voltage_now").exists() {
                    let current_ua = read_sysfs_value::<i32>(ps_path.join("current_now")).ok(); // uA
                    let voltage_uv = read_sysfs_value::<i32>(ps_path.join("voltage_now")).ok(); // uV
                    if let (Some(c), Some(v)) = (current_ua, voltage_uv) {
                        // Power (W) = (Voltage (V) * Current (A))
                        // (v / 1e6 V) * (c / 1e6 A) = (v * c / 1e12) W
                        Some((c as f64 * v as f64 / 1_000_000_000_000.0) as f32)
                    } else { None }
                } else { None };

                let charge_start_threshold = read_sysfs_value::<u8>(ps_path.join("charge_control_start_threshold")).ok();
                let charge_stop_threshold = read_sysfs_value::<u8>(ps_path.join("charge_control_end_threshold")).ok();

                batteries.push(BatteryInfo {
                    name: name.clone(),
                    ac_connected: overall_ac_connected,
                    charging_state: status_str,
                    capacity_percent,
                    power_rate_watts,
                    charge_start_threshold,
                    charge_stop_threshold,
                });
            }
        }
    }
    Ok(batteries)
}

pub fn get_system_load() -> Result<SystemLoad> {
    let loadavg_str = read_sysfs_file_trimmed("/proc/loadavg")?;
    let parts: Vec<&str> = loadavg_str.split_whitespace().collect();
    if parts.len() < 3 {
        return Err(SysMonitorError::ParseError(
            "Could not parse /proc/loadavg: expected at least 3 parts".to_string(),
        ));
    }
    let load_avg_1min = parts[0].parse().map_err(|_| SysMonitorError::ParseError(format!("Failed to parse 1min load: {}", parts[0])))?;
    let load_avg_5min = parts[1].parse().map_err(|_| SysMonitorError::ParseError(format!("Failed to parse 5min load: {}", parts[1])))?;
    let load_avg_15min = parts[2].parse().map_err(|_| SysMonitorError::ParseError(format!("Failed to parse 15min load: {}", parts[2])))?;

    Ok(SystemLoad {
        load_avg_1min,
        load_avg_5min,
        load_avg_15min,
    })
}

pub fn collect_system_report(config: &AppConfig) -> Result<SystemReport> {
    let system_info = get_system_info()?;
    let cpu_cores = get_all_cpu_core_info()?;
    let cpu_global = get_cpu_global_info()?;
    let batteries = get_battery_info(config)?;
    let system_load = get_system_load()?;

    Ok(SystemReport {
        system_info,
        cpu_cores,
        cpu_global,
        batteries,
        system_load,
        timestamp: SystemTime::now(),
    })
}

