use log::{debug, error, warn};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::error::Error;
use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread;
use std::time::Duration;

use crate::config::{AppConfig, load_config_from_path};

/// Watches a configuration file for changes and reloads it when modified
pub struct ConfigWatcher {
    rx: Receiver<Result<Event, notify::Error>>,
    _watcher: RecommendedWatcher, // keep watcher alive while watching
    config_path: String,
    last_event_time: std::time::Instant,
}

impl ConfigWatcher {
    /// Initialize a new config watcher for the given path
    pub fn new(config_path: &str) -> Result<Self, notify::Error> {
        let (tx, rx) = channel();

        // Create a watcher with default config
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

        // Start watching the config file
        watcher.watch(Path::new(config_path), RecursiveMode::NonRecursive)?;

        debug!("Started watching config file: {config_path}");

        Ok(Self {
            rx,
            _watcher: watcher,
            config_path: config_path.to_string(),
            last_event_time: std::time::Instant::now(),
        })
    }

    /// Check for config file changes and reload if necessary
    ///
    /// # Returns
    ///
    /// `Some(AppConfig)` if the config was reloaded, `None` otherwise
    pub fn check_for_changes(&mut self) -> Option<Result<AppConfig, Box<dyn Error>>> {
        // Process all pending events before deciding to reload
        let mut should_reload = false;

        loop {
            match self.rx.try_recv() {
                Ok(Ok(event)) => {
                    // Process various file events that might indicate configuration changes
                    match event.kind {
                        EventKind::Modify(_) => {
                            debug!("Detected modification to config file: {}", self.config_path);
                            should_reload = true;
                        }
                        EventKind::Create(_) => {
                            debug!("Detected recreation of config file: {}", self.config_path);
                            should_reload = true;
                        }
                        EventKind::Remove(_) => {
                            // Some editors delete then recreate the file when saving
                            // Just log this event and wait for the create event
                            debug!(
                                "Detected removal of config file: {} - waiting for recreation",
                                self.config_path
                            );
                        }
                        _ => {} // Ignore other event types
                    }

                    if should_reload {
                        self.last_event_time = std::time::Instant::now();
                    }
                }
                Ok(Err(e)) => {
                    // File watcher error, log but continue
                    warn!("Error watching config file: {e}");
                }
                Err(TryRecvError::Empty) => {
                    // No more events
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    // Channel disconnected, watcher is dead
                    error!("Config watcher channel disconnected");
                    return None;
                }
            }
        }

        // Debounce rapid file changes (e.g., from editors that write multiple times)
        if should_reload {
            // Wait to ensure file writing is complete
            let debounce_time = Duration::from_millis(250);
            let time_since_last_event = self.last_event_time.elapsed();

            if time_since_last_event < debounce_time {
                thread::sleep(debounce_time - time_since_last_event);
            }

            // Ensure the file exists before attempting to reload
            let config_path = Path::new(&self.config_path);
            if !config_path.exists() {
                warn!(
                    "Config file does not exist after change events: {}",
                    self.config_path
                );
                return None;
            }

            debug!("Reloading configuration from {}", self.config_path);

            // Attempt to reload the config from the specific path being watched
            match load_config_from_path(Some(&self.config_path)) {
                Ok(config) => {
                    debug!("Successfully reloaded configuration");
                    Some(Ok(config))
                }
                Err(e) => {
                    error!("Failed to reload configuration: {e}");
                    Some(Err(Box::new(e)))
                }
            }
        } else {
            None
        }
    }
}
