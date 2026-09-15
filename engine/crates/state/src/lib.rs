//! Desired, current, and persisted system state.

/// Persisted state describing a configured DAIA appliance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplianceState {
    profile_name: String,
}

impl ApplianceState {
    /// Creates appliance state for the selected appliance profile.
    #[must_use]
    pub fn new(profile_name: impl Into<String>) -> Self {
        Self {
            profile_name: profile_name.into(),
        }
    }

    /// Returns the configured appliance profile name.
    #[must_use]
    pub fn profile_name(&self) -> &str {
        &self.profile_name
    }
}

#[cfg(test)]
mod tests {
    use super::ApplianceState;

    #[test]
    fn stores_appliance_profile_name() {
        let state = ApplianceState::new("ai-workstation");

        assert_eq!(state.profile_name(), "ai-workstation");
    }
}
