use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;
use std::thread;
use std::error::Error;

use crate::config::{load_config, AppConfig};

/// Watches a configuration file for changes and reloads it when modified
pub struct ConfigWatcher {
    rx: Receiver<Result<Event, notify::Error>>,
    _watcher: RecommendedWatcher, // keep watcher alive while watching
    config_path: String,
}

impl ConfigWatcher {
    /// Initialize a new config watcher for the given path
    pub fn new(config_path: &str) -> Result<Self, notify::Error> {
        let (tx, rx) = channel();

        // Create a watcher with default config
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

        // Start watching the config file
        watcher.watch(Path::new(config_path), RecursiveMode::NonRecursive)?;

        Ok(Self {
            rx,
            _watcher: watcher,
            config_path: config_path.to_string(),
        })
    }

    /// Check for config file changes and reload if necessary
    ///
    /// # Returns
    ///
    /// `Some(AppConfig)` if the config was reloaded, `None`` otherwise
    pub fn check_for_changes(&self) -> Option<Result<AppConfig, Box<dyn Error>>> {
        // Non-blocking check for file events
        match self.rx.try_recv() {
            Ok(Ok(event)) => {
                // Only process write/modify events
                if matches!(event.kind, EventKind::Modify(_)) {
                    // Add a small delay to ensure the file write is complete
                    thread::sleep(Duration::from_millis(100));

                    // Attempt to reload the config
                    match load_config() {
                        Ok(config) => {
                            println!("Configuration file changed. Reloaded configuration.");
                            Some(Ok(config))
                        }
                        Err(e) => {
                            eprintln!("Error reloading configuration: {e}");
                            Some(Err(Box::new(e)))
                        }
                    }
                } else {
                    None
                }
            }
            // No events or channel errors
            _ => None,
        }
    }

    /// Get the path of the config file being watched
    pub const fn config_path(&self) -> &String {
        &self.config_path
    }
}