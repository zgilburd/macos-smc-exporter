use serde::Serialize;
#[cfg(target_os = "macos")]
use smc::{SMC, SMCError};
use tiny_http::{Header, Response, Server, StatusCode};

#[derive(Serialize)]
struct SMCResult {
    sensors: Vec<SensorReading>,
    total_keys: usize,
    readable_keys: usize,
}

#[derive(Serialize)]
struct SensorReading {
    key: String,
    temp_c: f64,
}

#[derive(Serialize)]
struct PowerMetricsResult {
    cpu_power_mw: f64,
    gpu_power_mw: f64,
    ane_power_mw: f64,
    combined_power_mw: f64,
    gpu_freq_hz: f64,
    gpu_idle_ratio: f64,
    thermal_pressure: String,
}

#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
fn fcc_to_str(code: u32) -> String {
    let bytes = code.to_be_bytes();
    String::from_utf8_lossy(&bytes).to_string()
}

#[cfg(not(target_os = "macos"))]
fn run_smc() -> SMCResult {
    eprintln!("run_smc: stubbed on non-macOS targets");
    SMCResult {
        sensors: vec![],
        total_keys: 0,
        readable_keys: 0,
    }
}

#[cfg(target_os = "macos")]
fn run_smc() -> SMCResult {
    let smc = match SMC::shared() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open SMC: {:?}", e);
            return SMCResult {
                sensors: vec![],
                total_keys: 0,
                readable_keys: 0,
            };
        }
    };

    let all_keys = match smc.smc_keys() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("Failed to list SMC keys: {:?}", e);
            return SMCResult {
                sensors: vec![],
                total_keys: 0,
                readable_keys: 0,
            };
        }
    };

    let temp_key_names: Vec<&str> = vec![
        "TC0C", "TC1C", "TC2C", "TC3C", "TC0A", "TC1A", "TC1B", "TG1D",
        "TH0A", "TH1A", "TH1B", "TCMb", "TCDX",
        "vtC0", "vtD0", "vtS0", "vtR0", "vtG0", "vtW0", "vtX0", "vtY0", "vtZ0",
        "stC0", "stD0", "stE0", "stF0", "stG0", "stH0", "stI0", "stJ0",
        "stK0", "stL0", "stM0", "stN0", "stO0", "stP0", "stQ0", "stR0",
        "stS0", "stT0", "stU0", "stV0", "stW0", "stX0", "stY0", "stZ0",
        "ms0T", "ms1T", "ms2T", "ms3T", "ms4T", "ms5T", "ms6T", "ms7T",
        "ms8T", "ms9T",
        "eC0 ", "eC1 ", "eP0 ", "eP1 ", "eP2 ", "eP3 ", "eP4 ", "eP5 ",
        "eP6 ", "eP7 ", "eP8 ", "eP9 ", "eP10", "eP11", "eP12", "eP13",
        "GT1S", "GT2S", "GT3S",
        "NTH0", "NTH1",
        "PT0P", "PT0S", "PT1P", "PT1S",
        "TH0P", "TH1P",
        "MB0T", "MBJT",
        "SB0T", "SB1T",
        "SI0T", "SI1T",
        "SS0T", "SS1T",
        "TB0T", "TB1T",
        "TH0T", "TH1T",
        "TS0C", "TS0F", "TS1C", "TS1F",
    ];

    let mut sensors = Vec::new();
    let mut readable = 0;

    for key in &all_keys {
        let name = fcc_to_str(key.code.0);
        if !temp_key_names.contains(&name.as_str()) {
            continue;
        }

        let data_type = key.info.id.0;
        let type_str = fcc_to_str(data_type);

        if matches!(data_type, 0x666c7420 | 0x66706532 | 0x73703738) {
            match smc.read_key::<f64>(key.code) {
                Ok(temp) => {
                    if temp > -273.0 && temp < 200.0 {
                        sensors.push(SensorReading {
                            key: name.clone(),
                            temp_c: (temp * 100.0).round() / 100.0,
                        });
                        readable += 1;
                    }
                    continue;
                }
                Err(SMCError::KeyNotFound(_)) => continue,
                Err(_) => {}
            }
        }

        if data_type == 0x73693820 {
            match smc.read_key::<i8>(key.code) {
                Ok(temp) => {
                    let t = temp as f64;
                    if t > -273.0 && t < 200.0 {
                        sensors.push(SensorReading {
                            key: name.clone(),
                            temp_c: (t * 100.0).round() / 100.0,
                        });
                        readable += 1;
                    }
                    continue;
                }
                Err(SMCError::KeyNotFound(_)) => continue,
                Err(_) => {}
            }
        }

        if data_type == 0x75693820 {
            match smc.read_key::<u8>(key.code) {
                Ok(temp) => {
                    let t = temp as f64;
                    if t > -273.0 && t < 200.0 {
                        sensors.push(SensorReading {
                            key: name.clone(),
                            temp_c: (t * 100.0).round() / 100.0,
                        });
                        readable += 1;
                    }
                    continue;
                }
                Err(SMCError::KeyNotFound(_)) => continue,
                Err(_) => {}
            }
        }

        if data_type == 0x73703165 {
            eprintln!("  [sp1e - unsupported] {}", name);
            continue;
        }

        if type_str != "      " {
            eprintln!("  [type={}] {}", type_str, name);
        }
    }

    SMCResult {
        sensors,
        total_keys: temp_key_names.len(),
        readable_keys: readable,
    }
}

