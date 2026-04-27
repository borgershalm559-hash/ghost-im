//! Cross-platform paths for Ghost data files.
//! Linux:   ~/.ghost/identity.encrypted
//! Windows: %APPDATA%/Ghost/identity.encrypted
//! macOS:   ~/Library/Application Support/Ghost/identity.encrypted
//!
//! Override via `GHOST_HOME` environment variable (used in tests and for portability).

use directories::ProjectDirs;
#[allow(unused_imports)]
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathsError {
    #[error("could not resolve user data directory (no home directory?)")]
    NoHome,
}

/// Root data directory for Ghost.
pub fn ghost_home() -> Result<PathBuf, PathsError> {
    if let Ok(custom) = std::env::var("GHOST_HOME") {
        return Ok(PathBuf::from(custom));
    }
    // On Linux directories crate uses XDG_DATA_HOME or ~/.local/share/ghost. We override
    // to "~/.ghost" to match the design spec.
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return Ok(Path::new(&home).join(".ghost"));
        }
        return Err(PathsError::NoHome);
    }
    #[cfg(not(target_os = "linux"))]
    {
        let pd = ProjectDirs::from("im", "Ghost", "Ghost").ok_or(PathsError::NoHome)?;
        Ok(pd.data_dir().to_path_buf())
    }
}

pub fn identity_file() -> Result<PathBuf, PathsError> {
    Ok(ghost_home()?.join("identity.encrypted"))
}

pub fn database_file() -> Result<PathBuf, PathsError> {
    Ok(ghost_home()?.join("ghost.db"))
}

pub fn logs_dir() -> Result<PathBuf, PathsError> {
    Ok(ghost_home()?.join("logs"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_home_respects_env_override() {
        let temp = std::env::temp_dir().join("ghost-home-test-1");
        std::env::set_var("GHOST_HOME", &temp);
        let home = ghost_home().unwrap();
        assert_eq!(home, temp);
        std::env::remove_var("GHOST_HOME");
    }

    #[test]
    fn paths_compose_correctly() {
        let temp = std::env::temp_dir().join("ghost-home-test-2");
        std::env::set_var("GHOST_HOME", &temp);
        assert_eq!(identity_file().unwrap(), temp.join("identity.encrypted"));
        assert_eq!(database_file().unwrap(), temp.join("ghost.db"));
        assert_eq!(logs_dir().unwrap(), temp.join("logs"));
        std::env::remove_var("GHOST_HOME");
    }
}
