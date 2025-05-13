// Configuration loading functionality
use std::fs;
use std::path::PathBuf;

use crate::config::types::{AppConfig, AppConfigToml, ConfigError, DaemonConfig, ProfileConfig};

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
                                battery_charge_thresholds: toml_app_config.battery_charge_thresholds,
                                ignored_power_supplies: toml_app_config.ignored_power_supplies,
                                poll_interval_sec: toml_app_config.poll_interval_sec,
                                daemon: DaemonConfig {
                                    poll_interval_sec: toml_app_config.daemon.poll_interval_sec,
                                    adaptive_interval: toml_app_config.daemon.adaptive_interval,
                                    min_poll_interval_sec: toml_app_config.daemon.min_poll_interval_sec,
                                    max_poll_interval_sec: toml_app_config.daemon.max_poll_interval_sec,
                                    throttle_on_battery: toml_app_config.daemon.throttle_on_battery,
                                    log_level: toml_app_config.daemon.log_level,
                                    stats_file_path: toml_app_config.daemon.stats_file_path,
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