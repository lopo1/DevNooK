use crate::models::AppStateData;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".devnook")
}

pub fn state_file_path() -> PathBuf {
    config_dir().join("state.json")
}

pub fn load_state() -> AppStateData {
    load_state_from(&state_file_path())
}

pub fn load_state_from(path: &Path) -> AppStateData {
    if !path.exists() {
        return AppStateData::default();
    }
    match fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => AppStateData::default(),
    }
}

pub fn save_state(state: &AppStateData) -> Result<()> {
    save_state_to(state, &state_file_path())
}

pub fn save_state_to(state: &AppStateData, path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        if !dir.exists() {
            fs::create_dir_all(dir).with_context(|| format!("Failed to create {:?}", dir))?;
        }
    }
    let tmp_path = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(state)?;
    fs::write(&tmp_path, json).with_context(|| format!("Failed to write {:?}", tmp_path))?;
    fs::rename(&tmp_path, path)
        .with_context(|| format!("Failed to atomically replace {:?}", path))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{EditorKind, Project, ProjectCategory};

    #[test]
    fn round_trip_atomic_persistence() {
        let dir = std::env::temp_dir().join(format!("devnook_test_{}", uuid::Uuid::new_v4()));
        let file = dir.join("state.json");

        let mut data = AppStateData::default();
        data.preferred_editor = EditorKind::Cursor;
        data.projects.push(Project {
            id: "p1".into(),
            name: "demo".into(),
            path: PathBuf::from("/tmp/demo"),
            category: ProjectCategory::Rust,
            favorite: true,
            git_branch: Some("main".into()),
            tags: vec!["cargo".into()],
            default_command: Some("cargo run".into()),
            preferred_editor: None,
            service_drafts: vec![],
            last_opened_at: None,
        });

        save_state_to(&data, &file).expect("save_state_to should succeed");
        let loaded = load_state_from(&file);
        assert_eq!(loaded.preferred_editor, EditorKind::Cursor);
        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].name, "demo");

        let _ = fs::remove_dir_all(dir);
    }
}
