use std::path::Path;
use std::process::{Command, Stdio};

/// Represents detected conflicts with other power management services
#[derive(Debug)]
pub struct ConflictDetection {
    /// Whether TLP service was detected
    pub tlp: bool,
    /// Whether GNOME Power Profiles daemon was detected
    pub gnome_power: bool,
    /// Whether tuned service was detected
    pub tuned: bool,
    /// Other power managers that were detected
    pub other: Vec<String>,
}

impl ConflictDetection {
    /// Returns true if any conflicts were detected
    pub fn has_conflicts(&self) -> bool {
        self.tlp || self.gnome_power || self.tuned || !self.other.is_empty()
    }

    /// Get formatted conflict information
    pub fn get_conflict_message(&self) -> String {
        if !self.has_conflicts() {
            return "No conflicts detected with other power management services.".to_string();
        }

        let mut message =
            "Potential conflicts detected with other power management services:\n".to_string();

        if self.tlp {
            message.push_str("- TLP service is active. This may interfere with CPU settings.\n");
        }

        if self.gnome_power {
            message.push_str(
                "- GNOME Power Profiles daemon is active. This may override CPU/power settings.\n",
            );
        }

        if self.tuned {
            message.push_str(
                "- Tuned service is active. This may conflict with CPU frequency settings.\n",
            );
        }

        for other in &self.other {
            message.push_str(&format!(
                "- {other} is active. This may conflict with superfreq.\n"
            ));
        }

        message.push_str("\nConsider disabling conflicting services for optimal operation.");

        message
    }
}

/// Detect if systemctl is available
fn systemctl_exists() -> bool {
    Command::new("sh")
        .arg("-c")
        .arg("command -v systemctl")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Check if a specific systemd service is active.
// TODO: maybe we can use some kind of a binding here
// or figure out a better detection method?
fn is_service_active(service: &str) -> bool {
    if !systemctl_exists() {
        return false;
    }

    Command::new("systemctl")
        .arg("--quiet")
        .arg("is-active")
        .arg(service)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Check for conflicts with other power management services
pub fn detect_conflicts() -> ConflictDetection {
    let mut conflicts = ConflictDetection {
        tlp: false,
        gnome_power: false,
        tuned: false,
        other: Vec::new(),
    };

    // Check for TLP
    conflicts.tlp = is_service_active("tlp.service");

    // Check for GNOME Power Profiles daemon
    conflicts.gnome_power = is_service_active("power-profiles-daemon.service");

    // Check for tuned
    conflicts.tuned = is_service_active("tuned.service");

    // Check for other common power managers
    let other_services = ["thermald.service", "powertop.service"];
    for service in other_services {
        if is_service_active(service) {
            conflicts.other.push(service.to_string());
        }
    }

    // Also check if TLP is installed but not running as a service
    // FIXME: This will obviously not work on non-FHS distros like NixOS
    // which I kinda want to prioritize. Though, since we can't actually
    // predict store paths I also don't know how else we can perform this
    // check...
    if !conflicts.tlp
        && Path::new("/usr/share/tlp").exists()
        && Command::new("sh")
            .arg("-c")
            .arg("tlp-stat -s 2>/dev/null | grep -q 'TLP power save = enabled'")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    {
        conflicts.tlp = true;
    }

    conflicts
}
