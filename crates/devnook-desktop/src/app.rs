use devnook_core::models::{
    AppLanguage, AppStateData, EditorKind, HealthCheckKind, PipelineStep, PlanRunState, PortEntry,
    ProjectCategory, RunRecord, ScanCandidate, ServiceConfig, ServiceDraft, ServiceRuntime,
    StartupPlan, StepRunState, ViewMode,
};
use devnook_core::pipeline::{PipelineEvent, run_startup_plan};
use devnook_core::ports::{check_port_conflict, inspect_listening_ports, kill_pid};
use devnook_core::scanner::{
    import_single_project, open_in_editor, open_url_in_browser, preview_scan_candidates,
    scan_projects,
};
use devnook_core::service::{ServiceEvent, ServiceManager, append_log};
use devnook_core::storage::{load_state, save_state};
use crate::ui::{palette, pipelines_view, ports_view, projects_view, services_view};
use chrono::Local;
use gpui_kit::component::{
    Theme, ThemeMode,
    button::{Button, ButtonVariants as _},
    input::InputState,
};
use gpui_kit::{prelude::*, *};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Projects,
    Services,
    Ports,
    Pipelines,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStreamFilter {
    All,
    Stdout,
    Stderr,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortOwnershipFilter {
    All,
    Managed,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortFamilyFilter {
    All,
    IPv4,
    IPv6,
}

pub struct DevNookApp {
    pub active_tab: ActiveTab,
    pub data: AppStateData,
    pub status_banner: Option<String>,

    // Projects
    pub project_search_input: Entity<InputState>,
    pub import_path_input: Entity<InputState>,
    pub filter_favorites_only: bool,
    pub filter_listening_only: bool,
    pub filter_category: Option<ProjectCategory>,
    pub scan_candidates: Option<Vec<ScanCandidate>>,

    // Services
    pub selected_service_id: Option<String>,
    pub pending_project_id: Option<String>,
    pub svc_name_input: Entity<InputState>,
    pub svc_dir_input: Entity<InputState>,
    pub svc_cmd_input: Entity<InputState>,
    pub svc_port_input: Entity<InputState>,
    pub svc_health_input: Entity<InputState>,
    pub svc_env_input: Entity<InputState>,
    pub log_search_input: Entity<InputState>,
    pub log_stream_filter: LogStreamFilter,
    pub selected_run_history_idx: Option<usize>,

    // Ports
    pub port_search_input: Entity<InputState>,
    pub port_entries: Vec<PortEntry>,
    pub port_ownership_filter: PortOwnershipFilter,
    pub port_family_filter: PortFamilyFilter,
    pub confirm_kill_pid: Option<(u32, u16)>,
    pub expanded_port_cmds: HashSet<(u32, u16)>,

    // Pipelines
    pub selected_plan_id: Option<String>,
    pub plan_name_input: Entity<InputState>,
    pub plan_desc_input: Entity<InputState>,
    pub plan_run_state: PlanRunState,

    // Runtime & channels
    pub focus_handle: FocusHandle,
    pub runtimes: Arc<Mutex<HashMap<String, ServiceRuntime>>>,
    pub service_manager: Arc<ServiceManager>,
    pub pipeline_tx: mpsc::UnboundedSender<PipelineEvent>,
    _subscriptions: Vec<Subscription>,
}

impl DevNookApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut data = load_state();
        Theme::change(
            if data.dark_mode {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            None,
            cx,
        );
        if data.projects.is_empty() && !data.scan_roots.is_empty() {
            data.projects = scan_projects(&data.scan_roots, &data.projects);
            let _ = save_state(&data);
        }

        let (svc_tx, mut svc_rx) = mpsc::unbounded_channel::<ServiceEvent>();
        let (pipe_tx, mut pipe_rx) = mpsc::unbounded_channel::<PipelineEvent>();
        let service_manager = Arc::new(ServiceManager::new(svc_tx));
        let runtimes = Arc::new(Mutex::new(HashMap::new()));

        let port_entries = inspect_listening_ports(&data.projects, &data.services, &HashMap::new());

        let project_search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search projects / 搜索项目名、路径或技术栈...")
        });
        let import_path_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Paste directory path / 粘贴项目或扫描根目录...")
        });

        let svc_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Service name (e.g. API Server)"));
        let svc_dir_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Working directory"));
        let svc_cmd_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Command (e.g. npm run dev)"));
        let svc_port_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Port (e.g. 3000)"));
        let svc_health_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("tcp:3000 | http:localhost:3000/health | alive")
        });
        let svc_env_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("KEY=VAL; PORT=3000"));
        let log_search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search logs / 搜索日志关键字..."));

        let port_search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Filter port, PID, command, or project / 搜索端口或进程...")
        });

        let plan_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Plan name (e.g. Fullstack Dev)"));
        let plan_desc_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Description..."));

        let selected_service_id = data.services.first().map(|s| s.id.clone());
        let selected_plan_id = data.plans.first().map(|p| p.id.clone());

        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;

            let mut svc_events = Vec::new();
            while let Ok(ev) = svc_rx.try_recv() {
                svc_events.push(ev);
            }
            let mut pipe_events = Vec::new();
            while let Ok(ev) = pipe_rx.try_recv() {
                pipe_events.push(ev);
            }

            if svc_events.is_empty() && pipe_events.is_empty() {
                continue;
            }

            let updated = this.update(cx, |app, cx| {
                if let Ok(mut rt_map) = app.runtimes.lock() {
                    for ev in svc_events {
                        match ev {
                            ServiceEvent::StatusChanged {
                                service_id,
                                status,
                                pid,
                            } => {
                                let entry = rt_map.entry(service_id).or_default();
                                if status.is_active() && entry.started_at.is_none() {
                                    entry.started_at =
                                        Some(Local::now().format("%H:%M:%S").to_string());
                                    entry.logs.clear();
                                }
                                if !status.is_active() {
                                    if let Some(started) = entry.started_at.take() {
                                        if !entry.logs.is_empty() {
                                            entry.history.insert(
                                                0,
                                                RunRecord {
                                                    started_at: started,
                                                    ended_at: Local::now()
                                                        .format("%H:%M:%S")
                                                        .to_string(),
                                                    exit_summary: status.label(false).to_string(),
                                                    logs: entry.logs.iter().cloned().collect(),
                                                },
                                            );
                                            if entry.history.len() > 10 {
                                                entry.history.truncate(10);
                                            }
                                        }
                                    }
                                }
                                entry.status = status;
                                entry.pid = pid;
                            }
                            ServiceEvent::PortsUpdated { service_id, ports } => {
                                let entry = rt_map.entry(service_id).or_default();
                                entry.listening_ports = ports;
                            }
                            ServiceEvent::LogAppended { service_id, line } => {
                                let entry = rt_map.entry(service_id).or_default();
                                append_log(entry, line);
                            }
                        }
                    }
                }

                for ev in pipe_events {
                    match ev {
                        PipelineEvent::StepUpdated {
                            plan_id,
                            step_index,
                            state,
                        } => {
                            if app.plan_run_state.active_plan_id.as_deref() == Some(&plan_id) {
                                if let Some(slot) =
                                    app.plan_run_state.step_states.get_mut(step_index)
                                {
                                    slot.1 = state;
                                }
                            }
                        }
                        PipelineEvent::ServiceStartedByPlan {
                            plan_id,
                            service_id,
                        } => {
                            if app.plan_run_state.active_plan_id.as_deref() == Some(&plan_id) {
                                app.plan_run_state.started_by_plan.insert(service_id);
                            }
                        }
                        PipelineEvent::Finished {
                            plan_id,
                            success: _,
                            message,
                        } => {
                            if app.plan_run_state.active_plan_id.as_deref() == Some(&plan_id) {
                                app.plan_run_state.running = false;
                                app.plan_run_state.summary = Some(message.clone());
                                app.status_banner = Some(message);
                            }
                        }
                    }
                }

                cx.notify();
            });

            if updated.is_err() {
                break;
            }
        })
        .detach();

        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);

        let mut app = Self {
            active_tab: ActiveTab::Projects,
            data,
            status_banner: None,
            project_search_input,
            import_path_input,
            filter_favorites_only: false,
            filter_listening_only: false,
            filter_category: None,
            scan_candidates: None,
            selected_service_id: selected_service_id.clone(),
            pending_project_id: None,
            svc_name_input,
            svc_dir_input,
            svc_cmd_input,
            svc_port_input,
            svc_health_input,
            svc_env_input,
            log_search_input,
            log_stream_filter: LogStreamFilter::All,
            selected_run_history_idx: None,
            port_search_input,
            port_entries,
            port_ownership_filter: PortOwnershipFilter::All,
            port_family_filter: PortFamilyFilter::All,
            confirm_kill_pid: None,
            expanded_port_cmds: HashSet::new(),
            selected_plan_id: selected_plan_id.clone(),
            plan_name_input,
            plan_desc_input,
            plan_run_state: PlanRunState::default(),
            focus_handle,
            runtimes,
            service_manager,
            pipeline_tx: pipe_tx,
            _subscriptions: Vec::new(),
        };

        if let Some(sid) = selected_service_id {
            app.select_service(&sid, window, cx);
        }
        if let Some(pid) = selected_plan_id {
            app.select_plan(&pid, window, cx);
        }

        app
    }

    pub fn is_zh(&self) -> bool {
        self.data.language == AppLanguage::Zh
    }

    fn persist(&mut self) {
        if let Err(err) = save_state(&self.data) {
            self.status_banner = Some(format!("Save failed: {err}"));
        }
    }

    pub fn toggle_language(&mut self, cx: &mut Context<Self>) {
        self.data.language = match self.data.language {
            AppLanguage::Zh => AppLanguage::En,
            AppLanguage::En => AppLanguage::Zh,
        };
        self.persist();
        cx.notify();
    }

    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.data.dark_mode = !self.data.dark_mode;
        Theme::change(
            if self.data.dark_mode {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            None,
            cx,
        );
        cx.refresh_windows();
        self.persist();
        cx.notify();
    }

    pub fn toggle_view_mode(&mut self, cx: &mut Context<Self>) {
        self.data.view_mode = match self.data.view_mode {
            ViewMode::List => ViewMode::Card,
            ViewMode::Card => ViewMode::List,
        };
        self.persist();
        cx.notify();
    }

    pub fn open_scan_preview(&mut self, cx: &mut Context<Self>) {
        let candidates = preview_scan_candidates(&self.data.scan_roots, &self.data.projects);
        let count = candidates.len();
        self.scan_candidates = Some(candidates);
        self.status_banner = Some(if self.is_zh() {
            format!("扫描完成，发现 {count} 个候选项目，请勾选后导入。")
        } else {
            format!("Found {count} candidate projects.")
        });
        cx.notify();
    }

    pub fn confirm_scan_import(&mut self, cx: &mut Context<Self>) {
        if let Some(cands) = self.scan_candidates.take() {
            let mut added = 0usize;
            for cand in cands {
                if cand.selected
                    && !self
                        .data
                        .projects
                        .iter()
                        .any(|p| p.path == cand.project.path)
                {
                    self.data.projects.push(cand.project);
                    added += 1;
                }
            }
            self.data.projects.sort_by(|a, b| {
                b.favorite
                    .cmp(&a.favorite)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
            self.persist();
            self.status_banner = Some(if self.is_zh() {
                format!("已导入 {added} 个新项目。")
            } else {
                format!("Imported {added} new projects.")
            });
        }
        cx.notify();
    }

    pub fn import_project_or_root(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.import_path_input.read(cx).value().trim().to_string();
        if raw.is_empty() {
            return;
        }
        let path = PathBuf::from(&raw);
        match import_single_project(&path) {
            Ok(proj) => {
                if !self.data.scan_roots.contains(&path) {
                    self.data.scan_roots.push(path.clone());
                }
                if !self.data.projects.iter().any(|p| p.path == proj.path) {
                    self.data.projects.insert(0, proj);
                }
                self.persist();
                self.import_path_input
                    .update(cx, |inp, cx| inp.set_value("", window, cx));
                self.status_banner = Some(format!("Imported {raw}"));
            }
            Err(err) => {
                self.status_banner = Some(format!("Import failed: {err}"));
            }
        }
        cx.notify();
    }

    pub fn remove_scan_root(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.data.scan_roots.len() {
            self.data.scan_roots.remove(index);
            self.persist();
            self.status_banner = Some(if self.is_zh() {
                "已移除扫描目录。".to_string()
            } else {
                "Removed scan root.".to_string()
            });
        }
        cx.notify();
    }

    pub fn remove_project_from_library(&mut self, project_id: &str, cx: &mut Context<Self>) {
        self.data.projects.retain(|p| p.id != project_id);
        self.persist();
        self.status_banner = Some(if self.is_zh() {
            "已从项目库移除（不会删除磁盘源码）。".to_string()
        } else {
            "Removed from library (source files untouched).".to_string()
        });
        cx.notify();
    }

    pub fn toggle_favorite(&mut self, project_id: &str, cx: &mut Context<Self>) {
        if let Some(p) = self.data.projects.iter_mut().find(|p| p.id == project_id) {
            p.favorite = !p.favorite;
        }
        self.persist();
        cx.notify();
    }

    pub fn set_preferred_editor(&mut self, editor: EditorKind, cx: &mut Context<Self>) {
        self.data.preferred_editor = editor;
        self.persist();
        self.status_banner = Some(format!("Editor: {}", editor.label()));
        cx.notify();
    }

    pub fn open_project_in_editor(&mut self, project_id: &str, cx: &mut Context<Self>) {
        let editor = self
            .data
            .projects
            .iter()
            .find(|p| p.id == project_id)
            .and_then(|p| p.preferred_editor)
            .unwrap_or(self.data.preferred_editor);
        self.open_project_with(project_id, editor, cx);
    }

    pub fn open_project_with(
        &mut self,
        project_id: &str,
        editor: EditorKind,
        cx: &mut Context<Self>,
    ) {
        if let Some(p) = self.data.projects.iter_mut().find(|p| p.id == project_id) {
            match open_in_editor(&p.path, editor) {
                Ok(()) => {
                    p.last_opened_at = Some(Local::now().to_rfc3339());
                    let name = p.name.clone();
                    self.persist();
                    self.status_banner = Some(format!("Opened {name} in {}", editor.label()));
                }
                Err(err) => {
                    self.status_banner = Some(format!("Failed: {err}"));
                }
            }
        }
        cx.notify();
    }

    pub fn create_service_from_draft(
        &mut self,
        project_id: &str,
        draft: &ServiceDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(p) = self.data.projects.iter().find(|p| p.id == project_id).cloned() {
            self.selected_service_id = None;
            self.pending_project_id = Some(p.id.clone());
            self.svc_name_input.update(cx, |inp, cx| {
                inp.set_value(format!("{}-{}", p.name, draft.name), window, cx)
            });
            let dir = if draft.relative_dir == "." {
                p.path
            } else {
                p.path.join(&draft.relative_dir)
            };
            self.svc_dir_input.update(cx, |inp, cx| {
                inp.set_value(dir.to_string_lossy().to_string(), window, cx)
            });
            self.svc_cmd_input
                .update(cx, |inp, cx| inp.set_value(draft.command.clone(), window, cx));
            self.svc_port_input.update(cx, |inp, cx| {
                inp.set_value(
                    draft.expected_port.map(|v| v.to_string()).unwrap_or_default(),
                    window,
                    cx,
                )
            });
            let probe = draft
                .expected_port
                .map(|v| format!("tcp:{v}"))
                .unwrap_or_else(|| "alive".to_string());
            self.svc_health_input
                .update(cx, |inp, cx| inp.set_value(probe, window, cx));
        }
        cx.notify();
    }

    pub fn prepare_service_from_project(
        &mut self,
        project_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(p) = self.data.projects.iter().find(|p| p.id == project_id).cloned() {
            self.selected_service_id = None;
            self.pending_project_id = Some(p.id.clone());
            self.svc_name_input
                .update(cx, |inp, cx| inp.set_value(p.name.clone(), window, cx));
            self.svc_dir_input.update(cx, |inp, cx| {
                inp.set_value(p.path.to_string_lossy().to_string(), window, cx)
            });
            self.svc_cmd_input.update(cx, |inp, cx| {
                inp.set_value(p.default_command.unwrap_or_default(), window, cx)
            });
            self.svc_port_input
                .update(cx, |inp, cx| inp.set_value("", window, cx));
            self.svc_health_input
                .update(cx, |inp, cx| inp.set_value("alive", window, cx));
        }
        cx.notify();
    }

    pub fn reset_service_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_service_id = None;
        self.pending_project_id = None;
        self.selected_run_history_idx = None;
        for inp in [
            &self.svc_name_input,
            &self.svc_dir_input,
            &self.svc_cmd_input,
            &self.svc_port_input,
            &self.svc_health_input,
            &self.svc_env_input,
        ] {
            inp.update(cx, |i, cx| i.set_value("", window, cx));
        }
        cx.notify();
    }

    pub fn select_service(
        &mut self,
        service_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_service_id = Some(service_id.to_string());
        self.selected_run_history_idx = None;
        if let Some(svc) = self.data.services.iter().find(|s| s.id == service_id).cloned() {
            self.pending_project_id = svc.project_id.clone();
            self.svc_name_input
                .update(cx, |i, cx| i.set_value(svc.name, window, cx));
            self.svc_dir_input.update(cx, |i, cx| {
                i.set_value(svc.working_dir.to_string_lossy().to_string(), window, cx)
            });
            self.svc_cmd_input
                .update(cx, |i, cx| i.set_value(svc.command, window, cx));
            self.svc_port_input.update(cx, |i, cx| {
                i.set_value(
                    svc.expected_port.map(|p| p.to_string()).unwrap_or_default(),
                    window,
                    cx,
                )
            });
            let health_str = match svc.health_check {
                HealthCheckKind::ProcessAlive => "alive".to_string(),
                HealthCheckKind::TcpPort(p) => format!("tcp:{p}"),
                HealthCheckKind::HttpGet(u) => u,
                HealthCheckKind::LogPattern(pat) => format!("log:{pat}"),
            };
            self.svc_health_input
                .update(cx, |i, cx| i.set_value(health_str, window, cx));
            let env_str = svc
                .env_vars
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("; ");
            self.svc_env_input
                .update(cx, |i, cx| i.set_value(env_str, window, cx));
        }
        cx.notify();
    }

    pub fn save_service_from_form(&mut self, cx: &mut Context<Self>) {
        let name = self.svc_name_input.read(cx).value().trim().to_string();
        let dir = self.svc_dir_input.read(cx).value().trim().to_string();
        let cmd = self.svc_cmd_input.read(cx).value().trim().to_string();
        let port_raw = self.svc_port_input.read(cx).value().trim().to_string();
        let health_raw = self.svc_health_input.read(cx).value().trim().to_string();
        let env_raw = self.svc_env_input.read(cx).value().trim().to_string();

        if name.is_empty() || dir.is_empty() || cmd.is_empty() {
            self.status_banner = Some(if self.is_zh() {
                "服务名称、工作目录和启动命令不能为空。".to_string()
            } else {
                "Service name, working directory, and command are required.".to_string()
            });
            cx.notify();
            return;
        }

        let expected_port = port_raw.parse::<u16>().ok();
        let health_check = parse_health_check(&health_raw, expected_port);
        let env_vars: Vec<(String, String)> = env_raw
            .split([';', '\n'])
            .filter_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                let key = k.trim().to_string();
                if key.is_empty() {
                    None
                } else {
                    Some((key, v.trim().to_string()))
                }
            })
            .collect();

        if let Some(existing_id) = self.selected_service_id.clone() {
            if let Some(svc) = self.data.services.iter_mut().find(|s| s.id == existing_id) {
                svc.name = name.clone();
                svc.working_dir = PathBuf::from(dir);
                svc.command = cmd;
                svc.expected_port = expected_port;
                svc.health_check = health_check;
                svc.env_vars = env_vars;
            }
        } else {
            let new_id = Uuid::new_v4().to_string();
            let svc = ServiceConfig {
                id: new_id.clone(),
                name: name.clone(),
                project_id: self.pending_project_id.clone(),
                working_dir: PathBuf::from(dir),
                command: cmd,
                expected_port,
                health_check,
                env_vars,
                is_web: expected_port.map(|p| p == 3000 || p == 5173 || p == 4321).unwrap_or(false),
            };
            self.data.services.push(svc);
            self.selected_service_id = Some(new_id);
        }

        self.persist();
        self.status_banner = Some(format!("Saved '{name}'."));
        cx.notify();
    }

    pub fn start_service(&mut self, service_id: &str, cx: &mut Context<Self>) {
        if let Some(cfg) = self.data.services.iter().find(|s| s.id == service_id).cloned() {
            if let Some(port) = cfg.expected_port {
                if let Some((pid, proc_name)) = check_port_conflict(port) {
                    self.status_banner = Some(if self.is_zh() {
                        format!("端口 :{port} 已被外部进程 {proc_name} (PID {pid}) 占用，已阻止启动。")
                    } else {
                        format!("Port :{port} is already occupied by {proc_name} (PID {pid}).")
                    });
                    cx.notify();
                    return;
                }
            }
            self.service_manager.start_service(cfg.clone());
            self.status_banner = Some(format!("Starting '{}'...", cfg.name));
        }
        cx.notify();
    }

    pub fn stop_service(&mut self, service_id: &str, cx: &mut Context<Self>) {
        self.service_manager.stop_service(service_id);
        self.status_banner = Some("Sent stop signal.".to_string());
        cx.notify();
    }

    pub fn restart_service(&mut self, service_id: &str, cx: &mut Context<Self>) {
        self.service_manager.stop_service(service_id);
        if let Some(cfg) = self.data.services.iter().find(|s| s.id == service_id).cloned() {
            let mgr = self.service_manager.clone();
            mgr.rt_handle().spawn(async move {
                tokio::time::sleep(Duration::from_millis(600)).await;
                mgr.start_service(cfg);
            });
            self.status_banner = Some("Restarting service...".to_string());
        }
        cx.notify();
    }

    pub fn open_browser_url(&mut self, url: &str, cx: &mut Context<Self>) {
        match open_url_in_browser(url) {
            Ok(()) => self.status_banner = Some(format!("Opened {url}")),
            Err(err) => self.status_banner = Some(format!("Browser error: {err}")),
        }
        cx.notify();
    }

    pub fn delete_service(
        &mut self,
        service_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.service_manager.stop_service(service_id);
        self.data.services.retain(|s| s.id != service_id);
        for plan in &mut self.data.plans {
            plan.steps.retain(|st| st.service_id != service_id);
        }
        self.persist();
        self.reset_service_form(window, cx);
        self.status_banner = Some("Deleted service.".to_string());
        cx.notify();
    }

    pub fn clear_service_logs(&mut self, service_id: &str, cx: &mut Context<Self>) {
        if let Ok(mut map) = self.runtimes.lock() {
            if let Some(rt) = map.get_mut(service_id) {
                rt.logs.clear();
            }
        }
        cx.notify();
    }

    pub fn refresh_ports(&mut self, cx: &mut Context<Self>) {
        let rt_snapshot = self.runtimes.lock().map(|m| m.clone()).unwrap_or_default();
        self.port_entries =
            inspect_listening_ports(&self.data.projects, &self.data.services, &rt_snapshot);
        self.status_banner = Some(format!("{} listening ports.", self.port_entries.len()));
        cx.notify();
    }

    pub fn kill_port_pid(&mut self, pid: u32, force: bool, cx: &mut Context<Self>) {
        match kill_pid(pid, force) {
            Ok(()) => {
                self.status_banner = Some(format!(
                    "Sent {} to PID {pid}.",
                    if force { "SIGKILL" } else { "SIGTERM" }
                ));
                self.refresh_ports(cx);
            }
            Err(err) => {
                self.status_banner = Some(format!("Failed to kill PID {pid}: {err}"));
                cx.notify();
            }
        }
    }

    pub fn create_new_plan(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let new_id = Uuid::new_v4().to_string();
        let plan = StartupPlan {
            id: new_id.clone(),
            name: format!("Startup Plan {}", self.data.plans.len() + 1),
            description: "Sequential service orchestration".to_string(),
            steps: Vec::new(),
        };
        self.data.plans.push(plan);
        self.persist();
        self.select_plan(&new_id, window, cx);
    }

    pub fn select_plan(&mut self, plan_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_plan_id = Some(plan_id.to_string());
        if let Some(p) = self.data.plans.iter().find(|p| p.id == plan_id).cloned() {
            self.plan_name_input
                .update(cx, |i, cx| i.set_value(p.name, window, cx));
            self.plan_desc_input
                .update(cx, |i, cx| i.set_value(p.description, window, cx));
        }
        cx.notify();
    }

    pub fn save_current_plan(&mut self, plan_id: &str, cx: &mut Context<Self>) {
        let name = self.plan_name_input.read(cx).value().trim().to_string();
        let desc = self.plan_desc_input.read(cx).value().trim().to_string();
        if let Some(p) = self.data.plans.iter_mut().find(|p| p.id == plan_id) {
            if !name.is_empty() {
                p.name = name;
            }
            p.description = desc;
        }
        self.persist();
        self.status_banner = Some("Saved startup plan.".to_string());
        cx.notify();
    }

    pub fn delete_plan(&mut self, plan_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.data.plans.retain(|p| p.id != plan_id);
        self.persist();
        if let Some(first_id) = self.data.plans.first().map(|p| p.id.clone()) {
            self.select_plan(&first_id, window, cx);
        } else {
            self.selected_plan_id = None;
            cx.notify();
        }
    }

    pub fn add_step_to_plan(&mut self, plan_id: &str, service_id: &str, cx: &mut Context<Self>) {
        if let Some(p) = self.data.plans.iter_mut().find(|p| p.id == plan_id) {
            p.steps.push(PipelineStep {
                service_id: service_id.to_string(),
                wait_for_ready: true,
                timeout_secs: 30,
                delay_after_ms: 200,
            });
        }
        self.persist();
        cx.notify();
    }

    pub fn toggle_step_wait(&mut self, plan_id: &str, index: usize, cx: &mut Context<Self>) {
        if let Some(p) = self.data.plans.iter_mut().find(|p| p.id == plan_id) {
            if let Some(step) = p.steps.get_mut(index) {
                step.wait_for_ready = !step.wait_for_ready;
            }
        }
        self.persist();
        cx.notify();
    }

    pub fn move_plan_step(
        &mut self,
        plan_id: &str,
        index: usize,
        delta: isize,
        cx: &mut Context<Self>,
    ) {
        if let Some(p) = self.data.plans.iter_mut().find(|p| p.id == plan_id) {
            let target = index as isize + delta;
            if target >= 0 && (target as usize) < p.steps.len() {
                p.steps.swap(index, target as usize);
                self.persist();
            }
        }
        cx.notify();
    }

    pub fn remove_plan_step(&mut self, plan_id: &str, index: usize, cx: &mut Context<Self>) {
        if let Some(p) = self.data.plans.iter_mut().find(|p| p.id == plan_id) {
            if index < p.steps.len() {
                p.steps.remove(index);
                self.persist();
            }
        }
        cx.notify();
    }

    pub fn run_plan(&mut self, plan_id: &str, cx: &mut Context<Self>) {
        let Some(plan) = self.data.plans.iter().find(|p| p.id == plan_id).cloned() else {
            return;
        };
        if plan.steps.is_empty() {
            self.status_banner = Some("Add at least one service step first.".to_string());
            cx.notify();
            return;
        }

        self.plan_run_state = PlanRunState {
            running: true,
            active_plan_id: Some(plan.id.clone()),
            step_states: plan
                .steps
                .iter()
                .map(|s| (s.service_id.clone(), StepRunState::Pending))
                .collect(),
            started_by_plan: HashSet::new(),
            summary: Some(format!("Running '{}' ({} steps)...", plan.name, plan.steps.len())),
        };

        run_startup_plan(
            plan,
            self.data.services.clone(),
            self.runtimes.clone(),
            self.service_manager.clone(),
            self.pipeline_tx.clone(),
        );
        cx.notify();
    }

    pub fn stop_plan_services(&mut self, _plan_id: &str, cx: &mut Context<Self>) {
        let started: Vec<String> = self.plan_run_state.started_by_plan.iter().cloned().collect();
        for sid in &started {
            self.service_manager.stop_service(sid);
        }
        self.plan_run_state.running = false;
        self.plan_run_state.summary = Some(if self.is_zh() {
            format!("已停止本次方案启动的 {} 个服务（原有运行服务已保留）。", started.len())
        } else {
            format!("Stopped {} services started by this plan.", started.len())
        });
        cx.notify();
    }
}

