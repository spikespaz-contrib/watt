use crate::core::TurboSetting;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

// Structs for configuration using serde::Deserialize
#[derive(Deserialize, Debug, Clone)]
pub struct ProfileConfig {
    pub governor: Option<String>,
    pub turbo: Option<TurboSetting>,
    pub epp: Option<String>, // Energy Performance Preference (EPP)
    pub epb: Option<String>, // Energy Performance Bias (EPB) - usually an integer, but string for flexibility from sysfs
    pub min_freq_mhz: Option<u32>,
    pub max_freq_mhz: Option<u32>,
    pub platform_profile: Option<String>,
    pub turbo_auto_settings: Option<TurboAutoSettings>,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        ProfileConfig {
            governor: Some("schedutil".to_string()), // common sensible default (?)
            turbo: Some(TurboSetting::Auto),
            epp: None,              // defaults depend on governor and system
            epb: None,              // defaults depend on governor and system
            min_freq_mhz: None,     // no override
            max_freq_mhz: None,     // no override
            platform_profile: None, // no override
            turbo_auto_settings: Some(TurboAutoSettings::default()),
        }
    }
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct AppConfig {
    #[serde(default)]
    pub charger: ProfileConfig,
    #[serde(default)]
    pub battery: ProfileConfig,
    pub battery_charge_thresholds: Option<(u8, u8)>, // (start_threshold, stop_threshold)
    pub ignored_power_supplies: Option<Vec<String>>,
    #[serde(default = "default_poll_interval_sec")]
    pub poll_interval_sec: u64,
    #[serde(default)]
    pub daemon: DaemonConfig,
}

fn default_poll_interval_sec() -> u64 {
    5
}

// Error type for config loading
#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Toml(toml::de::Error),
    NoValidConfigFound,
    HomeDirNotFound,
}

impl From<std::io::Error> for ConfigError {
    fn from(err: std::io::Error) -> ConfigError {
        ConfigError::Io(err)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> ConfigError {
        ConfigError::Toml(err)
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "I/O error: {}", e),
            ConfigError::Toml(e) => write!(f, "TOML parsing error: {}", e),
            ConfigError::NoValidConfigFound => write!(f, "No valid configuration file found."),
            ConfigError::HomeDirNotFound => write!(f, "Could not determine user home directory."),
        }
    }
}

impl std::error::Error for ConfigError {}

