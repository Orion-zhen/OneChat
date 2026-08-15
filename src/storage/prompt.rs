use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::domain::PromptPreset;

use super::codec::write_text;
use super::{Result, Storage, StorageError, conflict, missing};

impl Storage {
    pub fn load_prompt_preset(&self, name: &str) -> Result<Option<PromptPreset>> {
        let _guard = self.lock()?;
        let directory = self.prompt_directory(name)?;
        if !directory.is_dir() {
            return Ok(None);
        }
        Ok(Some(read_prompt(&directory, name)?))
    }

    pub fn insert_prompt_preset(&self, preset: &PromptPreset) -> Result<()> {
        let _guard = self.lock()?;
        let preset = validated_preset(preset)?;
        let directory = self.prompt_directory(&preset.name)?;
        if directory.exists() {
            return Err(conflict("prompt preset", &preset.name));
        }
        write_prompt(&directory, &preset)
    }

    pub fn update_prompt_preset(&self, original_name: &str, preset: &PromptPreset) -> Result<()> {
        let _guard = self.lock()?;
        let preset = validated_preset(preset)?;
        let original_directory = self.prompt_directory(original_name)?;
        if !original_directory.is_dir() {
            return Err(missing("prompt preset", original_name));
        }
        let directory = self.prompt_directory(&preset.name)?;
        if directory != original_directory && directory.exists() {
            return Err(conflict("prompt preset", &preset.name));
        }
        write_prompt(&directory, &preset)?;
        if directory != original_directory {
            fs::remove_dir_all(original_directory)?;
        }
        Ok(())
    }

    pub fn delete_prompt_preset(&self, name: &str) -> Result<()> {
        let _guard = self.lock()?;
        let directory = self.prompt_directory(name)?;
        match fs::remove_dir_all(directory) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(missing("prompt preset", name))
            }
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn read_prompt_presets(&self) -> Result<Vec<PromptPreset>> {
        let mut directories = fs::read_dir(&self.prompts_dir)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        directories.retain(|path| {
            path.is_dir()
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with('.'))
        });

        let mut presets = Vec::with_capacity(directories.len());
        for directory in directories {
            let Some(name) = directory
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            presets.push(read_prompt(&directory, &name)?);
        }
        presets.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(presets)
    }

    fn prompt_directory(&self, name: &str) -> Result<PathBuf> {
        Ok(self.prompts_dir.join(validate_name(name)?))
    }
}

fn validated_preset(preset: &PromptPreset) -> Result<PromptPreset> {
    let name = validate_name(&preset.name)?;
    Ok(PromptPreset::new(
        name,
        &preset.system_prompt,
        &preset.assistant_opening,
    ))
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

fn read_prompt(directory: &Path, name: &str) -> Result<PromptPreset> {
    let system_prompt = fs::read_to_string(directory.join(format!("{name}.md")))?;
    let assistant_opening = match fs::read_to_string(directory.join(format!("{name}.opening.md"))) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    Ok(PromptPreset::new(name, system_prompt, assistant_opening))
}

fn write_prompt(directory: &Path, preset: &PromptPreset) -> Result<()> {
    fs::create_dir_all(directory)?;
    write_markdown(
        &directory.join(format!("{}.md", preset.name)),
        &preset.system_prompt,
    )?;
    let opening_path = directory.join(format!("{}.opening.md", preset.name));
    if preset.assistant_opening.is_empty() {
        match fs::remove_file(opening_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    } else {
        write_markdown(&opening_path, &preset.assistant_opening)?;
    }
    Ok(())
}

fn write_markdown(path: &Path, content: &str) -> Result<()> {
    let content = content.trim();
    if content.is_empty() {
        write_text(path, "")
    } else {
        write_text(path, &format!("{content}\n"))
    }
}
