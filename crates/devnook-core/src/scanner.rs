use crate::models::{EditorKind, Project, ProjectCategory, ScanCandidate, ServiceDraft};
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;
use walkdir::{DirEntry, WalkDir};

const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    ".next",
    "dist",
    "build",
    "vendor",
    ".venv",
    "venv",
    "__pycache__",
    ".idea",
    ".vscode",
    ".turbo",
    ".cargo",
];

fn is_ignored(entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    entry
        .file_name()
        .to_str()
        .map(|name| IGNORED_DIRS.contains(&name))
        .unwrap_or(false)
}

fn detect_git_branch(dir: &Path) -> Option<String> {
    let head_path = dir.join(".git").join("HEAD");
    let content = fs::read_to_string(head_path).ok()?;
    let trimmed = content.trim();
    if let Some(rest) = trimmed.strip_prefix("ref: refs/heads/") {
        Some(rest.to_string())
    } else if trimmed.len() >= 7 {
        Some(trimmed[..7].to_string())
    } else {
        None
    }
}

fn detect_project_meta(
    dir: &Path,
) -> Option<(ProjectCategory, Vec<String>, Option<String>, Vec<ServiceDraft>)> {
    let cargo_toml = dir.join("Cargo.toml");
    let package_json = dir.join("package.json");
    let go_mod = dir.join("go.mod");
    let pyproject = dir.join("pyproject.toml");
    let requirements = dir.join("requirements.txt");
    let pom_xml = dir.join("pom.xml");
    let build_gradle = dir.join("build.gradle");
    let compose = dir.join("docker-compose.yml");
    let compose_yaml = dir.join("docker-compose.yaml");

    let mut drafts = Vec::new();

    if cargo_toml.exists() {
        let mut tags = vec!["cargo".to_string()];
        let mut is_web = false;
        if let Ok(raw) = fs::read_to_string(&cargo_toml) {
            if raw.contains("gpui") {
                tags.push("gpui".to_string());
            }
            if raw.contains("tokio") {
                tags.push("tokio".to_string());
            }
            if raw.contains("axum") || raw.contains("actix") || raw.contains("rocket") {
                tags.push("web-api".to_string());
                is_web = true;
            }
        }
        drafts.push(ServiceDraft {
            name: "cargo-run".to_string(),
            relative_dir: ".".to_string(),
            command: "cargo run".to_string(),
            expected_port: if is_web { Some(8080) } else { None },
            is_web,
        });
        return Some((
            ProjectCategory::Rust,
            tags,
            Some("cargo run".to_string()),
            drafts,
        ));
    }

    if package_json.exists() {
        let mut tags = vec!["npm".to_string()];
        let pm = if dir.join("pnpm-lock.yaml").exists() {
            "pnpm"
        } else if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
            "bun"
        } else if dir.join("yarn.lock").exists() {
            "yarn"
        } else {
            "npm run"
        };

        let mut default_cmd = Some(format!("{pm} dev"));
        if let Ok(raw) = fs::read_to_string(&package_json) {
            let is_next = raw.contains("\"next\"");
            let is_vite = raw.contains("\"vite\"");
            let is_react = raw.contains("\"react\"");
            let is_nest = raw.contains("\"@nestjs/core\"");

            if is_next {
                tags.push("nextjs".to_string());
            } else if is_vite {
                tags.push("vite".to_string());
            } else if is_react {
                tags.push("react".to_string());
            }
            if is_nest {
                tags.push("nestjs".to_string());
            }

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(scripts) = val.get("scripts").and_then(|s| s.as_object()) {
                    for key in ["dev", "start", "serve"] {
                        if scripts.contains_key(key) {
                            let cmd = if pm == "npm run" {
                                format!("npm run {key}")
                            } else {
                                format!("{pm} {key}")
                            };
                            if key == "dev" || default_cmd.is_none() {
                                default_cmd = Some(cmd.clone());
                            }
                            let port = if is_vite {
                                Some(5173)
                            } else if is_next || is_react || is_nest {
                                Some(3000)
                            } else {
                                None
                            };
                            drafts.push(ServiceDraft {
                                name: key.to_string(),
                                relative_dir: ".".to_string(),
                                command: cmd,
                                expected_port: port,
                                is_web: is_next || is_vite || is_react,
                            });
                        }
                    }
                }
            }
        }
        if drafts.is_empty() {
            if let Some(ref cmd) = default_cmd {
                drafts.push(ServiceDraft {
                    name: "dev".to_string(),
                    relative_dir: ".".to_string(),
                    command: cmd.clone(),
                    expected_port: Some(3000),
                    is_web: true,
                });
            }
        }
        return Some((ProjectCategory::Node, tags, default_cmd, drafts));
    }

    if go_mod.exists() {
        drafts.push(ServiceDraft {
            name: "go-run".to_string(),
            relative_dir: ".".to_string(),
            command: "go run .".to_string(),
            expected_port: Some(8080),
            is_web: false,
        });
        return Some((
            ProjectCategory::Go,
            vec!["go".to_string()],
            Some("go run .".to_string()),
            drafts,
        ));
    }

    if pyproject.exists() || requirements.exists() {
        let mut tags = vec!["python".to_string()];
        let mut cmd = "python3 main.py".to_string();
        if let Ok(raw) = fs::read_to_string(&pyproject).or_else(|_| fs::read_to_string(&requirements))
        {
            if raw.contains("fastapi") || raw.contains("uvicorn") {
                tags.push("fastapi".to_string());
                cmd = "uvicorn main:app --reload".to_string();
            } else if raw.contains("django") {
                tags.push("django".to_string());
                cmd = "python3 manage.py runserver".to_string();
            } else if raw.contains("flask") {
                tags.push("flask".to_string());
                cmd = "flask run".to_string();
            }
        }
        drafts.push(ServiceDraft {
            name: "python-app".to_string(),
            relative_dir: ".".to_string(),
            command: cmd.clone(),
            expected_port: Some(8000),
            is_web: false,
        });
        return Some((ProjectCategory::Python, tags, Some(cmd), drafts));
    }

    if pom_xml.exists() || build_gradle.exists() {
        let cmd = if pom_xml.exists() {
            "mvn spring-boot:run"
        } else {
            "./gradlew bootRun"
        };
        drafts.push(ServiceDraft {
            name: "jvm-app".to_string(),
            relative_dir: ".".to_string(),
            command: cmd.to_string(),
            expected_port: Some(8080),
            is_web: false,
        });
        return Some((
            ProjectCategory::Java,
            vec!["java".to_string()],
            Some(cmd.to_string()),
            drafts,
        ));
    }

    if compose.exists() || compose_yaml.exists() {
        drafts.push(ServiceDraft {
            name: "compose".to_string(),
            relative_dir: ".".to_string(),
            command: "docker compose up".to_string(),
            expected_port: None,
            is_web: false,
        });
        return Some((
            ProjectCategory::Docker,
            vec!["docker-compose".to_string()],
            Some("docker compose up".to_string()),
            drafts,
        ));
    }

    None
}

