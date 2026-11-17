use crate::models::{HealthCheckKind, LogLine, LogStream, ServiceConfig, ServiceRuntime, ServiceStatus};
use chrono::Local;
use regex::Regex;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

#[derive(Debug, Clone)]
pub enum ServiceEvent {
    StatusChanged {
        service_id: String,
        status: ServiceStatus,
        pid: Option<u32>,
    },
    PortsUpdated {
        service_id: String,
        ports: Vec<u16>,
    },
    LogAppended {
        service_id: String,
        line: LogLine,
    },
}

pub struct ServiceManager {
    runtime: tokio::runtime::Runtime,
    children: Arc<Mutex<HashMap<String, u32>>>,
    stop_flags: Arc<Mutex<HashMap<String, bool>>>,
    tx: mpsc::UnboundedSender<ServiceEvent>,
}

impl ServiceManager {
    pub fn new(tx: mpsc::UnboundedSender<ServiceEvent>) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");
        Self {
            runtime,
            children: Arc::new(Mutex::new(HashMap::new())),
            stop_flags: Arc::new(Mutex::new(HashMap::new())),
            tx,
        }
    }

    pub fn rt_handle(&self) -> tokio::runtime::Handle {
        self.runtime.handle().clone()
    }

    pub fn start_service(&self, config: ServiceConfig) {
        let service_id = config.id.clone();
        if let Ok(mut flags) = self.stop_flags.lock() {
            flags.insert(service_id.clone(), false);
        }

        let tx = self.tx.clone();
        let children = self.children.clone();
        let stop_flags = self.stop_flags.clone();

        self.runtime.spawn(async move {
            let now = || Local::now().format("%H:%M:%S").to_string();
            let emit_sys = |msg: String| {
                let _ = tx.send(ServiceEvent::LogAppended {
                    service_id: service_id.clone(),
                    line: LogLine {
                        timestamp: now(),
                        stream: LogStream::System,
                        text: msg,
                    },
                });
            };

            let _ = tx.send(ServiceEvent::StatusChanged {
                service_id: service_id.clone(),
                status: ServiceStatus::Starting,
                pid: None,
            });
            emit_sys(format!(
                "Starting `{}` in {:?}",
                config.command, config.working_dir
            ));

            let mut cmd = Command::new("sh");
            cmd.arg("-c")
                .arg(&config.command)
                .current_dir(&config.working_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            #[cfg(unix)]
            unsafe {
                cmd.pre_exec(|| {
                    libc_setsid();
                    Ok(())
                });
            }

            for (k, v) in &config.env_vars {
                if !k.trim().is_empty() {
                    cmd.env(k, v);
                }
            }

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(err) => {
                    let msg = format!("Spawn failed: {err}");
                    emit_sys(msg.clone());
                    let _ = tx.send(ServiceEvent::StatusChanged {
                        service_id: service_id.clone(),
                        status: ServiceStatus::Failed(msg),
                        pid: None,
                    });
                    return;
                }
            };

            let pid = child.id().unwrap_or(0);
            if let Ok(mut map) = children.lock() {
                map.insert(service_id.clone(), pid);
            }
            let _ = tx.send(ServiceEvent::StatusChanged {
                service_id: service_id.clone(),
                status: ServiceStatus::Starting,
                pid: Some(pid),
            });

            let log_matched = Arc::new(Mutex::new(false));
            let log_regex = match &config.health_check {
                HealthCheckKind::LogPattern(pat) => Regex::new(pat).ok(),
                _ => None,
            };

            let secrets: Vec<String> = config
                .env_vars
                .iter()
                .map(|(_, v)| v.trim().to_string())
                .filter(|v| v.len() >= 4)
                .collect();

            if let Some(stdout) = child.stdout.take() {
                let tx_out = tx.clone();
                let sid = service_id.clone();
                let matched_flag = log_matched.clone();
                let rx_pat = log_regex.clone();
                let sec = secrets.clone();
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stdout).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        if let Some(ref re) = rx_pat {
                            if re.is_match(&line) {
                                if let Ok(mut m) = matched_flag.lock() {
                                    *m = true;
                                }
                            }
                        }
                        let _ = tx_out.send(ServiceEvent::LogAppended {
                            service_id: sid.clone(),
                            line: LogLine {
                                timestamp: Local::now().format("%H:%M:%S").to_string(),
                                stream: LogStream::Stdout,
                                text: redact_secrets(&line, &sec),
                            },
                        });
                    }
                });
            }

            if let Some(stderr) = child.stderr.take() {
                let tx_err = tx.clone();
                let sid = service_id.clone();
                let matched_flag = log_matched.clone();
                let rx_pat = log_regex.clone();
                let sec = secrets.clone();
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stderr).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        if let Some(ref re) = rx_pat {
                            if re.is_match(&line) {
                                if let Ok(mut m) = matched_flag.lock() {
                                    *m = true;
                                }
                            }
                        }
                        let _ = tx_err.send(ServiceEvent::LogAppended {
                            service_id: sid.clone(),
                            line: LogLine {
                                timestamp: Local::now().format("%H:%M:%S").to_string(),
                                stream: LogStream::Stderr,
                                text: redact_secrets(&line, &sec),
                            },
                        });
                    }
                });
            }

            // Health & port probe loop
            let mut is_ready = false;
            loop {
                match child.try_wait() {
                    Ok(Some(exit)) => {
                        if let Ok(mut map) = children.lock() {
                            map.remove(&service_id);
                        }
                        let stopped_by_user = stop_flags
                            .lock()
                            .ok()
                            .and_then(|f| f.get(&service_id).copied())
                            .unwrap_or(false);

                        if stopped_by_user || exit.success() {
                            emit_sys(format!("Process exited ({exit})"));
                            let _ = tx.send(ServiceEvent::StatusChanged {
                                service_id: service_id.clone(),
                                status: ServiceStatus::Stopped,
                                pid: None,
                            });
                        } else {
                            let msg = format!("Exited unexpectedly ({exit})");
                            emit_sys(msg.clone());
                            let _ = tx.send(ServiceEvent::StatusChanged {
                                service_id: service_id.clone(),
                                status: ServiceStatus::Failed(msg),
                                pid: None,
                            });
                        }
                        let _ = tx.send(ServiceEvent::PortsUpdated {
                            service_id: service_id.clone(),
                            ports: Vec::new(),
                        });
                        break;
                    }
                    Ok(None) => {}
                    Err(err) => {
                        emit_sys(format!("Wait error: {err}"));
                        break;
                    }
                }

                // Detect listening ports for PID & its descendants
                let ports = detect_process_tree_ports(pid);
                let _ = tx.send(ServiceEvent::PortsUpdated {
                    service_id: service_id.clone(),
                    ports: ports.clone(),
                });

                // Evaluate health check
                let healthy = match &config.health_check {
                    HealthCheckKind::ProcessAlive => true,
                    HealthCheckKind::TcpPort(port) => check_tcp_port(*port).await,
                    HealthCheckKind::HttpGet(url) => check_http_get(url).await,
                    HealthCheckKind::LogPattern(_) => {
                        log_matched.lock().map(|m| *m).unwrap_or(false)
                    }
                };

                if healthy && !is_ready {
                    is_ready = true;
                    emit_sys("Service is Ready".to_string());
                    let _ = tx.send(ServiceEvent::StatusChanged {
                        service_id: service_id.clone(),
                        status: ServiceStatus::Ready,
                        pid: Some(pid),
                    });
                } else if !healthy && is_ready {
                    let _ = tx.send(ServiceEvent::StatusChanged {
                        service_id: service_id.clone(),
                        status: ServiceStatus::Unhealthy("Health check failed".to_string()),
                        pid: Some(pid),
                    });
                }

                sleep(Duration::from_millis(800)).await;
            }
        });
    }

    pub fn stop_service(&self, service_id: &str) {
        if let Ok(mut flags) = self.stop_flags.lock() {
            flags.insert(service_id.to_string(), true);
        }
        let pid_opt = self.children.lock().ok().and_then(|m| m.get(service_id).copied());
        if let Some(pid) = pid_opt {
            // Kill process group (-pid) first, fallback to direct pid
            let _ = std::process::Command::new("kill")
                .args(["-TERM", &format!("-{pid}")])
                .status();
            let _ = std::process::Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status();
        }
    }
}

