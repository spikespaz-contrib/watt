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
                    // Only process write/modify events
                    if matches!(event.kind, EventKind::Modify(_)) {
                        should_reload = true;
                        self.last_event_time = std::time::Instant::now();
                    }
                }
                Ok(Err(e)) => {
                    // File watcher error, log but continue
                    eprintln!("Error watching config file: {e}");
                }
                Err(TryRecvError::Empty) => {
                    // No more events
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    // Channel disconnected, watcher is dead
                    eprintln!("Config watcher channel disconnected");
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

            // Attempt to reload the config from the specific path being watched
            match load_config_from_path(Some(&self.config_path)) {
                Ok(config) => Some(Ok(config)),
                Err(e) => Some(Err(Box::new(e))),
            }
        } else {
            None
        }
    }

    /// Get the path of the config file being watched
    pub const fn config_path(&self) -> &String {
        &self.config_path
    }
}
