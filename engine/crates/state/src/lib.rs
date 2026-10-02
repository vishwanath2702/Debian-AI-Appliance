//! Desired, current, and persisted system state.

const APPLIANCE_STATE_PATH: &str = "var/lib/daia/state.json";

/// Returns the persisted appliance state path beneath an appliance root.
#[must_use]
pub fn appliance_state_path(root: impl AsRef<std::path::Path>) -> std::path::PathBuf {
    root.as_ref().join(APPLIANCE_STATE_PATH)
}

#[derive(serde::Deserialize, serde::Serialize)]
struct PersistedImportedContentItem {
    source_item_id: String,
    path: std::path::PathBuf,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct PersistedApplianceState {
    profile_name: String,
    imported_content: Vec<PersistedImportedContentItem>,
    #[serde(default)]
    model_realizations: Vec<PersistedModelRealization>,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct PersistedModelRealization {
    id: String,
    engine: String,
    content: PersistedImportedContentItem,
}

/// Persisted state describing a configured DAIA appliance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplianceState {
    profile_name: String,
    imported_content: Vec<model::ImportedContentItem>,
    model_realizations: Vec<model::ModelRealization>,
}

impl ApplianceState {
    /// Creates appliance state for the selected appliance profile.
    #[must_use]
    pub fn new(profile_name: impl Into<String>) -> Self {
        Self {
            profile_name: profile_name.into(),
            imported_content: Vec::new(),
            model_realizations: Vec::new(),
        }
    }

    /// Returns the configured appliance profile name.
    #[must_use]
    pub fn profile_name(&self) -> &str {
        &self.profile_name
    }

    /// Records content realized on the configured appliance.
    pub fn record_imported_content(&mut self, item: model::ImportedContentItem) {
        if let Some(existing) = self
            .imported_content
            .iter_mut()
            .find(|existing| existing.source_item_id() == item.source_item_id())
        {
            *existing = item;
        } else {
            self.imported_content.push(item);
        }
    }

    /// Records multiple content items realized on the configured appliance.
    pub fn record_imported_contents(
        &mut self,
        items: impl IntoIterator<Item = model::ImportedContentItem>,
    ) {
        for item in items {
            self.record_imported_content(item);
        }
    }

    /// Returns content realized on the configured appliance.
    #[must_use]
    pub fn imported_content(&self) -> &[model::ImportedContentItem] {
        &self.imported_content
    }

    /// Applies successfully imported content and binds desired model realizations.
    pub fn apply_content_imports(
        &mut self,
        imported_content: impl IntoIterator<Item = model::ImportedContentItem>,
        model_realization_intents: &[model::ModelRealizationIntent],
    ) {
        self.record_imported_contents(imported_content);

        for intent in model_realization_intents {
            if let Some(imported_content) = self
                .imported_content
                .iter()
                .find(|item| item.source_item_id() == intent.source_item_id())
                .cloned()
            {
                self.record_model_realization(model::ModelRealization::new(
                    intent.id().clone(),
                    intent.engine().clone(),
                    imported_content,
                ));
            }
        }
    }

    /// Records a model realization intended for this appliance.
    pub fn record_model_realization(&mut self, realization: model::ModelRealization) {
        if let Some(existing) = self
            .model_realizations
            .iter_mut()
            .find(|existing| existing.id() == realization.id())
        {
            *existing = realization;
        } else {
            self.model_realizations.push(realization);
        }
    }

    /// Returns model realizations intended for this appliance.
    #[must_use]
    pub fn model_realizations(&self) -> &[model::ModelRealization] {
        &self.model_realizations
    }

    /// Serializes the persisted appliance state as JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let persisted = PersistedApplianceState {
            profile_name: self.profile_name.clone(),
            imported_content: self
                .imported_content
                .iter()
                .map(|item| PersistedImportedContentItem {
                    source_item_id: item.source_item_id().as_str().to_owned(),
                    path: item.path().to_path_buf(),
                })
                .collect(),
            model_realizations: self
                .model_realizations
                .iter()
                .map(|realization| PersistedModelRealization {
                    id: realization.id().as_str().to_owned(),
                    engine: realization.engine().as_str().to_owned(),
                    content: PersistedImportedContentItem {
                        source_item_id: realization.content().source_item_id().as_str().to_owned(),
                        path: realization.content().path().to_path_buf(),
                    },
                })
                .collect(),
        };