// The primary function to load application configuration.
// It tries user-specific and then system-wide TOML files.
// Falls back to default settings if no file is found or if parsing fails.
pub fn load_config() -> Result<AppConfig, ConfigError> {
    let mut config_paths: Vec<PathBuf> = Vec::new();

    // User-specific path
    if let Some(home_dir) = dirs::home_dir() {
        let user_config_path = home_dir.join(".config/auto_cpufreq_rs/config.toml");
        config_paths.push(user_config_path);
    } else {
        eprintln!(
            "Warning: Could not determine home directory. User-specific config will not be loaded."
        );
    }

    // System-wide path
    let system_config_path = PathBuf::from("/etc/auto_cpufreq_rs/config.toml");
    config_paths.push(system_config_path);

    for path in config_paths {
        if path.exists() {
            println!("Attempting to load config from: {}", path.display());
            match fs::read_to_string(&path) {
                Ok(contents) => {
                    match toml::from_str::<AppConfigToml>(&contents) {
                        Ok(toml_app_config) => {
                            // Convert AppConfigToml to AppConfig
                            let app_config = AppConfig {
                                charger: ProfileConfig::from(toml_app_config.charger),
                                battery: ProfileConfig::from(toml_app_config.battery),
                                battery_charge_thresholds: toml_app_config
                                    .battery_charge_thresholds,
                                ignored_power_supplies: toml_app_config.ignored_power_supplies,
                                poll_interval_sec: toml_app_config.poll_interval_sec,
                                daemon: DaemonConfig {
                                    poll_interval_sec: toml_app_config.daemon.poll_interval_sec,
                                    adaptive_interval: toml_app_config.daemon.adaptive_interval,
                                    min_poll_interval_sec: toml_app_config
                                        .daemon
                                        .min_poll_interval_sec,
                                    max_poll_interval_sec: toml_app_config
                                        .daemon
                                        .max_poll_interval_sec,
                                    throttle_on_battery: toml_app_config.daemon.throttle_on_battery,
                                    log_level: toml_app_config.daemon.log_level.clone(),
                                    stats_file_path: toml_app_config.daemon.stats_file_path.clone(),
                                },
                            };
                            return Ok(app_config);
                        }
                        Err(e) => {
                            eprintln!("Error parsing config file {}: {}", path.display(), e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error reading config file {}: {}", path.display(), e);
                }
            }
        }
    }

    println!("No configuration file found or all failed to parse. Using default configuration.");
    // Construct default AppConfig by converting default AppConfigToml
    let default_toml_config = AppConfigToml::default();
    Ok(AppConfig {
        charger: ProfileConfig::from(default_toml_config.charger),
        battery: ProfileConfig::from(default_toml_config.battery),
        battery_charge_thresholds: default_toml_config.battery_charge_thresholds,
        ignored_power_supplies: default_toml_config.ignored_power_supplies,
        poll_interval_sec: default_toml_config.poll_interval_sec,
        daemon: DaemonConfig::default(),
    })
}

// Intermediate structs for TOML parsing
#[derive(Deserialize, Debug, Clone)]
pub struct ProfileConfigToml {
    pub governor: Option<String>,
    pub turbo: Option<String>, // "always", "auto", "never"
    pub epp: Option<String>,
    pub epb: Option<String>,
    pub min_freq_mhz: Option<u32>,
    pub max_freq_mhz: Option<u32>,
    pub platform_profile: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct AppConfigToml {
    #[serde(default)]
    pub charger: ProfileConfigToml,
    #[serde(default)]
    pub battery: ProfileConfigToml,
    pub battery_charge_thresholds: Option<(u8, u8)>,
    pub ignored_power_supplies: Option<Vec<String>>,
    #[serde(default = "default_poll_interval_sec")]
    pub poll_interval_sec: u64,
    #[serde(default)]
    pub daemon: DaemonConfigToml,
}

impl Default for ProfileConfigToml {
    fn default() -> Self {
        ProfileConfigToml {
            governor: Some("schedutil".to_string()),
            turbo: Some("auto".to_string()),
            epp: None,
            epb: None,
            min_freq_mhz: None,
            max_freq_mhz: None,
            platform_profile: None,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct TurboAutoSettings {
    #[serde(default = "default_load_threshold_high")]
    pub load_threshold_high: f32,
    #[serde(default = "default_load_threshold_low")]
    pub load_threshold_low: f32,
    #[serde(default = "default_temp_threshold_high")]
    pub temp_threshold_high: f32,
}

// Default thresholds for Auto turbo mode
pub const DEFAULT_LOAD_THRESHOLD_HIGH: f32 = 70.0; // enable turbo if load is above this
pub const DEFAULT_LOAD_THRESHOLD_LOW: f32 = 30.0; // disable turbo if load is below this
pub const DEFAULT_TEMP_THRESHOLD_HIGH: f32 = 75.0; // disable turbo if temperature is above this

fn default_load_threshold_high() -> f32 {
    DEFAULT_LOAD_THRESHOLD_HIGH
}
fn default_load_threshold_low() -> f32 {
    DEFAULT_LOAD_THRESHOLD_LOW
}
fn default_temp_threshold_high() -> f32 {
    DEFAULT_TEMP_THRESHOLD_HIGH
}

impl Default for TurboAutoSettings {
    fn default() -> Self {
        TurboAutoSettings {
            load_threshold_high: DEFAULT_LOAD_THRESHOLD_HIGH,
            load_threshold_low: DEFAULT_LOAD_THRESHOLD_LOW,
            temp_threshold_high: DEFAULT_TEMP_THRESHOLD_HIGH,
        }
    }
}

impl From<ProfileConfigToml> for ProfileConfig {
    fn from(toml_config: ProfileConfigToml) -> Self {
        ProfileConfig {
            governor: toml_config.governor,
            turbo: toml_config
                .turbo
                .and_then(|s| match s.to_lowercase().as_str() {
                    "always" => Some(TurboSetting::Always),
                    "auto" => Some(TurboSetting::Auto),
                    "never" => Some(TurboSetting::Never),
                    _ => None,
                }),
            epp: toml_config.epp,
            epb: toml_config.epb,
            min_freq_mhz: toml_config.min_freq_mhz,
            max_freq_mhz: toml_config.max_freq_mhz,
            platform_profile: toml_config.platform_profile,
            turbo_auto_settings: Some(TurboAutoSettings::default()),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct DaemonConfig {
    #[serde(default = "default_poll_interval_sec")]
    pub poll_interval_sec: u64,
    #[serde(default = "default_adaptive_interval")]
    pub adaptive_interval: bool,
    #[serde(default = "default_min_poll_interval_sec")]
    pub min_poll_interval_sec: u64,
    #[serde(default = "default_max_poll_interval_sec")]
    pub max_poll_interval_sec: u64,
    #[serde(default = "default_throttle_on_battery")]
    pub throttle_on_battery: bool,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
    #[serde(default = "default_stats_file_path")]
    pub stats_file_path: Option<String>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum LogLevel {
    Error,
    Warning,
    Info,
    Debug,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            poll_interval_sec: default_poll_interval_sec(),
            adaptive_interval: default_adaptive_interval(),
            min_poll_interval_sec: default_min_poll_interval_sec(),
            max_poll_interval_sec: default_max_poll_interval_sec(),
            throttle_on_battery: default_throttle_on_battery(),
            log_level: default_log_level(),
            stats_file_path: default_stats_file_path(),
        }
    }
}

fn default_adaptive_interval() -> bool {
    false
}

fn default_min_poll_interval_sec() -> u64 {
    1
}

fn default_max_poll_interval_sec() -> u64 {
    30
}

fn default_throttle_on_battery() -> bool {
    true
}

fn default_log_level() -> LogLevel {
    LogLevel::Info
}

fn default_stats_file_path() -> Option<String> {
    None
}

#[derive(Deserialize, Debug, Clone)]
pub struct DaemonConfigToml {
    #[serde(default = "default_poll_interval_sec")]
    pub poll_interval_sec: u64,
    #[serde(default = "default_adaptive_interval")]
    pub adaptive_interval: bool,
    #[serde(default = "default_min_poll_interval_sec")]
    pub min_poll_interval_sec: u64,
    #[serde(default = "default_max_poll_interval_sec")]
    pub max_poll_interval_sec: u64,
    #[serde(default = "default_throttle_on_battery")]
    pub throttle_on_battery: bool,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
    #[serde(default = "default_stats_file_path")]
    pub stats_file_path: Option<String>,
}

impl Default for DaemonConfigToml {
    fn default() -> Self {
        Self {
            poll_interval_sec: default_poll_interval_sec(),
            adaptive_interval: default_adaptive_interval(),
            min_poll_interval_sec: default_min_poll_interval_sec(),
            max_poll_interval_sec: default_max_poll_interval_sec(),
            throttle_on_battery: default_throttle_on_battery(),
            log_level: default_log_level(),
            stats_file_path: default_stats_file_path(),
        }
    }
}
