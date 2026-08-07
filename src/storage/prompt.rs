use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::domain::SystemPromptPreset;

use super::codec::write_text;
use super::{Result, Storage, StorageError, conflict, missing};

impl Storage {
    pub fn load_prompt_preset(&self, name: &str) -> Result<Option<SystemPromptPreset>> {
        let _guard = self.lock()?;
        let path = self.prompt_path(name)?;
        match fs::read_to_string(path) {
            Ok(content) => Ok(Some(SystemPromptPreset::new(name, content))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn insert_prompt_preset(&self, preset: &SystemPromptPreset) -> Result<()> {
        let _guard = self.lock()?;
        let preset = validated_preset(preset)?;
        let path = self.prompt_path(&preset.name)?;
        if path.exists() {
            return Err(conflict("prompt preset", &preset.name));
        }
        write_prompt(&path, &preset.content)
    }

    pub fn update_prompt_preset(
        &self,
        original_name: &str,
        preset: &SystemPromptPreset,
    ) -> Result<()> {
        let _guard = self.lock()?;
        let preset = validated_preset(preset)?;
        let original_path = self.prompt_path(original_name)?;
        if !original_path.exists() {
            return Err(missing("prompt preset", original_name));
        }
        let path = self.prompt_path(&preset.name)?;
        if path != original_path && path.exists() {
            return Err(conflict("prompt preset", &preset.name));
        }
        write_prompt(&path, &preset.content)?;
        if path != original_path {
            fs::remove_file(original_path)?;
        }
        Ok(())
    }

    pub fn delete_prompt_preset(&self, name: &str) -> Result<()> {
        let _guard = self.lock()?;
        let path = self.prompt_path(name)?;
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(missing("prompt preset", name))
            }
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn read_prompt_presets(&self) -> Result<Vec<SystemPromptPreset>> {
        let mut paths = fs::read_dir(&self.prompts_dir)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        paths.retain(|path| {
            path.is_file()
                && path.extension().is_some_and(|extension| extension == "md")
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with('.'))
        });

        let mut presets = Vec::with_capacity(paths.len());
        for path in paths {
            let Some(name) = path
                .file_stem()
                .and_then(|name| name.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            presets.push(SystemPromptPreset::new(name, fs::read_to_string(path)?));
        }
        presets.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(presets)
    }

    fn prompt_path(&self, name: &str) -> Result<PathBuf> {
        let name = validate_name(name)?;
        Ok(self.prompts_dir.join(format!("{name}.md")))
    }
}

fn validated_preset(preset: &SystemPromptPreset) -> Result<SystemPromptPreset> {
    let name = validate_name(&preset.name)?;
    Ok(SystemPromptPreset::new(name, &preset.content))
}

fn validate_name(name: &str) -> Result<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed != name
        || name.starts_with('.')
        || Path::new(name).components().count() != 1
        || Path::new(name)
            .file_name()
            .is_none_or(|value| value != name)
    {
        return Err(StorageError::InvalidData(format!(
            "invalid prompt preset name: {name}"
        )));
    }
    Ok(name)
}

fn write_prompt(path: &Path, content: &str) -> Result<()> {
    let content = content.trim();
    if content.is_empty() {
        write_text(path, "")
    } else {
        write_text(path, &format!("{content}\n"))
    }
}