        serde_json::to_string_pretty(&persisted)
    }

    /// Reads persisted appliance state from JSON.
    pub fn read_json(path: impl AsRef<std::path::Path>) -> Result<Self, std::io::Error> {
        let json = std::fs::read_to_string(path)?;

        let persisted: PersistedApplianceState = serde_json::from_str(&json)
            .map_err(|error| std::io::Error::other(error.to_string()))?;

        let mut state = Self::new(persisted.profile_name);

        state.record_imported_contents(persisted.imported_content.into_iter().map(|item| {
            model::ImportedContentItem::new(
                model::ExternalContentItemId::new(item.source_item_id),
                item.path,
            )
        }));

        for realization in persisted.model_realizations {
            state.record_model_realization(model::ModelRealization::new(
                model::ModelRealizationId::new(realization.id),
                model::InferenceEngineId::new(realization.engine),
                model::ImportedContentItem::new(
                    model::ExternalContentItemId::new(realization.content.source_item_id),
                    realization.content.path,
                ),
            ));
        }

        Ok(state)
    }

    /// Reads persisted appliance state beneath the supplied appliance root.
    pub fn read_from_root(root: impl AsRef<std::path::Path>) -> Result<Self, std::io::Error> {
        Self::read_json(appliance_state_path(root))
    }

    /// Writes the persisted appliance state as JSON to the supplied path.
    pub fn write_json(&self, path: impl AsRef<std::path::Path>) -> Result<(), std::io::Error> {
        let json = self
            .to_json()
            .map_err(|error| std::io::Error::other(error.to_string()))?;

        std::fs::write(path, json)
    }

    /// Writes the persisted appliance state beneath the supplied appliance root.
    pub fn write_to_root(&self, root: impl AsRef<std::path::Path>) -> Result<(), std::io::Error> {
        let path = appliance_state_path(root);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        self.write_json(path)
    }
}

/// Holds the accepted Current State for one managed resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentState<T> {
    current: model::CurrentResource<T>,
}

impl<T> CurrentState<T> {
    /// Creates Current State from an already accepted resource.
    #[must_use]
    pub const fn new(current: model::CurrentResource<T>) -> Self {
        Self { current }
    }

    /// Returns the accepted current resource.
    #[must_use]
    pub const fn current(&self) -> &model::CurrentResource<T> {
        &self.current
    }

    /// Replaces Current State with another already accepted resource.
    pub fn replace(&mut self, current: model::CurrentResource<T>) {
        self.current = current;
    }
}

#[cfg(test)]
mod tests {
    use super::{ApplianceState, CurrentState};

    #[test]
    fn stores_accepted_current_resource() {
        let resource = model::CurrentResource::new(
            model::ResourceId::new("service/ollama"),
            model::ResourceType::new("service"),
            model::SchemaVersion::new(1),
            model::CurrentRevision::new(2),
            model::ServiceCurrentState::new(true, true, true),
        );

        let state = CurrentState::new(resource.clone());

        assert_eq!(state.current(), &resource);
    }

    #[test]
    fn replaces_current_state_with_accepted_resource() {
        let initial = model::CurrentResource::new(
            model::ResourceId::new("service/ollama"),
            model::ResourceType::new("service"),
            model::SchemaVersion::new(1),
            model::CurrentRevision::new(2),
            model::ServiceCurrentState::new(true, false, false),
        );
        let replacement = model::CurrentResource::new(
            model::ResourceId::new("service/ollama"),
            model::ResourceType::new("service"),
            model::SchemaVersion::new(1),
            model::CurrentRevision::new(7),
            model::ServiceCurrentState::new(true, true, true),
        );
        let mut state = CurrentState::new(initial);

        state.replace(replacement.clone());

        assert_eq!(state.current(), &replacement);
    }

