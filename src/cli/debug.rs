use crate::config::AppConfig;
use crate::conflict;
use crate::cpu;
use crate::monitor;
use std::error::Error;
use std::fs;

/// Prints comprehensive debug information about the system
pub fn run_debug(config: &AppConfig) -> Result<(), Box<dyn Error>> {
    println!("=== SUPERFREQ DEBUG INFORMATION ===");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Current date and time
    let now = std::time::SystemTime::now();
    println!("Timestamp: {now:?}");

    // Get system information and conflicts
    match monitor::collect_system_report(config) {
        Ok(report) => {
            println!("\n--- SYSTEM INFORMATION ---");
            println!("CPU Model: {}", report.system_info.cpu_model);
            println!("Architecture: {}", report.system_info.architecture);
            println!(
                "Linux Distribution: {}",
                report.system_info.linux_distribution
            );

            println!("\n--- CONFIGURATION ---");
            println!("Current Configuration: {config:#?}");

            println!("\n--- CPU INFORMATION ---");
            println!("Current Governor: {:?}", report.cpu_global.current_governor);
            println!(
                "Available Governors: {}",
                report.cpu_global.available_governors.join(", ")
            );
            println!("Turbo Status: {:?}", report.cpu_global.turbo_status);
            println!(
                "Energy Performance Preference (EPP): {:?}",
                report.cpu_global.epp
            );
            println!("Energy Performance Bias (EPB): {:?}", report.cpu_global.epb);

            // Add governor override information
            if let Some(override_governor) = cpu::get_governor_override() {
                println!("Governor Override: {}", override_governor.trim());
            } else {
                println!("Governor Override: None");
            }

            println!("\n--- PLATFORM PROFILE ---");
            println!(
                "Current Platform Profile: {:?}",
                report.cpu_global.platform_profile
            );
            match cpu::get_platform_profiles() {
                Ok(profiles) => println!("Available Platform Profiles: {}", profiles.join(", ")),
                Err(_) => println!("Available Platform Profiles: Not supported on this system"),
            }

            println!("\n--- CPU CORES DETAIL ---");
            println!("Total CPU Cores: {}", report.cpu_cores.len());
            for core in &report.cpu_cores {
                println!("Core {}:", core.core_id);
                println!(
                    "  Current Frequency: {} MHz",
                    core.current_frequency_mhz
                        .map_or_else(|| "N/A".to_string(), |f| f.to_string())
                );
                println!(
                    "  Min Frequency: {} MHz",
                    core.min_frequency_mhz
                        .map_or_else(|| "N/A".to_string(), |f| f.to_string())
                );
                println!(
                    "  Max Frequency: {} MHz",
                    core.max_frequency_mhz
                        .map_or_else(|| "N/A".to_string(), |f| f.to_string())
                );
                println!(
                    "  Usage: {}%",
                    core.usage_percent
                        .map_or_else(|| "N/A".to_string(), |u| format!("{u:.1}"))
                );
                println!(
                    "  Temperature: {}°C",
                    core.temperature_celsius
                        .map_or_else(|| "N/A".to_string(), |t| format!("{t:.1}"))
                );
            }

            println!("\n--- TEMPERATURE INFORMATION ---");
            println!(
                "Average CPU Temperature: {}",
                report.cpu_global.average_temperature_celsius.map_or_else(
                    || "N/A (CPU temperature sensor not detected)".to_string(),
                    |t| format!("{t:.1}°C")
                )
            );

            println!("\n--- BATTERY INFORMATION ---");
            if report.batteries.is_empty() {
                println!("No batteries found or all are ignored.");
            } else {
                for battery in &report.batteries {
                    println!("Battery: {}", battery.name);
                    println!("  AC Connected: {}", battery.ac_connected);
                    println!(
                        "  Charging State: {}",
                        battery.charging_state.as_deref().unwrap_or("N/A")
                    );
                    println!(
                        "  Capacity: {}%",
                        battery
                            .capacity_percent
                            .map_or_else(|| "N/A".to_string(), |c| c.to_string())
                    );
                    println!(
                        "  Power Rate: {} W",
                        battery
                            .power_rate_watts
                            .map_or_else(|| "N/A".to_string(), |p| format!("{p:.2}"))
                    );
                    println!(
                        "  Charge Start Threshold: {}",
                        battery
                            .charge_start_threshold
                            .map_or_else(|| "N/A".to_string(), |t| t.to_string())
                    );
                    println!(
                        "  Charge Stop Threshold: {}",
                        battery
                            .charge_stop_threshold
                            .map_or_else(|| "N/A".to_string(), |t| t.to_string())
                    );
                }
            }

            println!("\n--- SYSTEM LOAD ---");
            println!(
                "Load Average (1 min): {:.2}",
                report.system_load.load_avg_1min
            );
            println!(
                "Load Average (5 min): {:.2}",
                report.system_load.load_avg_5min
            );
            println!(
                "Load Average (15 min): {:.2}",
                report.system_load.load_avg_15min
            );

            println!("\n--- CONFLICT DETECTION ---");
            let conflicts = conflict::detect_conflicts();
            println!("{}", conflicts.get_conflict_message());

            println!("\n--- DAEMON STATUS ---");
            // Simple check for daemon status - can be expanded later
            let daemon_status = fs::metadata("/var/run/superfreq.pid").is_ok();
            println!("Daemon Running: {daemon_status}");

            Ok(())
        }
        Err(e) => Err(Box::new(e) as Box<dyn Error>),
    }
}
