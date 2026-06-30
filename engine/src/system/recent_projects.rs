use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};
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
    #[error("system clock is before the Unix epoch: {0}")]
    Clock(#[from] SystemTimeError),
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
    load_raw_from(&app_data_dir)
}

fn load_raw_from(app_data_dir: &Path) -> Result<Vec<RecentProject>, RecentProjectsError> {
    let path = recent_projects_path(app_data_dir);

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(vec![]),
        Err(err) => return Err(err.into()),
    };

    if content.trim().is_empty() {
        return Ok(vec![]);
    }

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
    let app_data_dir = crate::app_data::get_app_data_dir()?;
    add_recent_project_to(path, &app_data_dir)
}

fn add_recent_project_to(
    project_path: &Path,
    app_data_dir: &Path,
) -> Result<(), RecentProjectsError> {
    let mut projects = load_raw_from(app_data_dir)?;
    let normalized = normalize_for_dedup(project_path);

    projects.retain(|p| normalize_for_dedup(&p.path) != normalized);

    let entry = RecentProject {
        path: project_path.to_path_buf(),
        last_opened: unix_timestamp_secs()?.to_string(),
    };

    projects.insert(0, entry);
    projects.truncate(MAX_RECENT_PROJECTS);

    let path_out = recent_projects_path(app_data_dir);

    std::fs::create_dir_all(app_data_dir)?;
    let content = serde_json::to_string(&projects)?;
    std::fs::write(&path_out, content)?;

    Ok(())
}

fn unix_timestamp_secs() -> Result<u64, SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tempfile::{TempDir, tempdir};

    fn isolated_app_data_dir() -> TempDir {
        tempdir().expect("temp app-data dir")
    }

    fn create_project(root: &TempDir, name: &str) -> PathBuf {
        let path = root.path().join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn add_recent_project_persists_newest_first_and_deduplicates() {
        let app_data_dir = isolated_app_data_dir();
        let workspace = tempdir().unwrap();
        let first = create_project(&workspace, "first");
        let second = create_project(&workspace, "second");

        add_recent_project_to(&first, app_data_dir.path()).unwrap();
        add_recent_project_to(&second, app_data_dir.path()).unwrap();
        add_recent_project_to(&first, app_data_dir.path()).unwrap();

        let loaded = load_raw_from(app_data_dir.path()).unwrap();

        assert_eq!(
            loaded.iter().map(|p| &p.path).collect::<Vec<_>>(),
            vec![&first, &second]
        );
        let timestamps = loaded
            .iter()
            .map(|project| {
                project
                    .last_opened
                    .parse::<u64>()
                    .expect("recent project timestamp is Unix seconds")
            })
            .collect::<Vec<_>>();
        assert_eq!(timestamps.len(), 2);
        assert!(timestamps.iter().all(|timestamp| *timestamp > 0));
    }

    #[test]
    fn add_recent_project_trims_to_max_length() {
        let app_data_dir = isolated_app_data_dir();
        let workspace = tempdir().unwrap();
        let projects = (0..(MAX_RECENT_PROJECTS + 3))
            .map(|index| create_project(&workspace, &format!("project-{index}")))
            .collect::<Vec<_>>();

        for project in &projects {
            add_recent_project_to(project, app_data_dir.path()).unwrap();
        }

        let loaded = load_raw_from(app_data_dir.path()).unwrap();
        let expected = projects
            .iter()
            .rev()
            .take(MAX_RECENT_PROJECTS)
            .collect::<Vec<_>>();

        assert_eq!(loaded.len(), MAX_RECENT_PROJECTS);
        assert_eq!(loaded.iter().map(|p| &p.path).collect::<Vec<_>>(), expected);
    }

    #[test]
    fn filter_existing_keeps_only_directories() {
        let workspace = tempdir().unwrap();
        let existing_dir = create_project(&workspace, "dir");
        let file_path = workspace.path().join("file.txt");
        let missing_dir = workspace.path().join("missing");
        std::fs::write(&file_path, "not a project dir").unwrap();

        let filtered = filter_existing(vec![
            RecentProject {
                path: missing_dir,
                last_opened: "1".to_string(),
            },
            RecentProject {
                path: file_path,
                last_opened: "2".to_string(),
            },
            RecentProject {
                path: existing_dir.clone(),
                last_opened: "3".to_string(),
            },
        ]);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].path, existing_dir);
        assert_eq!(filtered[0].last_opened, "3");
    }

    #[test]
    fn load_raw_handles_missing_empty_and_malformed_files() {
        let app_data_dir = isolated_app_data_dir();
        assert_eq!(load_raw_from(app_data_dir.path()).unwrap(), Vec::new());

        let path = recent_projects_path(app_data_dir.path());
        std::fs::write(&path, "").unwrap();
        assert_eq!(load_raw_from(app_data_dir.path()).unwrap(), Vec::new());

        std::fs::write(&path, "not-json").unwrap();
        assert_eq!(load_raw_from(app_data_dir.path()).unwrap(), Vec::new());
    }

    proptest! {
        #[test]
        fn test_recent_projects_invariants(paths in proptest::collection::vec("[a-zA-Z0-9]{1,5}", 0..20)) {
            let app_data_dir = isolated_app_data_dir();
            for p_str in &paths {
                let p = PathBuf::from(p_str);
                let _ = add_recent_project_to(&p, app_data_dir.path());
            }
            let loaded = load_raw_from(app_data_dir.path()).unwrap();
            let mut normalized = loaded
                .iter()
                .map(|project| normalize_for_dedup(&project.path))
                .collect::<Vec<_>>();
            normalized.sort();
            normalized.dedup();

            assert!(loaded.len() <= MAX_RECENT_PROJECTS);
            assert_eq!(loaded.len(), normalized.len());
        }
    }
}
