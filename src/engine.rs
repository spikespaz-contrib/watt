use crate::config::{AppConfig, ProfileConfig};
use crate::core::{OperationalMode, SystemReport, TurboSetting};
use crate::cpu::{self};
use crate::util::error::ControlError;

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

/// Determines the appropriate CPU profile based on power status or forced mode,
/// and applies the settings using functions from the `cpu` module.
pub fn determine_and_apply_settings(
    report: &SystemReport,
    config: &AppConfig,
    force_mode: Option<OperationalMode>,
) -> Result<(), EngineError> {
    // First, check if there's a governor override set
    if let Some(override_governor) = cpu::get_governor_override() {
        println!(
            "Engine: Governor override is active: '{}'. Setting governor.",
            override_governor.trim()
        );
        cpu::set_governor(override_governor.trim(), None)?;
    }

    let selected_profile_config: &ProfileConfig;

    if let Some(mode) = force_mode {
        match mode {
            OperationalMode::Powersave => {
                println!("Engine: Forced Powersave mode selected. Applying 'battery' profile.");
                selected_profile_config = &config.battery;
            }
            OperationalMode::Performance => {
                println!("Engine: Forced Performance mode selected. Applying 'charger' profile.");
                selected_profile_config = &config.charger;
            }
        }
    } else {
        // Determine AC/Battery status
        // If no batteries, assume AC power (desktop).
        // Otherwise, check the ac_connected status from the (first) battery.
        // XXX: This relies on the setting ac_connected in BatteryInfo being set correctly.
        let on_ac_power = report.batteries.is_empty()
            || report.batteries.first().map_or(false, |b| b.ac_connected);

        if on_ac_power {
            println!("Engine: On AC power, selecting Charger profile.");
            selected_profile_config = &config.charger;
        } else {
            println!("Engine: On Battery power, selecting Battery profile.");
            selected_profile_config = &config.battery;
        }
    }

    // Apply settings from selected_profile_config
    // TODO: The println! statements are for temporary debugging/logging
    // and we'd like to replace them with proper logging in the future.

    if let Some(governor) = &selected_profile_config.governor {
        println!("Engine: Setting governor to '{governor}'");
        cpu::set_governor(governor, None)?;
    }

    if let Some(turbo_setting) = selected_profile_config.turbo {
        println!("Engine: Setting turbo to '{turbo_setting:?}'");
        match turbo_setting {
            TurboSetting::Auto => {
                println!("Engine: Managing turbo in auto mode based on system conditions");
                manage_auto_turbo(report, selected_profile_config)?;
            }
            _ => cpu::set_turbo(turbo_setting)?,
        }
    }

    if let Some(epp) = &selected_profile_config.epp {
        println!("Engine: Setting EPP to '{epp}'");
        cpu::set_epp(epp, None)?;
    }

    if let Some(epb) = &selected_profile_config.epb {
        println!("Engine: Setting EPB to '{epb}'");
        cpu::set_epb(epb, None)?;
    }

    if let Some(min_freq) = selected_profile_config.min_freq_mhz {
        println!("Engine: Setting min frequency to '{min_freq} MHz'");
        cpu::set_min_frequency(min_freq, None)?;
    }

    if let Some(max_freq) = selected_profile_config.max_freq_mhz {
        println!("Engine: Setting max frequency to '{max_freq} MHz'");
        cpu::set_max_frequency(max_freq, None)?;
    }

    if let Some(profile) = &selected_profile_config.platform_profile {
        println!("Engine: Setting platform profile to '{profile}'");
        cpu::set_platform_profile(profile)?;
    }

    println!("Engine: Profile settings applied successfully.");

    Ok(())
}

fn manage_auto_turbo(report: &SystemReport, config: &ProfileConfig) -> Result<(), EngineError> {
    // Get the auto turbo settings from the config, or use defaults
    let turbo_settings = config.turbo_auto_settings.clone().unwrap_or_default();

    // Get average CPU temperature and CPU load
    let cpu_temp = report.cpu_global.average_temperature_celsius;

    // Check if we have CPU usage data available
    let avg_cpu_usage = if report.cpu_cores.is_empty() {
        None
    } else {
        let sum: f32 = report
            .cpu_cores
            .iter()
            .filter_map(|core| core.usage_percent)
            .sum();
        let count = report
            .cpu_cores
            .iter()
            .filter(|core| core.usage_percent.is_some())
            .count();

        if count > 0 {
            Some(sum / count as f32)
        } else {
            None
        }
    };

    // Decision logic for enabling/disabling turbo
    let enable_turbo = match (cpu_temp, avg_cpu_usage) {
        // If temperature is too high, disable turbo regardless of load
        (Some(temp), _) if temp >= turbo_settings.temp_threshold_high => {
            println!(
                "Engine: Auto Turbo: Disabled due to high temperature ({:.1}°C >= {:.1}°C)",
                temp, turbo_settings.temp_threshold_high
            );
            false
        }
        // If load is high enough, enable turbo (unless temp already caused it to disable)
        (_, Some(usage)) if usage >= turbo_settings.load_threshold_high => {
            println!(
                "Engine: Auto Turbo: Enabled due to high CPU load ({:.1}% >= {:.1}%)",
                usage, turbo_settings.load_threshold_high
            );
            true
        }
        // If load is low, disable turbo
        (_, Some(usage)) if usage <= turbo_settings.load_threshold_low => {
            println!(
                "Engine: Auto Turbo: Disabled due to low CPU load ({:.1}% <= {:.1}%)",
                usage, turbo_settings.load_threshold_low
            );
            false
        }
        // In intermediate load scenarios or if we can't determine, leave turbo in current state
        // For now, we'll disable it as a safe default
        _ => {
            println!("Engine: Auto Turbo: Disabled (default for indeterminate state)");
            false
        }
    };

    // Apply the turbo setting
    let turbo_setting = if enable_turbo {
        TurboSetting::Always
    } else {
        TurboSetting::Never
    };

    match cpu::set_turbo(turbo_setting) {
        Ok(()) => {
            println!(
                "Engine: Auto Turbo: Successfully set turbo to {}",
                if enable_turbo { "enabled" } else { "disabled" }
            );
            Ok(())
        }
        Err(e) => Err(EngineError::ControlError(e)),
    }
}
