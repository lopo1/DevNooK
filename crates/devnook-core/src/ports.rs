use crate::models::{AddressFamily, PortEntry, Project, ServiceConfig, ServiceRuntime};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

pub fn inspect_listening_ports(
    projects: &[Project],
    services: &[ServiceConfig],
    runtimes: &HashMap<String, ServiceRuntime>,
) -> Vec<PortEntry> {
    let output = Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "pcn"])
        .output();

    let Ok(out) = output else {
        return Vec::new();
    };
    let stdout = String::from_utf8_lossy(&out.stdout);

    // Group multiple bind addresses for the same (pid, port)
    let mut grouped: HashMap<(u32, u16), (String, Vec<String>)> = HashMap::new();
    let mut cur_pid: Option<u32> = None;
    let mut cur_cmd: String = String::new();

    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix('p') {
            cur_pid = rest.parse::<u32>().ok();
            cur_cmd.clear();
        } else if let Some(rest) = line.strip_prefix('c') {
            cur_cmd = rest.to_string();
        } else if let Some(rest) = line.strip_prefix('n') {
            if let Some(pid) = cur_pid {
                if let Some((addr, port_str)) = rest.rsplit_once(':') {
                    if let Ok(port) = port_str.parse::<u16>() {
                        let entry = grouped
                            .entry((pid, port))
                            .or_insert_with(|| (cur_cmd.clone(), Vec::new()));
                        let addr_str = addr.to_string();
                        if !entry.1.contains(&addr_str) {
                            entry.1.push(addr_str);
                        }
                    }
                }
            }
        }
    }

    let mut pid_meta: HashMap<u32, (String, Option<PathBuf>)> = HashMap::new();
    for (pid, _) in grouped.keys() {
        if pid_meta.contains_key(pid) {
            continue;
        }
        let full_cmd = get_pid_command(*pid);
        let cwd = get_pid_cwd(*pid);
        pid_meta.insert(*pid, (full_cmd, cwd));
    }

    let mut result: Vec<PortEntry> = grouped
        .into_iter()
        .map(|((pid, port), (proc_name, bind_addrs))| {
            let (command, cwd) = pid_meta
                .get(&pid)
                .cloned()
                .unwrap_or_else(|| (proc_name.clone(), None));

            let matched_service = services
                .iter()
                .find(|svc| {
                    runtimes
                        .get(&svc.id)
                        .and_then(|rt| rt.pid)
                        .map(|spid| spid == pid)
                        .unwrap_or(false)
                })
                .map(|svc| svc.name.clone());

            let is_managed = matched_service.is_some();
            let (matched_project, matched_project_id) =
                match_project_ownership(projects, cwd.as_deref(), &command);

            let has_v6 = bind_addrs.iter().any(|a| a.contains(':') || a.contains('['));
            let family = if has_v6 && !bind_addrs.iter().any(|a| !a.contains(':') && a != "*") {
                AddressFamily::IPv6
            } else {
                AddressFamily::IPv4
            };

            PortEntry {
                port,
                protocol: "TCP".to_string(),
                bind_addrs,
                family,
                pid,
                process_name: proc_name,
                command,
                cwd,
                matched_project,
                matched_project_id,
                matched_service,
                is_managed,
            }
        })
        .collect();

    result.sort_by_key(|e| e.port);
    result
}

pub fn check_port_conflict(port: u16) -> Option<(u32, String)> {
    let out = Command::new("lsof")
        .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-F", "pc"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut pid = None;
    let mut name = String::new();
    for line in stdout.lines() {
        if let Some(p) = line.strip_prefix('p') {
            pid = p.parse::<u32>().ok();
        } else if let Some(c) = line.strip_prefix('c') {
            name = c.to_string();
        }
    }
    pid.map(|p| (p, name))
}

fn get_pid_command(pid: u32) -> String {
    Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
}

fn get_pid_cwd(pid: u32) -> Option<PathBuf> {
    let out = Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix('n') {
            return Some(PathBuf::from(path));
        }
    }
    None
}

fn match_project_ownership(
    projects: &[Project],
    cwd: Option<&std::path::Path>,
    command: &str,
) -> (Option<String>, Option<String>) {
    let mut best: Option<(usize, &str, &str)> = None;
    for proj in projects {
        let proj_str = proj.path.to_string_lossy();
        let cwd_match = cwd.map(|c| c.starts_with(&proj.path)).unwrap_or(false);
        let cmd_match = !proj_str.is_empty() && command.contains(proj_str.as_ref());
        if cwd_match || cmd_match {
            let len = proj_str.len();
            if best.map(|(b_len, _, _)| len > b_len).unwrap_or(true) {
                best = Some((len, proj.name.as_str(), proj.id.as_str()));
            }
        }
    }
    match best {
        Some((_, name, id)) => (Some(name.to_string()), Some(id.to_string())),
        None => (None, None),
    }
}

pub fn kill_pid(pid: u32, force: bool) -> Result<()> {
    let sig = if force { "-KILL" } else { "-TERM" };
    let status = Command::new("kill")
        .args([sig, &pid.to_string()])
        .status()
        .with_context(|| format!("Failed to invoke kill {sig} {pid}"))?;
    if !status.success() {
        anyhow::bail!("kill {sig} {pid} exited with status {status}");
    }
    Ok(())
}