fn empty_power_result() -> PowerMetricsResult {
    PowerMetricsResult {
        cpu_power_mw: 0.0,
        gpu_power_mw: 0.0,
        ane_power_mw: 0.0,
        combined_power_mw: 0.0,
        gpu_freq_hz: 0.0,
        gpu_idle_ratio: 0.0,
        thermal_pressure: "Unknown".to_string(),
    }
}

fn run_powermetrics() -> PowerMetricsResult {
    let output = match std::process::Command::new("sudo")
        .args([
            "-n",
            "powermetrics",
            "--samplers",
            "gpu_power,cpu_power,thermal",
            "-A",
            "-n",
            "1",
            "-i",
            "2000",
            "--show-extra-power-info",
            "--format",
            "plist",
        ])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Failed to run powermetrics: {}", e);
            return empty_power_result();
        }
    };

    if !output.status.success() {
        eprintln!(
            "powermetrics exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        return empty_power_result();
    }

    parse_powermetrics_plist(&output.stdout)
}

fn parse_powermetrics_plist(bytes: &[u8]) -> PowerMetricsResult {
    let root = match plist::Value::from_reader(std::io::Cursor::new(bytes)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse powermetrics plist: {}", e);
            return empty_power_result();
        }
    };

    let top = match root.as_dictionary() {
        Some(d) => d,
        None => {
            eprintln!("powermetrics plist root is not a dictionary");
            return empty_power_result();
        }
    };

    fn plist_f64(v: &plist::Value) -> Option<f64> {
        v.as_real()
            .or_else(|| v.as_signed_integer().map(|i| i as f64))
            .or_else(|| v.as_unsigned_integer().map(|u| u as f64))
    }

    fn get_f64(top: &plist::Dictionary, path: &[&str]) -> f64 {
        let mut cur = match top.get(path[0]) {
            Some(v) => v,
            None => return 0.0,
        };
        for key in &path[1..] {
            let next = cur.as_dictionary().and_then(|d| d.get(*key));
            cur = match next {
                Some(v) => v,
                None => return 0.0,
            };
        }
        plist_f64(cur).unwrap_or(0.0)
    }

    fn get_str(top: &plist::Dictionary, key: &str) -> String {
        top.get(key)
            .and_then(|v| v.as_string())
            .unwrap_or("Unknown")
            .to_string()
    }

    // Unit notes (verified empirically against `powermetrics --format plist` on M3 Ultra):
    //   processor.cpu_power / gpu_power / ane_power / combined_power: already mW
    //   gpu.freq_hz: actually MHz despite the field name (e.g. ~776.3 at idle)
    //   gpu.idle_ratio: a 0–1 fraction
    // Also note: `gpu_power` lives under `processor`, not under `gpu`.
    PowerMetricsResult {
        cpu_power_mw: get_f64(top, &["processor", "cpu_power"]).round(),
        gpu_power_mw: get_f64(top, &["processor", "gpu_power"]).round(),
        ane_power_mw: get_f64(top, &["processor", "ane_power"]).round(),
        combined_power_mw: get_f64(top, &["processor", "combined_power"]).round(),
        gpu_freq_hz: (get_f64(top, &["gpu", "freq_hz"]) * 1_000_000.0).round(),
        gpu_idle_ratio: (get_f64(top, &["gpu", "idle_ratio"]) * 100.0).round(),
        thermal_pressure: get_str(top, "thermal_pressure"),
    }
}

fn push_to_vm(json: &str, url: &str) {
    let client = reqwest::blocking::Client::new();
    match client
        .post(url)
        .header("Content-Type", "application/json")
        .body(json.to_string())
        .send()
    {
        Ok(resp) if resp.status().is_success() => {
            eprintln!("Pushed to VM: {}", url)
        }
        Ok(resp) => {
            eprintln!(
                "VM response {}: {}",
                resp.status(),
                resp.text().unwrap_or_default()
            )
        }
        Err(e) => eprintln!("Failed to push to VM: {}", e),
    }
}