    #[test]
    fn appliance_state_path_is_beneath_appliance_root() {
        assert_eq!(
            super::appliance_state_path("/target"),
            std::path::PathBuf::from("/target/var/lib/daia/state.json")
        );
    }

    #[test]
    fn serializes_appliance_state_as_json() {
        let mut state = ApplianceState::new("ai-workstation");
        state.record_imported_content(model::ImportedContentItem::new(
            model::ExternalContentItemId::new("model"),
            "/var/lib/daia/content/model.gguf",
        ));

        let json = state.to_json().expect("appliance state should serialize");

        assert_eq!(
            json,
            concat!(
                "{\n",
                "  \"profile_name\": \"ai-workstation\",\n",
                "  \"imported_content\": [\n",
                "    {\n",
                "      \"source_item_id\": \"model\",\n",
                "      \"path\": \"/var/lib/daia/content/model.gguf\"\n",
                "    }\n",
                "  ],\n",
                "  \"model_realizations\": []\n",
                "}"
            )
        );
    }

    #[test]
    fn round_trips_model_realization() {
        let content = model::ImportedContentItem::new(
            model::ExternalContentItemId::new("source-model"),
            "/var/lib/daia/content/model.gguf",
        );

        let mut expected = ApplianceState::new("ai-workstation");
        expected.record_imported_content(content.clone());
        expected.record_model_realization(model::ModelRealization::new(
            model::ModelRealizationId::new("model"),
            model::InferenceEngineId::ollama(),
            content,
        ));

        let path = std::env::temp_dir().join(format!(
            "daia-model-realization-state-{}-{}.json",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));

        expected
            .write_json(&path)
            .expect("appliance state should be written");

        let actual = ApplianceState::read_json(&path).expect("appliance state should be read");

        std::fs::remove_file(&path).expect("temporary appliance state should be removed");

        assert_eq!(actual, expected);
        assert_eq!(actual.model_realizations().len(), 1);
        assert_eq!(actual.model_realizations()[0].id().as_str(), "model");
        assert_eq!(
            actual.model_realizations()[0].engine(),
            &model::InferenceEngineId::ollama()
        );
    }

