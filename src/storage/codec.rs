use std::{fs, path::Path};

use serde::{Serialize, de::DeserializeOwned};

use super::{Result, StorageError};

pub(super) fn read_jsonc<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let source = fs::read_to_string(path)?;
    json5::from_str(&source).map_err(|error| StorageError::Parse {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

pub(super) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut contents = serde_json::to_string_pretty(value)?;
    contents.push('\n');

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("onechat");
    let temporary = path.with_file_name(format!(
        ".{file_name}.{}.tmp",
        crate::domain::new_id("write")
    ));
    fs::write(&temporary, contents)?;

    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path)?;
    }

    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(())
}
