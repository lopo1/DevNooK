use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectCategory {
    Rust,
    Node,
    Go,
    Python,
    Java,
    Docker,
    Other,
}

impl ProjectCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Node => "Node / Web",
            Self::Go => "Go",
            Self::Python => "Python",
            Self::Java => "Java / JVM",
            Self::Docker => "Docker",
            Self::Other => "Other",
        }
    }

    pub fn all() -> &'static [ProjectCategory] {
        &[
            Self::Rust,
            Self::Node,
            Self::Go,
            Self::Python,
            Self::Java,
            Self::Docker,
            Self::Other,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorKind {
    Zed,
    VSCode,
    Cursor,
    GoLand,
    RustRover,
    IntelliJ,
    Terminal,
    Finder,
}

impl EditorKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Zed => "Zed",
            Self::VSCode => "VS Code",
            Self::Cursor => "Cursor",
            Self::GoLand => "GoLand",
            Self::RustRover => "RustRover",
            Self::IntelliJ => "IntelliJ IDEA",
            Self::Terminal => "Terminal",
            Self::Finder => "Finder",
        }
    }

    pub fn all() -> &'static [EditorKind] {
        &[
            Self::Zed,
            Self::VSCode,
            Self::Cursor,
            Self::GoLand,
            Self::RustRover,
            Self::IntelliJ,
            Self::Terminal,
            Self::Finder,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppLanguage {
    #[default]
    Zh,
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ViewMode {
    #[default]
    List,
    Card,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDraft {
    pub name: String,
    pub relative_dir: String,
    pub command: String,
    pub expected_port: Option<u16>,
    pub is_web: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub category: ProjectCategory,
    pub favorite: bool,
    pub git_branch: Option<String>,
    pub tags: Vec<String>,
    pub default_command: Option<String>,
    #[serde(default)]
    pub preferred_editor: Option<EditorKind>,
    #[serde(default)]
    pub service_drafts: Vec<ServiceDraft>,
    pub last_opened_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScanCandidate {
    pub project: Project,
    pub selected: bool,
    pub already_imported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthCheckKind {
    ProcessAlive,
    TcpPort(u16),
    HttpGet(String),
    LogPattern(String),
}

impl HealthCheckKind {
    pub fn summary(&self) -> String {
        match self {
            Self::ProcessAlive => "Process Alive".to_string(),
            Self::TcpPort(p) => format!("TCP :{p}"),
            Self::HttpGet(url) => format!("HTTP {url}"),
            Self::LogPattern(pat) => format!("Log /{pat}/"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub id: String,
    pub name: String,
    pub project_id: Option<String>,
    pub working_dir: PathBuf,
    pub command: String,
    pub expected_port: Option<u16>,
    pub health_check: HealthCheckKind,
    #[serde(default)]
    pub env_vars: Vec<(String, String)>,
    #[serde(default)]
    pub is_web: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceStatus {
    Stopped,
    Starting,
    Ready,
    Unhealthy(String),
    Failed(String),
}

impl ServiceStatus {
    pub fn label(&self, zh: bool) -> &str {
        match (self, zh) {
            (Self::Stopped, true) => "已停止",
            (Self::Stopped, false) => "Stopped",
            (Self::Starting, true) => "启动中...",
            (Self::Starting, false) => "Starting...",
            (Self::Ready, true) => "就绪",
            (Self::Ready, false) => "Ready",
            (Self::Unhealthy(_), true) => "异常",
            (Self::Unhealthy(_), false) => "Unhealthy",
            (Self::Failed(_), true) => "失败",
            (Self::Failed(_), false) => "Failed",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Starting | Self::Ready | Self::Unhealthy(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    Stdout,
    Stderr,
    System,
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub timestamp: String,
    pub stream: LogStream,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct RunRecord {
    pub started_at: String,
    pub ended_at: String,
    pub exit_summary: String,
    pub logs: Vec<LogLine>,
}

#[derive(Debug, Clone)]
pub struct ServiceRuntime {
    pub status: ServiceStatus,
    pub pid: Option<u32>,
    pub listening_ports: Vec<u16>,
    pub started_at: Option<String>,
    pub logs: VecDeque<LogLine>,
    pub history: Vec<RunRecord>,
}

impl Default for ServiceRuntime {
    fn default() -> Self {
        Self {
            status: ServiceStatus::Stopped,
            pid: None,
            listening_ports: Vec::new(),
            started_at: None,
            logs: VecDeque::with_capacity(2000),
            history: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressFamily {
    IPv4,
    IPv6,
}

#[derive(Debug, Clone)]
pub struct PortEntry {
    pub port: u16,
    pub protocol: String,
    pub bind_addrs: Vec<String>,
    pub family: AddressFamily,
    pub pid: u32,
    pub process_name: String,
    pub command: String,
    pub cwd: Option<PathBuf>,
    pub matched_project: Option<String>,
    pub matched_project_id: Option<String>,
    pub matched_service: Option<String>,
    pub is_managed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStep {
    pub service_id: String,
    pub wait_for_ready: bool,
    pub timeout_secs: u64,
    pub delay_after_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupPlan {
    pub id: String,
    pub name: String,
    pub description: String,
    pub steps: Vec<PipelineStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepRunState {
    Pending,
    Reused,
    Starting,
    WaitingReady,
    Done,
    Failed(String),
}

impl StepRunState {
    pub fn label(&self, zh: bool) -> &str {
        match (self, zh) {
            (Self::Pending, true) => "待执行",
            (Self::Pending, false) => "Pending",
            (Self::Reused, true) => "已复用",
            (Self::Reused, false) => "Reused",
            (Self::Starting, true) => "启动中",
            (Self::Starting, false) => "Starting",
            (Self::WaitingReady, true) => "等待就绪",
            (Self::WaitingReady, false) => "Waiting Ready",
            (Self::Done, true) => "就绪",
            (Self::Done, false) => "Ready",
            (Self::Failed(_), true) => "失败",
            (Self::Failed(_), false) => "Failed",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlanRunState {
    pub running: bool,
    pub active_plan_id: Option<String>,
    pub step_states: Vec<(String, StepRunState)>,
    pub started_by_plan: HashSet<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateData {
    pub scan_roots: Vec<PathBuf>,
    pub preferred_editor: EditorKind,
    #[serde(default)]
    pub language: AppLanguage,
    #[serde(default)]
    pub dark_mode: bool,
    #[serde(default)]
    pub view_mode: ViewMode,
    pub projects: Vec<Project>,
    pub services: Vec<ServiceConfig>,
    pub plans: Vec<StartupPlan>,
}

impl Default for AppStateData {
    fn default() -> Self {
        let mut default_roots = Vec::new();
        if let Some(home) = dirs::home_dir() {
            let projects_dir = home.join("Projects");
            if projects_dir.exists() {
                default_roots.push(projects_dir);
            }
        }
        let disk_projects = PathBuf::from("/Volumes/disk/projects");
        if disk_projects.exists() {
            default_roots.push(disk_projects);
        }

        Self {
            scan_roots: default_roots,
            preferred_editor: EditorKind::Zed,
            language: AppLanguage::Zh,
            dark_mode: true,
            view_mode: ViewMode::List,
            projects: Vec::new(),
            services: Vec::new(),
            plans: Vec::new(),
        }
    }
}
