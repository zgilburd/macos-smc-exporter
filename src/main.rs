use serde::Serialize;
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

fn fcc_to_str(code: u32) -> String {
    let bytes = code.to_be_bytes();
    String::from_utf8_lossy(&bytes).to_string()
}

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

fn run_powermetrics() -> PowerMetricsResult {
    let output = std::process::Command::new("sudo")
        .args([
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
        .expect("Failed to run powermetrics");

    if !output.status.success() {
        return PowerMetricsResult {
            cpu_power_mw: 0.0,
            gpu_power_mw: 0.0,
            ane_power_mw: 0.0,
            combined_power_mw: 0.0,
            gpu_freq_hz: 0.0,
            gpu_idle_ratio: 0.0,
            thermal_pressure: "Unknown".to_string(),
        };
    }

    let tmp = "/tmp/pm.plist";
    std::fs::write(tmp, &output.stdout).expect("Failed to write plist temp");

    let py_output = std::process::Command::new("python3")
        .args([
            "-c",
            "import sys,plistlib,json; d=plistlib.load(open(sys.argv[1],'rb')); p=d.get('processor',{}); g=d.get('gpu',{}); print(json.dumps({'cpu_power':p.get('cpu_power',0),'gpu_power':p.get('gpu_power',0),'ane_power':p.get('ane_power',0),'combined_power':p.get('combined_power',0),'freq_hz':g.get('freq_hz',0),'idle_ratio':g.get('idle_ratio',0),'thermal_pressure':d.get('thermal_pressure','Unknown')}))",
            tmp,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("Failed to run python plist parser");

    if !py_output.status.success() {
        eprintln!("Python plist parse failed: {}", String::from_utf8_lossy(&py_output.stderr));
        return PowerMetricsResult {
            cpu_power_mw: 0.0,
            gpu_power_mw: 0.0,
            ane_power_mw: 0.0,
            combined_power_mw: 0.0,
            gpu_freq_hz: 0.0,
            gpu_idle_ratio: 0.0,
            thermal_pressure: "Unknown".to_string(),
        };
    }

    let json_str = String::from_utf8_lossy(&py_output.stdout);
    let vals: serde_json::Value = serde_json::from_str(&json_str).unwrap_or_else(|_| {
        eprintln!("Failed to parse python output: {}", json_str);
        serde_json::json!({})
    });

    let to_f64 = |k: &str| vals.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let to_str = |k: &str| vals.get(k).and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();

    PowerMetricsResult {
        cpu_power_mw: (to_f64("cpu_power") * 1000.0).round(),
        gpu_power_mw: (to_f64("gpu_power") * 1000.0).round(),
        ane_power_mw: (to_f64("ane_power") * 1000.0).round(),
        combined_power_mw: (to_f64("combined_power") * 1000.0).round(),
        gpu_freq_hz: (to_f64("freq_hz") * 1_000_000_000.0).round(),
        gpu_idle_ratio: (to_f64("idle_ratio") * 100.0).round(),
        thermal_pressure: to_str("thermal_pressure"),
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
    out.push_str(&format!("macos_smc_total_keys {}\n", smc.total_keys));
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
        eprintln!("smc-reader: serving /metrics on :9100 (mode={})", mode);

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
                    PowerMetricsResult {
                        cpu_power_mw: 0.0,
                        gpu_power_mw: 0.0,
                        ane_power_mw: 0.0,
                        combined_power_mw: 0.0,
                        gpu_freq_hz: 0.0,
                        gpu_idle_ratio: 0.0,
                        thermal_pressure: "Unknown".to_string(),
                    }
                };

                let metrics = generate_metrics(&smc, &pm);
                let header = Header::from_bytes(b"Content-Type", b"text/plain; version=0.0.4; charset=utf-8").unwrap();
                let response = Response::from_string(metrics)
                    .with_header(header)
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
                    let out = generate_metrics(
                        s,
                        &PowerMetricsResult {
                            cpu_power_mw: 0.0,
                            gpu_power_mw: 0.0,
                            ane_power_mw: 0.0,
                            combined_power_mw: 0.0,
                            gpu_freq_hz: 0.0,
                            gpu_idle_ratio: 0.0,
                            thermal_pressure: "Unknown".to_string(),
                        },
                    );
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
                    serde_json::Value::Object(
                        serde_json::from_value(serde_json::to_value(s).unwrap()).unwrap(),
                    ),
                );
            }
            if let Some(p) = pm {
                output.insert(
                    "powermetrics".to_string(),
                    serde_json::Value::Object(
                        serde_json::from_value(serde_json::to_value(p).unwrap()).unwrap(),
                    ),
                );
            }
            let json = serde_json::to_string_pretty(&output).unwrap();
            eprintln!("{}", json);
            push_to_vm(&json, url);
        }
    }
}
