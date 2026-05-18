# macOS SMC Exporter

Rust binary that reads Apple SMC (System Management Controller) temperature sensors and powermetrics GPU/CPU power data on Apple Silicon Macs, exposing them as Prometheus metrics.

## Metrics Exposed

### SMC Temperature Sensors
| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `macos_smc_temperature_c` | gauge | `key` | Temperature in Celsius from SMC sensor |
| `macos_smc_total_keys` | gauge | — | Total number of temperature keys scanned |
| `macos_smc_readable_keys` | gauge | — | Number of keys successfully read |

### Power Metrics (from `powermetrics`)
| Metric | Type | Description |
|--------|------|-------------|
| `macos_cpu_power_mw` | gauge | CPU power in milliwatts |
| `macos_gpu_power_mw` | gauge | GPU power in milliwatts |
| `macos_ane_power_mw` | gauge | ANE (Neural Engine) power in milliwatts |
| `macos_combined_power_mw` | gauge | Combined CPU+GPU+ANE power in milliwatts |
| `macos_gpu_frequency_hz` | gauge | GPU frequency in Hz |
| `macos_gpu_idle_ratio_percent` | gauge | GPU idle residency percentage |
| `macos_thermal_pressure_level` | gauge | 0=Nominal, 1=Light, 2=Moderate, 3=Heavy |

## Modes

### HTTP Server Mode (default when no VM URL given)
```bash
smc-reader both
# Serves /metrics on http://localhost:9100
# /health for health check
```

### CLI Mode (one-shot, outputs to stdout)
```bash
smc-reader both https://vm.3pac.net/api/v1/write
# Outputs Prometheus format to stdout
# JSON summary + push result to stderr
```

### Selective Mode
```bash
smc-reader smc       # SMC temps only
smc-reader powermetrics  # Power metrics only
```

## Building

```bash
cargo build --release
```

## Installation on macOS

```bash
# Build
cargo build --release

# Install binary
sudo cp target/release/smc-reader /usr/local/bin/smc-reader
sudo chmod +x /usr/local/bin/smc-reader

# Create launchd service
sudo tee /Library/LaunchDaemons/com.zgilburd.smc-reader.plist > /dev/null << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.zgilburd.smc-reader</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/smc-reader</string>
        <string>both</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/smc-reader.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/smc-reader.stderr.log</string>
</dict>
</plist>
EOF

# Load service
sudo launchctl bootstrap system /Library/LaunchDaemons/com.zgilburd.smc-reader.plist

# Verify
curl -s http://localhost:9100/metrics
```

## Grafana Alloy Integration

Add to your Alloy config:

```alloy
prometheus.scrape "macos" {
  targets = [{ __address__ = "localhost:9100" }]
  job_name = "macos/m3-ultra"
  forward_to = [prometheus.remote_write.default.receiver]
}
```

## Architecture Notes

### SMC on Apple Silicon
- Apple Silicon Macs use the RTBuddy (Ring Topology Buddy) endpoint for SMC communication, not the classic `AppleSMC` IOKit driver.
- The `smc` Rust crate (v0.2.4) provides IOKit-based SMC access and works on Apple Silicon.
- Most temperature keys use `sp1e` (fixed-point 8.8) format which is **not** supported by the `smc` crate. Only keys using `flt `, `fpe2`, `sp78`, `si8 `, or `ui8 ` types are readable.
- On the M3 Ultra Mac Studio, only 3 of 99 temperature keys are readable: `TCDX` (Display), `TCMb` (Memory), `stR0` (Thermal sensor).

### Power Metrics
- Uses `powermetrics --samplers gpu_power,cpu_power,thermal -A -n 1 -i 2000 --show-extra-power-info --format plist`
- Parses the XML plist via Python's `plistlib` (reliable parsing of nested Apple plist structure)
- GPU power can spike to 40W+ under ML/AI workloads (e.g., LM Studio)
- Thermal pressure levels: Nominal, Light, Moderate, Heavy

### Dependencies
- `smc` v0.2 — macOS SMC IOKit access
- `tiny_http` — minimal HTTP server for Prometheus scraping
- `reqwest` — optional HTTP client for direct VM push
- Python 3 — plist parsing (system Python, no extra deps)