#[cfg(unix)]
fn libc_setsid() {
    unsafe extern "C" {
        fn setsid() -> i32;
    }
    unsafe {
        setsid();
    }
}

async fn check_tcp_port(port: u16) -> bool {
    let addr = format!("127.0.0.1:{port}");
    matches!(
        tokio::time::timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await,
        Ok(Ok(_))
    )
}

async fn check_http_get(url: &str) -> bool {
    let stripped = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);
    let (host_port, path) = match stripped.split_once('/') {
        Some((hp, p)) => (hp, format!("/{p}")),
        None => (stripped, "/".to_string()),
    };
    let Ok(Ok(mut stream)) =
        tokio::time::timeout(Duration::from_millis(700), TcpStream::connect(host_port)).await
    else {
        return false;
    };
    let req = format!("GET {path} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n\r\n");
    if stream.write_all(req.as_bytes()).await.is_err() {
        return false;
    }
    let mut buf = [0u8; 64];
    match tokio::time::timeout(Duration::from_millis(700), stream.read(&mut buf)).await {
        Ok(Ok(n)) if n > 0 => {
            let head = String::from_utf8_lossy(&buf[..n]);
            head.contains(" 200 ") || head.contains(" 204 ") || head.contains(" 30")
        }
        _ => false,
    }
}

fn detect_process_tree_ports(root_pid: u32) -> Vec<u16> {
    let mut pids = vec![root_pid];
    if let Ok(out) = std::process::Command::new("pgrep")
        .args(["-P", &root_pid.to_string()])
        .output()
    {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if let Ok(child_pid) = line.trim().parse::<u32>() {
                pids.push(child_pid);
            }
        }
    }

    let pid_list: Vec<String> = pids.iter().map(|p| p.to_string()).collect();
    let Ok(out) = std::process::Command::new("lsof")
        .args([
            "-nP",
            "-iTCP",
            "-sTCP:LISTEN",
            "-a",
            "-p",
            &pid_list.join(","),
            "-Fn",
        ])
        .output()
    else {
        return Vec::new();
    };

    let mut ports = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(rest) = line.strip_prefix('n') {
            if let Some((_, port_str)) = rest.rsplit_once(':') {
                if let Ok(port) = port_str.parse::<u16>() {
                    if !ports.contains(&port) {
                        ports.push(port);
                    }
                }
            }
        }
    }
    ports.sort();
    ports
}

pub fn append_log(runtime: &mut ServiceRuntime, line: LogLine) {
    if runtime.logs.len() >= 2000 {
        runtime.logs.pop_front();
    }
    runtime.logs.push_back(line);
}

fn redact_secrets(text: &str, secrets: &[String]) -> String {
    let mut out = text.to_string();
    for secret in secrets {
        if out.contains(secret) {
            out = out.replace(secret, "***");
        }
    }
    out
}
