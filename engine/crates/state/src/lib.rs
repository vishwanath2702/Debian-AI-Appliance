//! Desired, current, and persisted system state.

const APPLIANCE_STATE_PATH: &str = "var/lib/daia/state.json";

/// Returns the persisted appliance state path beneath an appliance root.
#[must_use]
pub fn appliance_state_path(root: impl AsRef<std::path::Path>) -> std::path::PathBuf {
    root.as_ref().join(APPLIANCE_STATE_PATH)
}

#[derive(serde::Serialize)]
struct PersistedImportedContentItem<'a> {
    source_item_id: &'a str,
    path: &'a std::path::Path,
}

#[derive(serde::Serialize)]
struct PersistedApplianceState<'a> {
    profile_name: &'a str,
    imported_content: Vec<PersistedImportedContentItem<'a>>,
}

/// Persisted state describing a configured DAIA appliance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplianceState {
    profile_name: String,
    imported_content: Vec<model::ImportedContentItem>,
}

impl ApplianceState {
    /// Creates appliance state for the selected appliance profile.
    #[must_use]
    pub fn new(profile_name: impl Into<String>) -> Self {
        Self {
            profile_name: profile_name.into(),
            imported_content: Vec::new(),
        }
    }

    /// Returns the configured appliance profile name.
    #[must_use]
    pub fn profile_name(&self) -> &str {
        &self.profile_name
    }

    /// Records content realized on the configured appliance.
    pub fn record_imported_content(&mut self, item: model::ImportedContentItem) {
        self.imported_content.push(item);
    }

    /// Records multiple content items realized on the configured appliance.
    pub fn record_imported_contents(
        &mut self,
        items: impl IntoIterator<Item = model::ImportedContentItem>,
    ) {
        self.imported_content.extend(items);
    }

    /// Returns content realized on the configured appliance.
    #[must_use]
    pub fn imported_content(&self) -> &[model::ImportedContentItem] {
        &self.imported_content
    }

    /// Serializes the persisted appliance state as JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let persisted = PersistedApplianceState {
            profile_name: &self.profile_name,
            imported_content: self
                .imported_content
                .iter()
                .map(|item| PersistedImportedContentItem {
                    source_item_id: item.source_item_id().as_str(),
                    path: item.path(),
                })
                .collect(),
        };

        serde_json::to_string_pretty(&persisted)
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

#[cfg(test)]
mod tests {
    use super::ApplianceState;

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
                "  ]\n",
                "}"
            )
        );
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
                "  \"imported_content\": []\n",
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
                "  \"imported_content\": []\n",
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
}
