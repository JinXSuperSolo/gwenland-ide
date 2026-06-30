use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppDataError {
    #[error("could not determine home/app-data directory for this platform")]
    PlatformDirectoryNotFound,
    #[error("failed to create app data directory: {0}")]
    DirectoryCreationFailed(#[from] std::io::Error),
}

/// Returns the GwenLand IDE app data directory for the current platform,
/// creating it (and any parents) if it does not already exist.
pub fn get_app_data_dir() -> Result<PathBuf, AppDataError> {
    let base_dir = platform_base_data_dir().ok_or(AppDataError::PlatformDirectoryNotFound)?;
    app_data_dir_from_base(base_dir)
}

fn platform_base_data_dir() -> Option<PathBuf> {
    if cfg!(target_os = "windows") || cfg!(target_os = "macos") {
        dirs::data_dir()
    } else {
        dirs::config_dir()
    }
}

fn app_data_dir_from_base(mut path: PathBuf) -> Result<PathBuf, AppDataError> {
    path.push(app_data_dir_name());
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn app_data_dir_name() -> &'static str {
    if cfg!(target_os = "linux") {
        "gwenland-ide"
    } else {
        "GwenLandIDE"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn app_data_dir_from_base_creates_platform_child() {
        let base = tempdir().expect("temp dir");
        let expected = base.path().join(app_data_dir_name());

        let path = app_data_dir_from_base(base.path().to_path_buf()).expect("app data path");

        assert_eq!(path, expected);
        assert!(path.is_dir());
    }

    #[test]
    fn app_data_dir_from_base_is_idempotent() {
        let base = tempdir().expect("temp dir");
        let first = app_data_dir_from_base(base.path().to_path_buf()).expect("first path");
        let second = app_data_dir_from_base(base.path().to_path_buf()).expect("second path");

        assert_eq!(first, second);
        assert!(second.is_dir());
    }
}
