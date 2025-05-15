use crate::battery;
use crate::config::{AppConfig, ProfileConfig};
use crate::core::{OperationalMode, SystemReport, TurboSetting};
use crate::cpu::{self};
use crate::util::error::{ControlError, EngineError};
use log::{debug, info, warn};

/// Try applying a CPU feature and handle common error cases. Centralizes the where we
/// previously did:
/// 1. Try to apply a feature setting
/// 2. If not supported, log a warning and continue
/// 3. If other error, propagate the error
fn try_apply_feature<F, T>(
    feature_name: &str,
    value_description: &str,
    apply_fn: F,
) -> Result<(), EngineError>
where
    F: FnOnce() -> Result<T, ControlError>,
{
    info!("Setting {feature_name} to '{value_description}'");

    match apply_fn() {
        Ok(_) => Ok(()),
        Err(e) => {
            if matches!(e, ControlError::NotSupported(_))
                || matches!(e, ControlError::InvalidValueError(_))
            {
                warn!(
                    "{feature_name} setting is not supported on this system. Skipping {feature_name} configuration."
                );
                Ok(())
            } else {
                Err(EngineError::ControlError(e))
            }
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
        info!(
            "Governor override is active: '{}'. Setting governor.",
            override_governor.trim()
        );

        // Apply the override governor setting
        try_apply_feature("override governor", override_governor.trim(), || {
            cpu::set_governor(override_governor.trim(), None)
        })?;
    }

    let selected_profile_config: &ProfileConfig;

    if let Some(mode) = force_mode {
        match mode {
            OperationalMode::Powersave => {
                info!("Forced Powersave mode selected. Applying 'battery' profile.");
                selected_profile_config = &config.battery;
            }
            OperationalMode::Performance => {
                info!("Forced Performance mode selected. Applying 'charger' profile.");
                selected_profile_config = &config.charger;
            }
        }
    } else {
        // Determine AC/Battery status
        // For desktops (no batteries), we should always use the AC power profile
        // For laptops, we check if any battery is present and not connected to AC
        let on_ac_power = if report.batteries.is_empty() {
            // No batteries means desktop/server, always on AC
            true
        } else {
            // At least one battery exists, check if it's on AC
            report.batteries.first().is_some_and(|b| b.ac_connected)
        };

        if on_ac_power {
            info!("On AC power, selecting Charger profile.");
            selected_profile_config = &config.charger;
        } else {
            info!("On Battery power, selecting Battery profile.");
            selected_profile_config = &config.battery;
        }
    }

    // Apply settings from selected_profile_config
    if let Some(governor) = &selected_profile_config.governor {
        info!("Setting governor to '{governor}'");
        // Let set_governor handle the validation
        if let Err(e) = cpu::set_governor(governor, None) {
            // If the governor is not available, log a warning
            if matches!(e, ControlError::InvalidValueError(_))
                || matches!(e, ControlError::NotSupported(_))
            {
                warn!(
                    "Configured governor '{governor}' is not available on this system. Skipping."
                );
            } else {
                return Err(e.into());
            }
        }
    }

    if let Some(turbo_setting) = selected_profile_config.turbo {
        info!("Setting turbo to '{turbo_setting:?}'");
        match turbo_setting {
            TurboSetting::Auto => {
                debug!("Managing turbo in auto mode based on system conditions");
                manage_auto_turbo(report, selected_profile_config)?;
            }
            _ => {
                try_apply_feature("Turbo boost", &format!("{turbo_setting:?}"), || {
                    cpu::set_turbo(turbo_setting)
                })?;
            }
        }
    }

    if let Some(epp) = &selected_profile_config.epp {
        try_apply_feature("EPP", epp, || cpu::set_epp(epp, None))?;
    }

    if let Some(epb) = &selected_profile_config.epb {
        try_apply_feature("EPB", epb, || cpu::set_epb(epb, None))?;
    }

    if let Some(min_freq) = selected_profile_config.min_freq_mhz {
        try_apply_feature("min frequency", &format!("{min_freq} MHz"), || {
            cpu::set_min_frequency(min_freq, None)
        })?;
    }

    if let Some(max_freq) = selected_profile_config.max_freq_mhz {
        try_apply_feature("max frequency", &format!("{max_freq} MHz"), || {
            cpu::set_max_frequency(max_freq, None)
        })?;
    }

    if let Some(profile) = &selected_profile_config.platform_profile {
        try_apply_feature("platform profile", profile, || {
            cpu::set_platform_profile(profile)
        })?;
    }

    // Set battery charge thresholds if configured
    if let Some(thresholds) = &selected_profile_config.battery_charge_thresholds {
        let start_threshold = thresholds.start;
        let stop_threshold = thresholds.stop;

        if start_threshold < stop_threshold && stop_threshold <= 100 {
            info!("Setting battery charge thresholds: {start_threshold}-{stop_threshold}%");
            match battery::set_battery_charge_thresholds(start_threshold, stop_threshold) {
                Ok(()) => debug!("Battery charge thresholds set successfully"),
                Err(e) => warn!("Failed to set battery charge thresholds: {e}"),
            }
        } else {
            warn!(
                "Invalid battery threshold values: start={start_threshold}, stop={stop_threshold}"
            );
        }
    }

    debug!("Profile settings applied successfully.");

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

    // Validate the configuration to ensure it's usable
    if turbo_settings.load_threshold_high <= turbo_settings.load_threshold_low {
        return Err(EngineError::ConfigurationError(
            "Invalid turbo auto settings: high threshold must be greater than low threshold"
                .to_string(),
        ));
    }

    // Decision logic for enabling/disabling turbo
    let enable_turbo = match (cpu_temp, avg_cpu_usage) {
        // If temperature is too high, disable turbo regardless of load
        (Some(temp), _) if temp >= turbo_settings.temp_threshold_high => {
            info!(
                "Auto Turbo: Disabled due to high temperature ({:.1}°C >= {:.1}°C)",
                temp, turbo_settings.temp_threshold_high
            );
            false
        }
        // If load is high enough, enable turbo (unless temp already caused it to disable)
        (_, Some(usage)) if usage >= turbo_settings.load_threshold_high => {
            info!(
                "Auto Turbo: Enabled due to high CPU load ({:.1}% >= {:.1}%)",
                usage, turbo_settings.load_threshold_high
            );
            true
        }
        // If load is low, disable turbo
        (_, Some(usage)) if usage <= turbo_settings.load_threshold_low => {
            info!(
                "Auto Turbo: Disabled due to low CPU load ({:.1}% <= {:.1}%)",
                usage, turbo_settings.load_threshold_low
            );
            false
        }
        // In intermediate load scenarios or if we can't determine, leave turbo in current state
        // For now, we'll disable it as a safe default
        _ => {
            info!("Auto Turbo: Disabled (default for indeterminate state)");
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
            debug!(
                "Auto Turbo: Successfully set turbo to {}",
                if enable_turbo { "enabled" } else { "disabled" }
            );
            Ok(())
        }
        Err(e) => Err(EngineError::ControlError(e)),
    }
}
