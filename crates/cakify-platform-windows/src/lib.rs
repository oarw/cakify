//! Windows-specific application boundaries.
//!
//! M0 only establishes the local data layout contract. Credential Manager,
//! DPAPI, known-folder COM calls, and Job Objects land behind this crate in M1
//! and M4 so the UI/core crates never depend on raw Win32 handles.

use std::{
    env,
    path::{Path, PathBuf},
};

use thiserror::Error;

pub const APP_DIRECTORY_NAME: &str = "Cakify";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppDataPaths {
    pub root: PathBuf,
    pub data: PathBuf,
    pub attachments: PathBuf,
    pub logs: PathBuf,
    pub cache: PathBuf,
}

impl AppDataPaths {
    pub fn from_local_app_data(local_app_data: impl AsRef<Path>) -> Self {
        let root = local_app_data.as_ref().join(APP_DIRECTORY_NAME);
        Self {
            data: root.join("data"),
            attachments: root.join("attachments"),
            logs: root.join("logs"),
            cache: root.join("cache"),
            root,
        }
    }

    pub fn create_layout(&self) -> Result<(), PlatformError> {
        for path in [
            &self.root,
            &self.data,
            &self.attachments,
            &self.logs,
            &self.cache,
        ] {
            std::fs::create_dir_all(path).map_err(|source| PlatformError::CreateDirectory {
                path: path.clone(),
                source,
            })?;
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("LOCALAPPDATA is not available")]
    MissingLocalAppData,
    #[error("failed to create application directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub fn app_data_paths() -> Result<AppDataPaths, PlatformError> {
    let local_app_data = env::var_os("LOCALAPPDATA").ok_or(PlatformError::MissingLocalAppData)?;
    Ok(AppDataPaths::from_local_app_data(local_app_data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_stable_and_scoped_to_cakify() {
        let paths = AppDataPaths::from_local_app_data(r"C:\Users\test\AppData\Local");
        assert_eq!(
            paths.root,
            PathBuf::from(r"C:\Users\test\AppData\Local\Cakify")
        );
        assert_eq!(paths.data, paths.root.join("data"));
        assert_eq!(paths.attachments, paths.root.join("attachments"));
    }
}