fn parse_health_check(raw: &str, expected_port: Option<u16>) -> HealthCheckKind {
    let trimmed = raw.trim();
    if let Some(port_str) = trimmed.strip_prefix("tcp:") {
        if let Ok(p) = port_str.trim().parse::<u16>() {
            return HealthCheckKind::TcpPort(p);
        }
    }
    if let Some(rest) = trimmed.strip_prefix("http:") {
        let clean = rest.trim_start_matches('/');
        return HealthCheckKind::HttpGet(clean.to_string());
    }
    if let Some(pat) = trimmed.strip_prefix("log:") {
        if !pat.trim().is_empty() {
            return HealthCheckKind::LogPattern(pat.trim().to_string());
        }
    }
    if trimmed == "alive" {
        return HealthCheckKind::ProcessAlive;
    }
    if let Some(p) = expected_port {
        return HealthCheckKind::TcpPort(p);
    }
    HealthCheckKind::ProcessAlive
}

impl Render for DevNookApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.active_tab;
        let zh = self.is_zh();
        let pal = palette(self.data.dark_mode);

        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    if this.scan_candidates.is_some() {
                        this.scan_candidates = None;
                        cx.notify();
                    } else if window.is_fullscreen() {
                        window.toggle_fullscreen();
                    }
                } else if event.keystroke.key == "f"
                    && event.keystroke.modifiers.control
                    && event.keystroke.modifiers.platform
                {
                    window.toggle_fullscreen();
                }
            }))
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_3()
            .bg(rgb(pal.bg))
            .text_color(rgb(pal.text))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pb_2()
                    .border_b_1()
                    .border_color(rgb(pal.border))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().text_xl().font_weight(FontWeight::BOLD).child("DevNook"))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        Button::new("tab-projects")
                                            .when(tab == ActiveTab::Projects, |b| b.primary())
                                            .when(tab != ActiveTab::Projects, |b| b.ghost())
                                            .label(if zh { "项目库" } else { "Projects" })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.active_tab = ActiveTab::Projects;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("tab-services")
                                            .when(tab == ActiveTab::Services, |b| b.primary())
                                            .when(tab != ActiveTab::Services, |b| b.ghost())
                                            .label(if zh { "服务与日志" } else { "Services & Logs" })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.active_tab = ActiveTab::Services;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("tab-ports")
                                            .when(tab == ActiveTab::Ports, |b| b.primary())
                                            .when(tab != ActiveTab::Ports, |b| b.ghost())
                                            .label(if zh { "端口管理" } else { "Port Manager" })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.active_tab = ActiveTab::Ports;
                                                this.refresh_ports(cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("tab-pipelines")
                                            .when(tab == ActiveTab::Pipelines, |b| b.primary())
                                            .when(tab != ActiveTab::Pipelines, |b| b.ghost())
                                            .label(if zh { "启动方案" } else { "Startup Plans" })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.active_tab = ActiveTab::Pipelines;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when_some(self.status_banner.clone(), |d, msg| {
                                d.child(
                                    div()
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .bg(rgb(0x1e293b))
                                        .text_xs()
                                        .text_color(rgb(0x93c5fd))
                                        .child(msg),
                                )
                            })
                            .child(
                                Button::new("toggle-lang")
                                    .ghost()
                                    .label(if zh { "中 / EN" } else { "EN / 中" })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_language(cx);
                                    })),
                            )
                            .child(
                                Button::new("toggle-theme")
                                    .ghost()
                                    .label(if self.data.dark_mode {
                                        if zh { "浅色" } else { "Light" }
                                    } else if zh {
                                        "暗色"
                                    } else {
                                        "Dark"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_theme(cx);
                                    })),
                            ),
                    ),
            )
            .child(div().flex_1().min_h_0().child(match self.active_tab {
                ActiveTab::Projects => projects_view::render_projects_view(self, cx).into_any_element(),
                ActiveTab::Services => services_view::render_services_view(self, cx).into_any_element(),
                ActiveTab::Ports => ports_view::render_ports_view(self, cx).into_any_element(),
                ActiveTab::Pipelines => {
                    pipelines_view::render_pipelines_view(self, cx).into_any_element()
                }
            }))
    }
}
