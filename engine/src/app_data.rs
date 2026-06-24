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
    let base_dir = if cfg!(target_os = "windows") || cfg!(target_os = "macos") {
        dirs::data_dir()
    } else {
        dirs::config_dir()
    };

    let Some(mut path) = base_dir else {
        return Err(AppDataError::PlatformDirectoryNotFound);
    };

    if cfg!(target_os = "linux") {
        path.push("gwenland-ide");
    } else {
        path.push("GwenLandIDE");
    }

    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_app_data_dir() {
        let path = get_app_data_dir().expect("get_app_data_dir should succeed");
        assert!(!path.as_os_str().is_empty());
        assert!(path.exists());

        let path2 = get_app_data_dir().expect("second call should succeed");
        assert_eq!(path, path2, "get_app_data_dir should be idempotent");
    }
}
