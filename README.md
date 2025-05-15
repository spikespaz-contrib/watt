<h1 id="header" align="center">
  Superfreq
</h1>

<div align="center">
  Modern, transparent and intelligent utility for CPU management on Linux.
</div>

<div align="center">
  <br/>
  <a href="#what-is-superfreq">Synopsis</a><br/>
  <a href="#features">Features</a> | <a href="#usage">Usage</a><br/>
  <a href="#Contributing">Contributing</a>
  <br/>
</div>

## What is Superfreq

Superfreq is a modern CPU frequency and power management utility for Linux
systems. It provides intelligent control of CPU governors, frequencies, and
power-saving features, helping optimize both performance and battery life.

It is greatly inspired by auto_cpufreq, but rewritten from ground up to provide
a smoother experience with a more efficient and more correct codebase. Some
features are omitted, and it is _not_ a drop-in replacement for auto_cpufreq,
but most common usecases are already implemented.

## Features

- **Real-time CPU Management**: Monitor and control CPU governors, frequencies,
  and turbo boost
- **Intelligent Power Management**: Different profiles for AC and battery
  operation
- **Fine-tuned Controls**: Adjust energy performance preferences, biases, and
  frequency limits
- **Per-core Control**: Apply settings globally or to specific CPU cores
- **Battery Management**: Monitor battery status and power consumption
- **System Load Tracking**: Track system load and make intelligent decisions
- **Daemon Mode**: Run in background with adaptive polling to minimize overhead
- **Conflict Detection**: Identifies and warns about conflicts with other power
  management tools

## Usage

### Basic Commands

```bash
# Show current system information
superfreq info

# Run as a daemon in the background
sudo superfreq daemon

# Run with verbose logging
sudo superfreq daemon --verbose

# Display comprehensive debug information
superfreq debug
```

### CPU Governor Control

```bash
# Set CPU governor for all cores
sudo superfreq set-governor performance

# Set CPU governor for a specific core
sudo superfreq set-governor powersave --core-id 0

# Force a specific governor mode persistently
sudo superfreq force-governor performance
```

### Turbo Boost Management

```bash
# Always enable turbo boost
sudo superfreq set-turbo always

# Disable turbo boost
sudo superfreq set-turbo never

# Let Superfreq manage turbo boost based on conditions
sudo superfreq set-turbo auto
```

### Power and Performance Settings

```bash
# Set Energy Performance Preference (EPP)
sudo superfreq set-epp performance

# Set Energy Performance Bias (EPB)
sudo superfreq set-epb 4

# Set ACPI platform profile
sudo superfreq set-platform-profile balanced
```

### Frequency Control

```bash
# Set minimum CPU frequency (in MHz)
sudo superfreq set-min-freq 800

# Set maximum CPU frequency (in MHz)
sudo superfreq set-max-freq 3000

# Set per-core frequency limits
sudo superfreq set-min-freq 1200 --core-id 0
sudo superfreq set-max-freq 2800 --core-id 1
```

### Battery Management

```bash
# Set battery charging thresholds to extend battery lifespan
sudo superfreq set-battery-thresholds 40 80  # Start charging at 40%, stop at 80%
```

Battery charging thresholds help extend battery longevity by preventing constant
charging to 100%. Different laptop vendors implement this feature differently,
but Superfreq attempts to support multiple vendor implementations including:

- Lenovo ThinkPad/IdeaPad (Standard implementation)
- ASUS laptops
- Huawei laptops
- Other devices using the standard Linux power_supply API

Note that battery management is sensitive, and that your mileage may vary.
Please open an issue if your vendor is not supported, but patches would help
more than issue reports, as supporting hardware _needs_ hardware.

## Configuration

Superfreq uses TOML configuration files. Default locations:

- `/etc/superfreq/config.toml`
- `/etc/superfreq.toml`

You can also specify a custom path by setting the `SUPERFREQ_CONFIG` environment
variable.

