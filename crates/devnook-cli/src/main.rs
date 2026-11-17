use devnook_core::models::AppStateData;
use devnook_core::ports::inspect_listening_ports;
use devnook_core::storage::load_state;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let json_mode = args.iter().any(|a| a == "--json");
    let positional: Vec<&str> = args[1..]
        .iter()
        .map(|s| s.as_str())
        .filter(|s| *s != "--json")
        .collect();

    let code = match positional.as_slice() {
        [] | ["--help"] | ["-h"] | ["help"] => {
            print_help();
            0
        }
        ["project", "list"] => {
            let state = load_state();
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&state.projects).unwrap_or_default()
                );
            } else {
                for p in &state.projects {
                    println!(
                        "{} [{}] {} {}",
                        if p.favorite { "★" } else { " " },
                        p.category.label(),
                        p.name,
                        p.path.display()
                    );
                }
            }
            0
        }
        ["service", "list"] => {
            let state = load_state();
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&state.services).unwrap_or_default()
                );
            } else {
                for s in &state.services {
                    let port = s
                        .expected_port
                        .map(|p| format!(":{p}"))
                        .unwrap_or_else(|| "-".to_string());
                    println!(
                        "{} ({}) | {} | {}",
                        s.name,
                        port,
                        s.command,
                        s.working_dir.display()
                    );
                }
            }
            0
        }
        ["ports"] => {
            let state = load_state();
            let entries = inspect_listening_ports(&state.projects, &state.services, &HashMap::new());
            if json_mode {
                let items: Vec<Value> = entries
                    .iter()
                    .map(|e| {
                        json!({
                            "port": e.port,
                            "pid": e.pid,
                            "process": e.process_name,
                            "bind_addrs": e.bind_addrs,
                            "command": e.command,
                            "cwd": e.cwd,
                            "project": e.matched_project,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&items).unwrap_or_default());
            } else {
                for e in &entries {
                    println!(
                        ":{:<5} PID {:<6} {:<16} project={:<15} {}",
                        e.port,
                        e.pid,
                        e.process_name,
                        e.matched_project.as_deref().unwrap_or("-"),
                        e.command
                    );
                }
            }
            0
        }
        ["plan", "list"] => {
            let state = load_state();
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&state.plans).unwrap_or_default()
                );
            } else {
                for plan in &state.plans {
                    println!("{} ({} steps) - {}", plan.name, plan.steps.len(), plan.description);
                }
            }
            0
        }
        ["mcp", "serve"] => {
            run_stdio_mcp();
            0
        }
        _ => {
            eprintln!("Unknown command. Run `devnook --help` for usage.");
            1
        }
    };

    std::process::exit(code);
}

fn print_help() {
    println!(
        "DevNook CLI & Read-Only MCP Server
Usage:
  devnook project list [--json] List imported projects
  devnook service list [--json] List registered services
  devnook ports [--json]        Inspect listening TCP ports and project ownership
  devnook plan list [--json]    List startup plans
  devnook mcp serve             Start read-only stdio MCP server"
    );
}

fn run_stdio_mcp() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let resp = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "devnook", "version": env!("CARGO_PKG_VERSION") }
                }
            }),
            "notifications/initialized" => continue,
            "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [
                        {
                            "name": "list_projects",
                            "description": "List imported DevNook projects (read-only)",
                            "inputSchema": { "type": "object", "properties": {} }
                        },
                        {
                            "name": "list_services",
                            "description": "List registered DevNook services (read-only)",
                            "inputSchema": { "type": "object", "properties": {} }
                        },
                        {
                            "name": "list_ports",
                            "description": "Inspect active TCP listening ports and matched project ownership",
                            "inputSchema": { "type": "object", "properties": {} }
                        },
                        {
                            "name": "list_plans",
                            "description": "List configured startup plans",
                            "inputSchema": { "type": "object", "properties": {} }
                        }
                    ]
                }
            }),
            "tools/call" => {
                let tool_name = req
                    .pointer("/params/name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let state: AppStateData = load_state();
                let payload = match tool_name {
                    "list_projects" => serde_json::to_string_pretty(&state.projects).unwrap_or_default(),
                    "list_services" => serde_json::to_string_pretty(&state.services).unwrap_or_default(),
                    "list_plans" => serde_json::to_string_pretty(&state.plans).unwrap_or_default(),
                    "list_ports" => {
                        let ports = inspect_listening_ports(
                            &state.projects,
                            &state.services,
                            &HashMap::new(),
                        );
                        let items: Vec<Value> = ports
                            .iter()
                            .map(|e| {
                                json!({
                                    "port": e.port,
                                    "pid": e.pid,
                                    "process": e.process_name,
                                    "bind_addrs": e.bind_addrs,
                                    "command": e.command,
                                    "cwd": e.cwd,
                                    "project": e.matched_project,
                                })
                            })
                            .collect();
                        serde_json::to_string_pretty(&items).unwrap_or_default()
                    }
                    _ => format!("Unknown tool: {tool_name}"),
                };
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": payload }]
                    }
                })
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {method}") }
            }),
        };

        let _ = writeln!(stdout, "{}", resp);
        let _ = stdout.flush();
    }
}
