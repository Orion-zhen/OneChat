use serde::{Deserialize, Serialize};

use crate::domain::{AppSettings, Model, Provider};

use super::codec::{read_jsonc, write_json};
use super::{Result, Storage, StorageError, conflict, missing};

#[derive(Clone, Debug, PartialEq)]
pub struct ModelCatalog {
    pub providers: Vec<Provider>,
    pub models: Vec<Model>,
}

impl ModelCatalog {
    pub(super) fn new(providers: Vec<Provider>, mut models: Vec<Model>) -> Self {
        models.sort_by(|a, b| {
            a.display_name
                .to_lowercase()
                .cmp(&b.display_name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        Self { providers, models }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct SettingsFile {
    #[serde(flatten)]
    pub(super) app: AppSettings,
    pub(super) providers: Vec<Provider>,
    pub(super) models: Vec<Model>,
}

impl Storage {
    pub fn save_settings(&self, app: &AppSettings) -> Result<()> {
        let _guard = self.lock()?;
        let mut settings = self.read_settings()?;
        settings.app = app.clone();
        settings.app.normalize();
        settings.app.retain_models(&settings.models);
        self.write_settings(&settings)
    }

    pub fn insert_provider(&self, provider: &Provider) -> Result<ModelCatalog> {
        self.edit_settings(|settings| {
            if settings.providers.iter().any(|item| item.id == provider.id) {
                return Err(conflict("provider", &provider.id));
            }
            settings.providers.push(provider.clone());
            Ok(())
        })
    }

    pub fn update_provider(&self, provider: &Provider) -> Result<ModelCatalog> {
        self.edit_settings(|settings| {
            let stored = settings
                .providers
                .iter_mut()
                .find(|item| item.id == provider.id)
                .ok_or_else(|| missing("provider", &provider.id))?;
            *stored = provider.clone();
            Ok(())
        })
    }

    pub fn reorder_providers(&self, ordered_ids: &[String]) -> Result<ModelCatalog> {
        self.edit_settings(|settings| {
            if ordered_ids.len() != settings.providers.len() {
                return Err(StorageError::InvalidData(
                    "provider order does not match the configured providers".into(),
                ));
            }

            let mut providers = Vec::with_capacity(ordered_ids.len());
            for id in ordered_ids {
                let index = settings
                    .providers
                    .iter()
                    .position(|provider| &provider.id == id)
                    .ok_or_else(|| missing("provider", id))?;
                providers.push(settings.providers.remove(index));
            }
            settings.providers = providers;
            Ok(())
        })
    }

    pub fn delete_provider(&self, id: &str) -> Result<ModelCatalog> {
        let mut state = self.lock()?;
        let sessions = self.sessions(&mut state)?;
        let mut settings = self.read_settings()?;
        let previous_len = settings.providers.len();
        settings.providers.retain(|provider| provider.id != id);
        if settings.providers.len() == previous_len {
            return Err(missing("provider", id));
        }

        let removed_models = settings
            .models
            .iter()
            .filter(|model| model.provider_id == id)
            .map(|model| model.id.clone())
            .collect::<Vec<_>>();
        settings.models.retain(|model| model.provider_id != id);
        settings.app.retain_models(&settings.models);
        self.clear_conversation_models(sessions, &removed_models)?;
        self.write_settings(&settings)?;
        Ok(ModelCatalog::new(settings.providers, settings.models))
    }

    pub fn insert_model(&self, model: &Model) -> Result<ModelCatalog> {
        self.edit_settings(|settings| {
            validate_model(settings, model, None)?;
            settings.models.push(model.clone());
            Ok(())
        })
    }

    pub fn update_model(&self, model: &Model) -> Result<ModelCatalog> {
        self.edit_settings(|settings| {
            if !settings.models.iter().any(|item| item.id == model.id) {
                return Err(missing("model", &model.id));
            }
            validate_model(settings, model, Some(&model.id))?;
            let stored = settings
                .models
                .iter_mut()
                .find(|item| item.id == model.id)
                .expect("model existence was checked");
            *stored = model.clone();
            Ok(())
        })
    }

    pub fn delete_model(&self, id: &str) -> Result<ModelCatalog> {
        let mut state = self.lock()?;
        let sessions = self.sessions(&mut state)?;
        let mut settings = self.read_settings()?;
        let previous_len = settings.models.len();
        settings.models.retain(|model| model.id != id);
        if settings.models.len() == previous_len {
            return Err(missing("model", id));
        }
        settings.app.retain_models(&settings.models);
        self.clear_conversation_models(sessions, &[id.to_string()])?;
        self.write_settings(&settings)?;
        Ok(ModelCatalog::new(settings.providers, settings.models))
    }

    fn edit_settings(
        &self,
        edit: impl FnOnce(&mut SettingsFile) -> Result<()>,
    ) -> Result<ModelCatalog> {
        let _guard = self.lock()?;
        let mut settings = self.read_settings()?;
        edit(&mut settings)?;
        self.write_settings(&settings)?;
        Ok(ModelCatalog::new(settings.providers, settings.models))
    }

    pub(super) fn read_settings(&self) -> Result<SettingsFile> {
        read_jsonc(&self.settings_path)
    }

    pub(super) fn write_settings(&self, settings: &SettingsFile) -> Result<()> {
        write_json(&self.settings_path, settings)
    }
}

fn validate_model(settings: &SettingsFile, model: &Model, current_id: Option<&str>) -> Result<()> {
    if !settings
        .providers
        .iter()
        .any(|provider| provider.id == model.provider_id)
    {
        return Err(missing("provider", &model.provider_id));
    }
    if settings
        .models
        .iter()
        .any(|stored| stored.id == model.id && Some(stored.id.as_str()) != current_id)
    {
        return Err(conflict("model", &model.id));
    }
    if let Some(reasoning) = &model.reasoning {
        reasoning.validate().map_err(StorageError::InvalidData)?;
    }
    if settings.models.iter().any(|stored| {
        Some(stored.id.as_str()) != current_id
            && stored.provider_id == model.provider_id
            && stored.remote_id == model.remote_id
    }) {
        return Err(StorageError::InvalidData(format!(
            "model {} already exists for provider {}",
            model.remote_id, model.provider_id
        )));
    }
    Ok(())
}
