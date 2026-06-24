use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub const MAX_RECENT_PROJECTS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecentProject {
    pub path: PathBuf,
    pub last_opened: String,
}

#[derive(Debug, Error)]
pub enum RecentProjectsError {
    #[error("app data directory unavailable: {0}")]
    AppData(#[from] crate::app_data::AppDataError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub fn recent_projects_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("recent_projects.json")
}

pub fn normalize_for_dedup(path: &Path) -> PathBuf {
    let mut normalized = path.to_path_buf();
    if let Ok(canon) = std::fs::canonicalize(path) {
        normalized = canon;
    }
    if cfg!(target_os = "windows") {
        PathBuf::from(normalized.to_string_lossy().to_lowercase())
    } else {
        normalized
    }
}

pub fn filter_existing(projects: Vec<RecentProject>) -> Vec<RecentProject> {
    projects
        .into_iter()
        .filter(|p| p.path.exists() && p.path.is_dir())
        .collect()
}

pub fn load_raw() -> Result<Vec<RecentProject>, RecentProjectsError> {
    let app_data_dir = crate::app_data::get_app_data_dir()?;
    let path = recent_projects_path(&app_data_dir);

    if !path.exists() {
        return Ok(vec![]);
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Ok(vec![]),
    };

    match serde_json::from_str(&content) {
        Ok(v) => Ok(v),
        Err(_) => Ok(vec![]),
    }
}

pub fn get_recent_projects() -> Result<Vec<RecentProject>, RecentProjectsError> {
    let raw = load_raw()?;
    Ok(filter_existing(raw))
}

pub fn add_recent_project(path: &Path) -> Result<(), RecentProjectsError> {
    let mut projects = load_raw()?;
    let normalized = normalize_for_dedup(path);

    projects.retain(|p| normalize_for_dedup(&p.path) != normalized);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let entry = RecentProject {
        path: path.to_path_buf(),
        last_opened: format!("{}", now),
    };

    projects.insert(0, entry);
    projects.truncate(MAX_RECENT_PROJECTS);

    let app_data_dir = crate::app_data::get_app_data_dir()?;
    let path_out = recent_projects_path(&app_data_dir);

    let content = serde_json::to_string(&projects)?;
    std::fs::write(&path_out, content)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_recent_projects_crud() {
        let p = std::env::temp_dir();
        add_recent_project(&p).unwrap();

        let loaded = load_raw().unwrap();
        assert!(!loaded.is_empty());
        assert_eq!(loaded[0].path, p);

        add_recent_project(&p).unwrap();
        let loaded = load_raw().unwrap();
        assert_eq!(
            loaded
                .iter()
                .filter(|x| normalize_for_dedup(&x.path) == normalize_for_dedup(&p))
                .count(),
            1
        );
    }

    proptest! {
        #[test]
        fn test_recent_projects_invariants(paths in proptest::collection::vec("[a-zA-Z0-9]{1,5}", 0..20)) {
            // we execute the addition and check invariants
            for p_str in &paths {
                let p = PathBuf::from(p_str);
                let _ = add_recent_project(&p);
            }
            let loaded = load_raw().unwrap();
            assert!(loaded.len() <= MAX_RECENT_PROJECTS);
        }
    }
}