    #[test]
    fn reads_appliance_state_beneath_root() {
        let mut expected = ApplianceState::new("ai-workstation");
        expected.record_imported_content(model::ImportedContentItem::new(
            model::ExternalContentItemId::new("model"),
            "/var/lib/daia/content/model.gguf",
        ));

        let root = std::env::temp_dir().join(format!(
            "daia-appliance-state-read-root-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));

        expected
            .write_to_root(&root)
            .expect("appliance state should be written beneath root");

        let actual = ApplianceState::read_from_root(&root)
            .expect("appliance state should be read beneath root");

        std::fs::remove_dir_all(&root).expect("temporary appliance root should be removed");

        assert_eq!(actual, expected);
    }

    #[test]
    fn writes_appliance_state_beneath_root() {
        let state = ApplianceState::new("ai-workstation");
        let root = std::env::temp_dir().join(format!(
            "daia-appliance-state-root-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));

        state
            .write_to_root(&root)
            .expect("appliance state should be written beneath root");

        let path = super::appliance_state_path(&root);
        let json = std::fs::read_to_string(&path).expect("appliance state should be readable");

        std::fs::remove_dir_all(&root).expect("temporary appliance root should be removed");

        assert_eq!(
            json,
            concat!(
                "{\n",
                "  \"profile_name\": \"ai-workstation\",\n",
                "  \"imported_content\": [],\n",
                "  \"model_realizations\": []\n",
                "}"
            )
        );
    }

    #[test]
    fn writes_appliance_state_as_json() {
        let state = ApplianceState::new("ai-workstation");
        let path = std::env::temp_dir().join(format!(
            "daia-appliance-state-{}-{}.json",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));

        state
            .write_json(&path)
            .expect("appliance state should be written");

        let json = std::fs::read_to_string(&path).expect("appliance state should be readable");
        std::fs::remove_file(&path).expect("temporary appliance state should be removed");

        assert_eq!(
            json,
            concat!(
                "{\n",
                "  \"profile_name\": \"ai-workstation\",\n",
                "  \"imported_content\": [],\n",
                "  \"model_realizations\": []\n",
                "}"
            )
        );
    }

    #[test]
    fn stores_appliance_profile_name() {
        let state = ApplianceState::new("ai-workstation");

        assert_eq!(state.profile_name(), "ai-workstation");
    }

    #[test]
    fn starts_without_imported_content() {
        let state = ApplianceState::new("ai-workstation");

        assert!(state.imported_content().is_empty());
    }

    #[test]
    fn records_multiple_imported_content_items() {
        let mut state = ApplianceState::new("ai-workstation");
        let first = model::ImportedContentItem::new(
            model::ExternalContentItemId::new("first"),
            "/var/lib/daia/content/first.gguf",
        );
        let second = model::ImportedContentItem::new(
            model::ExternalContentItemId::new("second"),
            "/var/lib/daia/content/second.gguf",
        );

        state.record_imported_contents(vec![first.clone(), second.clone()]);

        assert_eq!(state.imported_content(), &[first, second]);
    }

    #[test]
    fn records_imported_content() {
        let mut state = ApplianceState::new("ai-workstation");
        let item = model::ImportedContentItem::new(
            model::ExternalContentItemId::new("model"),
            "/var/lib/daia/content/model.gguf",
        );

        state.record_imported_content(item.clone());

        assert_eq!(state.imported_content(), &[item]);
    }

    #[test]
    fn replaces_imported_content_with_same_source_identity() {
        let mut state = ApplianceState::new("ai-workstation");

        state.record_imported_content(model::ImportedContentItem::new(
            model::ExternalContentItemId::new("model"),
            "/var/lib/daia/content/old.gguf",
        ));

        let replacement = model::ImportedContentItem::new(
            model::ExternalContentItemId::new("model"),
            "/var/lib/daia/content/new.gguf",
        );

        state.record_imported_content(replacement.clone());

        assert_eq!(state.imported_content(), &[replacement]);
    }

    #[test]
    fn applies_imported_content_and_model_realization_intents() {
        let mut state = ApplianceState::new("ai-workstation");

        let existing = model::ImportedContentItem::new(
            model::ExternalContentItemId::new("existing-source"),
            "/var/lib/daia/content/existing.gguf",
        );
        state.record_imported_content(existing.clone());

        let imported = model::ImportedContentItem::new(
            model::ExternalContentItemId::new("new-source"),
            "/var/lib/daia/content/new.gguf",
        );

        let intents = vec![model::ModelRealizationIntent::new(
            model::ModelRealizationId::new("new-model"),
            model::InferenceEngineId::ollama(),
            model::ExternalContentItemId::new("new-source"),
        )];

        state.apply_content_imports(vec![imported.clone()], &intents);

        assert_eq!(state.imported_content(), &[existing, imported.clone()]);
        assert_eq!(state.model_realizations().len(), 1);
        assert_eq!(
            state.model_realizations()[0],
            model::ModelRealization::new(
                model::ModelRealizationId::new("new-model"),
                model::InferenceEngineId::ollama(),
                imported,
            )
        );
    }

    #[test]
    fn replaces_model_realization_with_same_realization_identity() {
        let mut state = ApplianceState::new("ai-workstation");

        let old_content = model::ImportedContentItem::new(
            model::ExternalContentItemId::new("old-source"),
            "/var/lib/daia/content/old.gguf",
        );
        let new_content = model::ImportedContentItem::new(
            model::ExternalContentItemId::new("new-source"),
            "/var/lib/daia/content/new.gguf",
        );

        state.record_model_realization(model::ModelRealization::new(
            model::ModelRealizationId::new("assistant"),
            model::InferenceEngineId::ollama(),
            old_content,
        ));

        let replacement = model::ModelRealization::new(
            model::ModelRealizationId::new("assistant"),
            model::InferenceEngineId::ollama(),
            new_content,
        );

        state.record_model_realization(replacement.clone());

        assert_eq!(state.model_realizations(), &[replacement]);
    }
}
