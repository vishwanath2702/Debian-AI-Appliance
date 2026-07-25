use std::fs;

use engine::Engine;
use model::{Action, Capability, PlanStep, ProviderId};
use registry::Registry;

const DESKTOP_PROVIDER_YAML: &str = r"
id: desktop
capability: desktop
steps:
  - install_package_manifest: desktop
  - enable_service: display-manager
";

#[test]
fn plans_capability_from_yaml_provider_directory() {
    let directory = tempfile::tempdir().expect("temporary directory should exist");

    fs::write(directory.path().join("desktop.yaml"), DESKTOP_PROVIDER_YAML)
        .expect("desktop provider YAML should be written");

    let registry =
        Registry::from_directory(directory.path()).expect("provider directory should load");

    let engine = Engine::from_registry(registry);

    let plan = engine
        .plan(&Capability::new("desktop"))
        .expect("desktop capability should produce a plan");

    assert_eq!(plan.capability, Capability::new("desktop"));
    assert_eq!(plan.provider, ProviderId::new("desktop"));
    assert_eq!(
        plan.steps,
        vec![
            PlanStep::new(Action::InstallPackageManifest("desktop".to_owned())),
            PlanStep::new(Action::EnableService("display-manager".to_owned())),
        ]
    );
}
