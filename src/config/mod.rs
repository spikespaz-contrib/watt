pub mod watcher;

// Re-export all configuration types and functions
pub use self::types::*;
pub use self::load::*;

// Internal organization of config submodules
mod types;
mod load;