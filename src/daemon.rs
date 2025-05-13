use crate::config::AppConfig;
use crate::engine;
use crate::monitor;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Run the daemon in foreground mode
pub fn run_background(config: AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting superfreq daemon in foreground mode...");

    // Create a flag that will be set to true when a signal is received
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // Set up signal handlers
    ctrlc::set_handler(move || {
        println!("Received shutdown signal, exiting...");
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    println!(
        "Daemon initialized with poll interval: {}s",
        config.poll_interval_sec
    );

    // Main loop
    while running.load(Ordering::SeqCst) {
        let start_time = Instant::now();

        match monitor::collect_system_report(&config) {
            Ok(report) => {
                println!("Collected system report, applying settings...");
                match engine::determine_and_apply_settings(&report, &config, None) {
                    Ok(()) => {
                        println!("Successfully applied system settings");
                    }
                    Err(e) => {
                        eprintln!("Error applying system settings: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error collecting system report: {}", e);
            }
        }

        // Sleep for the remaining time in the poll interval
        let elapsed = start_time.elapsed();
        let poll_duration = Duration::from_secs(config.poll_interval_sec);
        if elapsed < poll_duration {
            let sleep_time = poll_duration - elapsed;
            println!("Sleeping for {}s until next cycle", sleep_time.as_secs());
            std::thread::sleep(sleep_time);
        }
    }

    println!("Daemon stopped");
    Ok(())
}