fn collect_monorepo_drafts(root: &Path) -> Vec<ServiceDraft> {
    let mut sub_drafts = Vec::new();
    let sub_containers = ["apps", "services", "packages"];
    let direct_names = ["frontend", "backend", "api", "web", "server", "client"];

    let mut candidate_dirs: Vec<(String, PathBuf)> = Vec::new();
    for name in direct_names {
        let p = root.join(name);
        if p.is_dir() {
            candidate_dirs.push((name.to_string(), p));
        }
    }
    for container in sub_containers {
        let cdir = root.join(container);
        if let Ok(entries) = fs::read_dir(&cdir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                        candidate_dirs.push((format!("{container}/{name}"), p));
                    }
                }
            }
        }
    }

    for (rel, sub_path) in candidate_dirs {
        if let Some((_, _, _, drafts)) = detect_project_meta(&sub_path) {
            for mut d in drafts {
                d.relative_dir = rel.clone();
                d.name = format!("{rel}:{}", d.name);
                sub_drafts.push(d);
            }
        }
    }
    sub_drafts
}

pub fn preview_scan_candidates(
    roots: &[PathBuf],
    existing: &[Project],
) -> Vec<ScanCandidate> {
    let existing_by_path: HashMap<PathBuf, &Project> =
        existing.iter().map(|p| (p.path.clone(), p)).collect();
    let mut candidates = Vec::new();
    let mut seen_paths = HashSet::new();

    for root in roots {
        if !root.exists() {
            continue;
        }
        let walker = WalkDir::new(root)
            .max_depth(4)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !is_ignored(e));

        for entry in walker.flatten() {
            if !entry.file_type().is_dir() {
                continue;
            }
            let dir = entry.path().to_path_buf();
            if seen_paths.contains(&dir) {
                continue;
            }
            if let Some((category, tags, default_command, mut service_drafts)) =
                detect_project_meta(&dir)
            {
                let sub_drafts = collect_monorepo_drafts(&dir);
                for sd in sub_drafts {
                    if !service_drafts.iter().any(|d| d.name == sd.name) {
                        service_drafts.push(sd);
                    }
                }
                seen_paths.insert(dir.clone());
                let name = dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unnamed")
                    .to_string();
                let git_branch = detect_git_branch(&dir);
                let already_imported = existing_by_path.contains_key(&dir);

                let project = if let Some(prev) = existing_by_path.get(&dir) {
                    let mut p = (*prev).clone();
                    p.category = category;
                    p.git_branch = git_branch;
                    p.service_drafts = service_drafts;
                    p
                } else {
                    Project {
                        id: Uuid::new_v4().to_string(),
                        name,
                        path: dir,
                        category,
                        favorite: false,
                        git_branch,
                        tags,
                        default_command,
                        preferred_editor: None,
                        service_drafts,
                        last_opened_at: None,
                    }
                };

                candidates.push(ScanCandidate {
                    project,
                    selected: true,
                    already_imported,
                });
            }
        }
    }

    candidates.sort_by(|a, b| {
        a.already_imported
            .cmp(&b.already_imported)
            .then_with(|| a.project.name.to_lowercase().cmp(&b.project.name.to_lowercase()))
    });
    candidates
}