### Sample Configuration

```toml
# Settings for when connected to a power source
[charger]
# CPU governor to use
governor = "performance"
# Turbo boost setting: "always", "auto", or "never"
turbo = "auto"
# Energy Performance Preference
epp = "performance"
# Energy Performance Bias (0-15 scale or named value)
epb = "balance_performance"
# Platform profile (if supported)
platform_profile = "performance"
# Min/max frequency in MHz (optional)
min_freq_mhz = 800
max_freq_mhz = 3500
# Optional: Profile-specific battery charge thresholds (overrides global setting)
# battery_charge_thresholds = [40, 80]  # Start at 40%, stop at 80%

# Settings for when on battery power
[battery]
governor = "powersave"
turbo = "auto"
epp = "power"
epb = "balance_power"
platform_profile = "low-power"
min_freq_mhz = 800
max_freq_mhz = 2500
# Optional: Profile-specific battery charge thresholds (overrides global setting)
# battery_charge_thresholds = [60, 80]  # Start at 60%, stop at 80% (more conservative)

# Global battery charging thresholds (applied to both profiles unless overridden)
# Start charging at 40%, stop at 80% - extends battery lifespan
# NOTE: Profile-specific thresholds (in [charger] or [battery] sections) take precedence over this global setting
battery_charge_thresholds = [40, 80]

# Daemon configuration
[daemon]
# Base polling interval in seconds
poll_interval_sec = 5
# Enable adaptive polling that changes with system state
adaptive_interval = true
# Minimum polling interval for adaptive polling (seconds)
min_poll_interval_sec = 1
# Maximum polling interval for adaptive polling (seconds)
max_poll_interval_sec = 30
# Double the polling interval when on battery to save power
throttle_on_battery = true
# Logging level: Error, Warning, Info, Debug
log_level = "Info"
# Optional stats file path
stats_file_path = "/var/run/superfreq-stats"

# Optional: List of power supplies to ignore
[power_supply_ignore_list]
mouse_battery = "hid-12:34:56:78:90:ab-battery"
# Add other devices to ignore here
```

## Advanced Features

Those are the more advanced features of Superfreq that some users might be more
inclined to use than others. If you have a use-case that is not covered, please
create an issue.

### Adaptive Polling

The daemon mode uses adaptive polling to balance responsiveness with efficiency:

- Increases polling frequency during system changes
- Decreases polling frequency during stable periods
- Reduces polling when on battery to save power

### Power Supply Filtering

Configure Superfreq to ignore certain power supplies (like peripheral batteries)
that might interfere with power state detection.

## Troubleshooting

### Permission Issues

Most CPU management commands require root privileges. If you see permission
errors, try running with `sudo`.

### Feature Compatibility

Not all features are available on all hardware:

- Turbo boost control requires CPU support for Intel/AMD boost features
- EPP/EPB settings require CPU driver support
- Platform profiles require ACPI platform profile support in your hardware

### Common Problems

1. **Settings not applying**: Check for conflicts with other power management
   tools
2. **CPU frequencies fluctuating**: May be due to thermal throttling
3. **Missing CPU information**: Verify kernel module support for your CPU

While reporting issues, please attach the results from `superfreq debug`.

## Contributing

Contributions to Superfreq are always welcome! Whether it's bug reports, feature
requests, or code contributions, please feel free to contribute.

If you are looking to reimplement features from auto_cpufreq, please consider
opening an issue first and let us know what you have in mind. Certain features
(such as the system tray) are deliberately ignored, and might not be desired in
the codebase as they stand.

### Setup

You will need Cargo and Rust installed on your system. For Nix users, using
Direnv is encouraged.

Non-Nix users may get the appropriate Cargo andn Rust versions from their
package manager.

## License

Superfreq is available under [Mozilla Public License v2.0](LICENSE) for your
convenience, and at our expense. Please see the license file for more details.
