use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactManifest {
    pub schema_version: u32,
    pub name: String,
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub source: String,
    pub destination: String,
    pub sha256: String,
    pub mode: String,
}

fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && path.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

pub fn validate_artifact_manifest(yaml: &str) -> Result<ArtifactManifest, String> {
    let manifest: ArtifactManifest =
        serde_yaml::from_str(yaml).map_err(|error| error.to_string())?;

    if manifest.schema_version != 1 {
        return Err("unsupported artifact manifest version".into());
    }
    if manifest.name.is_empty()
        || !manifest.name.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || (index > 0 && (byte.is_ascii_digit() || byte == b'-'))
        })
    {
        return Err("invalid artifact manifest name".into());
    }
    if manifest.artifacts.is_empty() {
        return Err("artifact manifest must not be empty".into());
    }

    let mut destinations = std::collections::HashSet::new();
    for artifact in &manifest.artifacts {
        if !valid_path(&artifact.source) || !valid_path(&artifact.destination) {
            return Err("unsafe artifact path".into());
        }
        if !destinations.insert(&artifact.destination) {
            return Err("duplicate artifact destination".into());
        }
        if artifact.sha256.len() != 64
            || !artifact.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()
                && !byte.is_ascii_uppercase())
        {
            return Err("invalid artifact SHA-256".into());
        }
        if artifact.mode != "0644" && artifact.mode != "0755" {
            return Err("unsupported artifact file mode".into());
        }
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::validate_artifact_manifest;

    const GOOD: &str = include_str!(
        "../../../../payload/artifact-manifests/tika.yaml"
    );

    #[test]
    fn accepts_tika_manifest() {
        assert!(validate_artifact_manifest(GOOD).is_ok());
    }

    #[test]
    fn rejects_parent_traversal() {
        let invalid = GOOD.replace(
            "payload/images/tika-3.3.3.0.tar",
            "../payload/images/tika-3.3.3.0.tar",
        );
        assert!(validate_artifact_manifest(&invalid).is_err());
    }

    #[test]
    fn rejects_unknown_fields() {
        assert!(validate_artifact_manifest(
            &format!("{GOOD}\nunknown: true\n")
        ).is_err());
    }

    #[test]
    fn rejects_duplicate_yaml_keys() {
        assert!(validate_artifact_manifest(
            &format!("{GOOD}\nname: duplicate\n")
        ).is_err());
    }

    #[test]
    fn rejects_unsupported_version() {
        assert!(validate_artifact_manifest(
            &GOOD.replace("schema_version: 1", "schema_version: 2")
        ).is_err());
    }

    #[test]
    fn rejects_unsafe_mode() {
        assert!(validate_artifact_manifest(
            &GOOD.replace("\"0644\"", "\"0777\"")
        ).is_err());
    }

    #[test]
    fn rejects_duplicate_destination() {
        let item = GOOD.split_once("  - source:").unwrap().1;
        let invalid = format!("{GOOD}  - source:{item}");
        assert!(validate_artifact_manifest(&invalid).is_err());
    }
}