pub fn scan_projects(roots: &[PathBuf], existing: &[Project]) -> Vec<Project> {
    let candidates = preview_scan_candidates(roots, existing);
    let mut found: Vec<Project> = candidates.into_iter().map(|c| c.project).collect();
    let seen: HashSet<PathBuf> = found.iter().map(|p| p.path.clone()).collect();

    for prev in existing {
        if !seen.contains(&prev.path) && prev.path.exists() {
            found.push(prev.clone());
        }
    }

    found.sort_by(|a, b| {
        b.favorite
            .cmp(&a.favorite)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    found
}

pub fn import_single_project(path: &Path) -> Result<Project> {
    if !path.exists() || !path.is_dir() {
        anyhow::bail!("Directory does not exist: {:?}", path);
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();
    let (category, tags, default_command, service_drafts) =
        detect_project_meta(path).unwrap_or((ProjectCategory::Other, vec![], None, vec![]));
    let git_branch = detect_git_branch(path);

    Ok(Project {
        id: Uuid::new_v4().to_string(),
        name,
        path: path.to_path_buf(),
        category,
        favorite: false,
        git_branch,
        tags,
        default_command,
        preferred_editor: None,
        service_drafts,
        last_opened_at: None,
    })
}

pub fn open_in_editor(path: &Path, editor: EditorKind) -> Result<()> {
    let path_str = path.to_string_lossy().to_string();

    #[cfg(target_os = "macos")]
    {
        match editor {
            EditorKind::Zed => {
                if Command::new("zed").arg(&path_str).spawn().is_err() {
                    Command::new("open")
                        .args(["-a", "Zed", &path_str])
                        .spawn()
                        .context("Failed to launch Zed")?;
                }
            }
            EditorKind::VSCode => {
                if Command::new("code").arg(&path_str).spawn().is_err() {
                    Command::new("open")
                        .args(["-a", "Visual Studio Code", &path_str])
                        .spawn()
                        .context("Failed to launch VS Code")?;
                }
            }
            EditorKind::Cursor => {
                if Command::new("cursor").arg(&path_str).spawn().is_err() {
                    Command::new("open")
                        .args(["-a", "Cursor", &path_str])
                        .spawn()
                        .context("Failed to launch Cursor")?;
                }
            }
            EditorKind::GoLand => {
                Command::new("open")
                    .args(["-a", "GoLand", &path_str])
                    .spawn()
                    .context("Failed to launch GoLand")?;
            }
            EditorKind::RustRover => {
                Command::new("open")
                    .args(["-a", "RustRover", &path_str])
                    .spawn()
                    .context("Failed to launch RustRover")?;
            }
            EditorKind::IntelliJ => {
                Command::new("open")
                    .args(["-a", "IntelliJ IDEA", &path_str])
                    .spawn()
                    .context("Failed to launch IntelliJ IDEA")?;
            }
            EditorKind::Terminal => {
                Command::new("open")
                    .args(["-a", "Terminal", &path_str])
                    .spawn()
                    .context("Failed to launch Terminal")?;
            }
            EditorKind::Finder => {
                Command::new("open")
                    .arg(&path_str)
                    .spawn()
                    .context("Failed to open Finder")?;
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let bin = match editor {
            EditorKind::Zed => "zed",
            EditorKind::VSCode => "code",
            EditorKind::Cursor => "cursor",
            EditorKind::GoLand => "goland",
            EditorKind::RustRover => "rustrover",
            EditorKind::IntelliJ => "idea",
            EditorKind::Terminal => "x-terminal-emulator",
            EditorKind::Finder => "xdg-open",
        };
        Command::new(bin)
            .arg(&path_str)
            .spawn()
            .with_context(|| format!("Failed to launch {bin}"))?;
    }

    Ok(())
}

pub fn open_url_in_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .with_context(|| format!("Failed to open browser for {url}"))?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .with_context(|| format!("Failed to open browser for {url}"))?;
    }
    Ok(())
}
