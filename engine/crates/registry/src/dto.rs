use model::{Action, CapabilityId, PlanStep, Provider, ProviderId};
use serde::Deserialize;

use crate::RegistryError;

/// Deserializes and validates one provider YAML document.
#[allow(clippy::redundant_pub_crate)]
pub(crate) fn provider_from_yaml(yaml: &str) -> Result<Provider, RegistryError> {
    let provider_dto = serde_yaml::from_str::<RegistryProviderDto>(yaml)?;

    provider_dto.try_into_provider()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryProviderDto {
    id: String,
    capability: String,
    steps: Vec<RegistryStepDto>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RegistryStepDto {
    InstallPackageManifest { install_package_manifest: String },
    EnableService { enable_service: String },
}

impl RegistryProviderDto {
    fn try_into_provider(self) -> Result<Provider, RegistryError> {
        let id = validated_value(&self.id, "provider id")?;
        let capability = validated_value(&self.capability, "provider capability")?;

        if self.steps.is_empty() {
            return Err(RegistryError::InvalidProvider(
                "provider must contain at least one step".to_owned(),
            ));
        }

        let steps = self
            .steps
            .into_iter()
            .map(RegistryStepDto::try_into_plan_step)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Provider {
            id: ProviderId::new(id),
            capability: CapabilityId::new(capability),
            steps,
        })
    }
}

impl RegistryStepDto {
    fn try_into_plan_step(self) -> Result<PlanStep, RegistryError> {
        match self {
            Self::InstallPackageManifest {
                install_package_manifest,
            } => {
                let manifest = validated_value(&install_package_manifest, "package manifest name")?;

                Ok(PlanStep::new(Action::InstallPackageManifest(manifest)))
            }
            Self::EnableService { enable_service } => {
                let service = validated_value(&enable_service, "service name")?;

                Ok(PlanStep::new(Action::EnableService(service)))
            }
        }
    }
}

fn validated_value(value: &str, field_name: &str) -> Result<String, RegistryError> {
    let value = value.trim();

    if value.is_empty() {
        return Err(RegistryError::InvalidProvider(format!(
            "{field_name} must not be empty"
        )));
    }

    Ok(value.to_owned())
}
