# DevNook

[简体中文](README.md) · **English**

A local project and dev service manager built with **Rust + GPUI (`gpui-kit`)**. It brings your local code repositories, preferred editors, service configs, live logs, port diagnostics, and sequential startup pipelines into one native desktop workspace, alongside a standalone CLI and read-only stdio MCP server.

## Core Features

| What you want to do | How it works in DevNook |
| --- | --- |
| **Find a project and resume coding** | Preview read-only scan candidates (including monorepo sub-service drafts), filter by name, path, tech stack, favorites, or active listening ports, toggle List/Card view, and open in Zed / VS Code / Cursor / GoLand / RustRover / Finder / Terminal. |
| **Start services and inspect health** | Save startup commands, working directories, expected ports, readiness probes, and env vars (auto-redacted in logs). Pre-flight port conflict checks block collisions; inspect readiness, actual listening ports, run history, and searchable stdout/stderr logs. |
| **Debug "port already in use"** | View merged TCP IPv4/IPv6 listeners with PID, full command, working directory, and matched project ownership. Filter by Managed/External or IPv4/IPv6; external processes require explicit PID confirmation before stopping. |
| **Prepare dependent services in order** | Compose registered services into Startup Plans. Already-ready services are reused automatically, upstream readiness is awaited before starting downstream services, and stopping a plan only stops instances started by that run. |

- **Bilingual & Themeable**: Toggle **简体中文 / English** and **Dark / Light** mode in the header. Preferences and workspace state are atomically persisted to `~/.devnook/state.json`.

## Workspace Architecture

```text
DevNook/
├── Cargo.toml                  # Workspace root (default-members = devnook-desktop)
├── .gitignore
└── crates/
    ├── devnook-core/           # Domain models, read-only scanner, port inspector, process supervisor & pipeline runner
    ├── devnook-cli/            # Lightweight CLI & read-only stdio MCP server (`devnook`)
    └── devnook-desktop/        # Native GPUI desktop application
```

## Quick Start

### Prerequisites

- **macOS** (with Xcode Command Line Tools installed)
- **Rust** `1.95+` (`stable` toolchain with `cargo`)

### Build & Run

```bash
# 1. Launch the GPUI desktop app (devnook-desktop)
cargo run

# 2. Run core library unit tests
cargo test -p devnook-core

# 3. Run the standalone CLI
cargo run -p devnook-cli -- --help
```

## CLI & Read-Only MCP

```bash
cargo run -p devnook-cli -- project list [--json]
cargo run -p devnook-cli -- service list [--json]
cargo run -p devnook-cli -- ports [--json]
cargo run -p devnook-cli -- plan list [--json]
```

After building `devnook-cli` (`cargo build --release -p devnook-cli`), configure the read-only stdio MCP server in your AI coding assistant:

```json
{
  "mcpServers": {
    "devnook": {
      "command": "/Volumes/disk/projects/rust/DevNook/target/release/devnook",
      "args": ["mcp", "serve"]
    }
  }
}
```

Available read-only MCP tools: `list_projects`, `list_services`, `list_ports`, `list_plans`.

## License

MIT
