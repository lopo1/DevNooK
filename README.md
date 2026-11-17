# DevNook

**简体中文** · [English](README.en.md)

基于 **Rust + GPUI (`gpui-kit`)** 构建的本地项目与开发服务管理器。将磁盘上的代码工程、常用编辑器、服务启停与日志、端口占用排查以及多服务依赖编排整合在一个原生桌面工作台中，并提供独立 CLI 与只读 stdio MCP Server。

## 核心功能

| 场景 | 在 DevNook 中的操作 |
| --- | --- |
| **找回项目并继续开发** | 只读扫描候选预览并勾选导入（含 Monorepo 子服务草稿），按名称、路径、技术栈、收藏或「监听中」筛选，切换列表/卡片视图，一键用 Zed / VS Code / Cursor / GoLand / RustRover / Finder / Terminal 打开。 |
| **启动服务与查看日志** | 保存启动命令、工作目录、期望端口、就绪探针与环境变量（日志自动脱敏），启动前检测端口冲突，确认后实时查看就绪状态、实际监听端口、历史运行记录与 stdout/stderr 日志，支持关键字搜索与流类型过滤。 |
| **排查“端口已被占用”** | 合并展示 TCP IPv4/IPv6 监听端口，查看进程 PID、完整启动命令、工作目录与项目归属；支持来源/地址族过滤，外部进程需二次确认 PID 后才可停止。 |
| **按顺序准备一组服务** | 将已登记服务编排为启动方案，自动复用已就绪实例，等待上游就绪后再启动下游；停止方案仅停止本轮由方案拉起的服务。 |

- **双语与主题**：右上角支持 **简体中文 / English** 与 **深色 / 浅色** 主题切换，配置与项目状态原子持久化至 `~/.devnook/state.json`。

## 工程结构

```text
DevNook/
├── Cargo.toml                  # Cargo Workspace 根配置 (default-members = devnook-desktop)
├── .gitignore
└── crates/
    ├── devnook-core/           # 核心领域模型、只读扫描器、端口排查、进程托管与依赖编排
    ├── devnook-cli/            # 轻量独立 CLI 与只读 stdio MCP Server (`devnook`)
    └── devnook-desktop/        # GPUI 原生桌面端界面
```

## 快速开始

### 环境要求

- **macOS**（已安装 Xcode Command Line Tools）
- **Rust** `1.95+`（`stable` 工具链，包含 `cargo`）

### 编译与运行

```bash
# 1. 启动 GPUI 桌面端 (devnook-desktop)
cargo run

# 2. 运行核心库单元测试
cargo test -p devnook-core

# 3. 运行独立 CLI
cargo run -p devnook-cli -- --help
```

## CLI 与只读 MCP 接入

```bash
cargo run -p devnook-cli -- project list [--json]
cargo run -p devnook-cli -- service list [--json]
cargo run -p devnook-cli -- ports [--json]
cargo run -p devnook-cli -- plan list [--json]
```

编译 `devnook-cli` 后，可在支持 MCP 的 AI 编程助手中配置只读 stdio MCP Server：

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

提供只读工具：`list_projects`、`list_services`、`list_ports`、`list_plans`。

## License

MIT