fn generate_metrics(smc: &SMCResult, pm: &PowerMetricsResult) -> String {
    let mut out = String::new();

    // SMC temperature sensors
    out.push_str("# HELP macos_smc_temperature_c Temperature from SMC sensors in Celsius\n");
    out.push_str("# TYPE macos_smc_temperature_c gauge\n");
    for sensor in &smc.sensors {
        out.push_str(&format!(
            "macos_smc_temperature_c{{key=\"{}\"}} {}\n",
            sensor.key, sensor.temp_c
        ));
    }
    out.push_str("# HELP macos_smc_total_keys Total number of SMC temperature keys scanned\n");
    out.push_str("# TYPE macos_smc_total_keys gauge\n");
    out.push_str(&format!("macos_smc_total_keys {}\n", smc.total_keys));

    out.push_str("# HELP macos_smc_readable_keys Number of SMC temperature keys successfully read\n");
    out.push_str("# TYPE macos_smc_readable_keys gauge\n");
    out.push_str(&format!("macos_smc_readable_keys {}\n", smc.readable_keys));

    // Power metrics
    out.push_str("# HELP macos_cpu_power_mw CPU power in milliwatts\n");
    out.push_str("# TYPE macos_cpu_power_mw gauge\n");
    out.push_str(&format!("macos_cpu_power_mw {}\n", pm.cpu_power_mw));

    out.push_str("# HELP macos_gpu_power_mw GPU power in milliwatts\n");
    out.push_str("# TYPE macos_gpu_power_mw gauge\n");
    out.push_str(&format!("macos_gpu_power_mw {}\n", pm.gpu_power_mw));

    out.push_str("# HELP macos_ane_power_mw ANE (Neural Engine) power in milliwatts\n");
    out.push_str("# TYPE macos_ane_power_mw gauge\n");
    out.push_str(&format!("macos_ane_power_mw {}\n", pm.ane_power_mw));

    out.push_str("# HELP macos_combined_power_mw Combined CPU+GPU+ANE power in milliwatts\n");
    out.push_str("# TYPE macos_combined_power_mw gauge\n");
    out.push_str(&format!("macos_combined_power_mw {}\n", pm.combined_power_mw));

    out.push_str("# HELP macos_gpu_frequency_hz GPU frequency in Hz\n");
    out.push_str("# TYPE macos_gpu_frequency_hz gauge\n");
    out.push_str(&format!("macos_gpu_frequency_hz {}\n", pm.gpu_freq_hz));

    out.push_str("# HELP macos_gpu_idle_ratio_percent GPU idle residency percentage\n");
    out.push_str("# TYPE macos_gpu_idle_ratio_percent gauge\n");
    out.push_str(&format!("macos_gpu_idle_ratio_percent {}\n", pm.gpu_idle_ratio));

    // Thermal pressure: 0=Nominal, 1=Light, 2=Moderate, 3=Heavy
    let thermal_val = match pm.thermal_pressure.as_str() {
        "Nominal" => 0.0,
        "Light" => 1.0,
        "Moderate" => 2.0,
        "Heavy" => 3.0,
        _ => -1.0,
    };
    out.push_str("# HELP macos_thermal_pressure_level Thermal pressure level (0=Nominal, 1=Light, 2=Moderate, 3=Heavy)\n");
    out.push_str("# TYPE macos_thermal_pressure_level gauge\n");
    out.push_str(&format!("macos_thermal_pressure_level {}\n", thermal_val));

    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = if args.len() > 1 { &args[1] } else { "both" };
    let vm_url = args.get(2);

    // Read mode: "both" (default), "smc", "powermetrics"
    // HTTP server mode: runs as daemon on :9100
    // CLI mode: outputs once to stdout

    let server_mode = vm_url.is_none();

    if server_mode {
        // HTTP server mode — run as a daemon
        let server = Server::http("0.0.0.0:9100").expect("Failed to start HTTP server on :9100");
        let metrics_header = Header::from_bytes(
            b"Content-Type",
            b"text/plain; version=0.0.4; charset=utf-8",
        )
        .expect("static Content-Type header is valid");
        eprintln!("macos-smc-exporter: serving /metrics on :9100 (mode={})", mode);

        for request in server.incoming_requests() {
            if request.url() == "/metrics" {
                let smc = if mode == "smc" || mode == "both" {
                    run_smc()
                } else {
                    SMCResult {
                        sensors: vec![],
                        total_keys: 0,
                        readable_keys: 0,
                    }
                };

                let pm = if mode == "powermetrics" || mode == "both" {
                    run_powermetrics()
                } else {
                    empty_power_result()
                };

                let metrics = generate_metrics(&smc, &pm);
                let response = Response::from_string(metrics)
                    .with_header(metrics_header.clone())
                    .with_status_code(StatusCode(200));
                let _ = request.respond(response);
            } else if request.url() == "/health" {
                let response = Response::from_string("ok").with_status_code(StatusCode(200));
                let _ = request.respond(response);
            } else {
                let response = Response::from_string("404 not found").with_status_code(StatusCode(404));
                let _ = request.respond(response);
            }
        }
    } else {
        // CLI mode — run once and output
        let smc = if mode == "smc" || mode == "both" {
            Some(run_smc())
        } else {
            None
        };

        let pm = if mode == "powermetrics" || mode == "both" {
            Some(run_powermetrics())
        } else {
            None
        };

        // Output Prometheus format to stdout
        if mode == "smc" || mode == "both" {
            if let Some(ref s) = smc {
                if let Some(ref p) = pm {
                    let out = generate_metrics(s, p);
                    print!("{}", out);
                } else {
                    let out = generate_metrics(s, &empty_power_result());
                    print!("{}", out);
                }
            }
        }

        if mode == "powermetrics" {
            if let Some(ref p) = pm {
                let out = generate_metrics(
                    &SMCResult {
                        sensors: vec![],
                        total_keys: 0,
                        readable_keys: 0,
                    },
                    p,
                );
                print!("{}", out);
            }
        }

        // Also output JSON for human consumption to stderr
        if let Some(url) = vm_url {
            let mut output = serde_json::Map::new();
            if let Some(s) = smc {
                output.insert(
                    "smc".to_string(),
                    serde_json::to_value(&s).expect("SMCResult serializes"),
                );
            }
            if let Some(p) = pm {
                output.insert(
                    "powermetrics".to_string(),
                    serde_json::to_value(&p).expect("PowerMetricsResult serializes"),
                );
            }
            let json = serde_json::to_string_pretty(&output)
                .expect("Map<String, Value> serializes");
            eprintln!("{}", json);
            push_to_vm(&json, url);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shape mirrors real `powermetrics --format plist` output on M3 Ultra:
    // gpu_power lives under `processor` (not `gpu`), and power fields are mW.
    const PLIST_HAPPY: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>processor</key>
    <dict>
        <key>cpu_power</key><real>993.156</real>
        <key>gpu_power</key><real>476.674</real>
        <key>ane_power</key><real>0.0</real>
        <key>combined_power</key><real>1469.83</real>
    </dict>
    <key>gpu</key>
    <dict>
        <key>freq_hz</key><real>776.3</real>
        <key>idle_ratio</key><real>0.777314</real>
    </dict>
    <key>thermal_pressure</key><string>Nominal</string>
</dict>
</plist>"#;

    const PLIST_EMPTY: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict/>
</plist>"#;

    #[test]
    fn parse_powermetrics_plist_happy_path() {
        let r = parse_powermetrics_plist(PLIST_HAPPY);
        assert_eq!(r.cpu_power_mw, 993.0);
        assert_eq!(r.gpu_power_mw, 477.0);
        assert_eq!(r.ane_power_mw, 0.0);
        assert_eq!(r.combined_power_mw, 1470.0);
        assert_eq!(r.gpu_freq_hz, 776_300_000.0);
        assert_eq!(r.gpu_idle_ratio, 78.0);
        assert_eq!(r.thermal_pressure, "Nominal");
    }

    #[test]
    fn parse_powermetrics_plist_missing_keys() {
        let r = parse_powermetrics_plist(PLIST_EMPTY);
        let z = empty_power_result();
        assert_eq!(r.cpu_power_mw, z.cpu_power_mw);
        assert_eq!(r.gpu_power_mw, z.gpu_power_mw);
        assert_eq!(r.ane_power_mw, z.ane_power_mw);
        assert_eq!(r.combined_power_mw, z.combined_power_mw);
        assert_eq!(r.gpu_freq_hz, z.gpu_freq_hz);
        assert_eq!(r.gpu_idle_ratio, z.gpu_idle_ratio);
        assert_eq!(r.thermal_pressure, z.thermal_pressure);
    }

    #[test]
    fn parse_powermetrics_plist_malformed() {
        let r = parse_powermetrics_plist(b"not a plist");
        assert_eq!(r.cpu_power_mw, 0.0);
        assert_eq!(r.thermal_pressure, "Unknown");
    }

    #[test]
    fn generate_metrics_emits_all_expected_series() {
        let smc = SMCResult {
            sensors: vec![
                SensorReading { key: "TC0C".to_string(), temp_c: 42.5 },
                SensorReading { key: "TG1D".to_string(), temp_c: 55.0 },
            ],
            total_keys: 99,
            readable_keys: 3,
        };
        let pm = PowerMetricsResult {
            cpu_power_mw: 5500.0,
            gpu_power_mw: 4000.0,
            ane_power_mw: 100.0,
            combined_power_mw: 9600.0,
            gpu_freq_hz: 1_500_000_000.0,
            gpu_idle_ratio: 75.0,
            thermal_pressure: "Nominal".to_string(),
        };
        let out = generate_metrics(&smc, &pm);
        for name in [
            "macos_smc_temperature_c",
            "macos_smc_total_keys",
            "macos_smc_readable_keys",
            "macos_cpu_power_mw",
            "macos_gpu_power_mw",
            "macos_ane_power_mw",
            "macos_combined_power_mw",
            "macos_gpu_frequency_hz",
            "macos_gpu_idle_ratio_percent",
            "macos_thermal_pressure_level",
        ] {
            assert!(out.contains(name), "missing metric {} in output", name);
            assert!(
                out.contains(&format!("# TYPE {} gauge", name)),
                "missing TYPE line for {}",
                name,
            );
            assert!(
                out.contains(&format!("# HELP {} ", name)),
                "missing HELP line for {}",
                name,
            );
        }
        assert!(out.contains("macos_smc_temperature_c{key=\"TC0C\"} 42.5"));
        assert!(out.contains("macos_smc_total_keys 99"));
        assert!(out.contains("macos_smc_readable_keys 3"));
    }

    #[test]
    fn generate_metrics_thermal_pressure_mapping() {
        let cases = [
            ("Nominal", 0.0),
            ("Light", 1.0),
            ("Moderate", 2.0),
            ("Heavy", 3.0),
            ("Bogus", -1.0),
            ("Unknown", -1.0),
        ];
        for (label, expected) in cases {
            let pm = PowerMetricsResult {
                thermal_pressure: label.to_string(),
                ..empty_power_result()
            };
            let smc = SMCResult { sensors: vec![], total_keys: 0, readable_keys: 0 };
            let out = generate_metrics(&smc, &pm);
            assert!(
                out.contains(&format!("macos_thermal_pressure_level {}", expected)),
                "thermal_pressure {:?} should map to {}",
                label, expected,
            );
        }
    }

    #[test]
    fn generate_metrics_sensor_label_naive_escaping() {
        // SMC keys are 4 ASCII chars in practice so this documents
        // current (naive) interpolation rather than fixing it.
        let smc = SMCResult {
            sensors: vec![SensorReading {
                key: "T\"X".to_string(),
                temp_c: 1.0,
            }],
            total_keys: 1,
            readable_keys: 1,
        };
        let out = generate_metrics(&smc, &empty_power_result());
        assert!(out.contains("key=\"T\"X\""));
    }

    #[test]
    fn fcc_to_str_roundtrip() {
        // 'TC1C' = 0x54_43_31_43
        assert_eq!(fcc_to_str(0x54433143), "TC1C");
    }

    #[test]
    fn json_summary_structure() {
        let smc = SMCResult {
            sensors: vec![SensorReading { key: "TC0C".to_string(), temp_c: 42.0 }],
            total_keys: 1,
            readable_keys: 1,
        };
        let pm = PowerMetricsResult {
            cpu_power_mw: 1000.0,
            gpu_power_mw: 2000.0,
            ane_power_mw: 0.0,
            combined_power_mw: 3000.0,
            gpu_freq_hz: 1_000_000_000.0,
            gpu_idle_ratio: 50.0,
            thermal_pressure: "Light".to_string(),
        };
        let mut output = serde_json::Map::new();
        output.insert(
            "smc".to_string(),
            serde_json::to_value(&smc).expect("SMCResult serializes"),
        );
        output.insert(
            "powermetrics".to_string(),
            serde_json::to_value(&pm).expect("PowerMetricsResult serializes"),
        );
        let json = serde_json::to_string_pretty(&output)
            .expect("Map<String, Value> serializes");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("round-trip");
        assert!(parsed["smc"]["sensors"].is_array());
        assert_eq!(parsed["smc"]["sensors"][0]["key"], "TC0C");
        assert!(parsed["powermetrics"]["cpu_power_mw"].is_number());
        assert_eq!(parsed["powermetrics"]["thermal_pressure"], "Light");
    }
}
