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
macos-smc-exporter both
# Serves /metrics on http://localhost:9100
# /health for health check
```

### CLI Mode (one-shot, outputs to stdout)
```bash
macos-smc-exporter both https://vm.3pac.net/api/v1/write
# Outputs Prometheus format to stdout
# JSON summary + push result to stderr
```

### Selective Mode
```bash
macos-smc-exporter smc       # SMC temps only
macos-smc-exporter powermetrics  # Power metrics only
```

### Privileges
`powermetrics` requires root. The exporter invokes it via `sudo -n` (non-interactive),
so any mode that includes `powermetrics` (i.e. `powermetrics` or `both`) must be run
as root or with passwordless sudo configured for the `powermetrics` binary. The
launchd daemon below runs in the system domain (root) so this is satisfied for the
daemon path; manual CLI invocations as non-root will log a sudo failure and emit
zero-filled power metrics rather than hang on a password prompt.

## Building

```bash
cargo build --release
```

## Installation on macOS

```bash
# Build
cargo build --release

# Install binary
sudo cp target/release/macos-smc-exporter /usr/local/bin/macos-smc-exporter
sudo chmod +x /usr/local/bin/macos-smc-exporter

# Create launchd service
sudo tee /Library/LaunchDaemons/com.zgilburd.macos-smc-exporter.plist > /dev/null << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.zgilburd.macos-smc-exporter</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/macos-smc-exporter</string>
        <string>both</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/macos-smc-exporter.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/macos-smc-exporter.stderr.log</string>
</dict>
</plist>
EOF

# Load service
sudo launchctl bootstrap system /Library/LaunchDaemons/com.zgilburd.macos-smc-exporter.plist

# Verify
curl -s http://localhost:9100/metrics
```

## Migrating from `smc-reader`

If a previous version was installed under the `smc-reader` name, the running
launchd daemon will still reference `/usr/local/bin/smc-reader` and
`com.zgilburd.smc-reader`. Swap to the new binary like this:

```bash
# 1. Confirm what's currently running and where it points
sudo launchctl list | grep -i smc
sudo plutil -p /Library/LaunchDaemons/com.zgilburd.smc-reader.plist

# 2. Build the new binary on the target Mac (not cross-compiled)
cargo build --release

# 3. Stop and unload the old daemon
sudo launchctl bootout system/com.zgilburd.smc-reader

# 4. Remove the old artifacts
sudo rm -f /usr/local/bin/smc-reader
sudo rm -f /Library/LaunchDaemons/com.zgilburd.smc-reader.plist
sudo rm -f /tmp/pm.plist /tmp/smc-reader.stdout.log /tmp/smc-reader.stderr.log

# 5. Install the new binary + plist using the steps in "Installation on macOS" above.

# 6. Verify the new daemon is up and serving metrics with the new HELP/TYPE lines
curl -s http://localhost:9100/metrics | grep -E '^# (HELP|TYPE) macos_smc_(total|readable)_keys'
curl -s http://localhost:9100/metrics | grep -E '^macos_(cpu|gpu)_power_mw '
```

If the existing plist is custom, the minimal-change path is: keep your plist,
update its `ProgramArguments` path and `Label` to the new names, then
`launchctl bootout`/`bootstrap` it.

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
- Parses the XML plist directly in Rust via the `plist` crate (no subprocess, no temp file)
- GPU power can spike to 40W+ under ML/AI workloads (e.g., LM Studio)
- Thermal pressure levels: Nominal, Light, Moderate, Heavy

### Dependencies
- `smc` v0.2 — macOS SMC IOKit access (macOS-only target dependency)
- `plist` v1 — Apple property-list parsing
- `tiny_http` — minimal HTTP server for Prometheus scraping
- `reqwest` — optional HTTP client for direct VM push
