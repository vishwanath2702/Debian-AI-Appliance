//! DAIA Model Export authorization broker.
//!
//! No caller identity, socket permission, or environment variable grants
//! authorization. An independently verified authorization mechanism must
//! be implemented before export operations are enabled.

use model::ModelExportOperation;

#[derive(Clone, Debug, Eq, PartialEq)]
enum AuthorizationDecision {
    Denied,
    VerifiedForOperation(Vec<u8>),
}

fn authorize_export(
    decision: &AuthorizationDecision,
    operation: &ModelExportOperation,
) -> Result<(), &'static str> {
    match decision {
        AuthorizationDecision::VerifiedForOperation(approved)
            if approved == &operation.canonical_bytes() =>
        {
            Ok(())
        }
        _ => Err("DAIA Model Export authorization unavailable"),
    }
}

fn main() {
    // No authorization provider is connected. Always deny.
    // No request parser or authorization provider exists yet.
    // Never construct a verified decision from caller-controlled input.
    eprintln!("DAIA Model Export authorization unavailable");
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::{ModelExportArtifact, ModelExportOperationId, ModelExportSource};

    fn operation(name: &str) -> ModelExportOperation {
        ModelExportOperation::new(
            ModelExportOperationId::new("operation-1").unwrap(),
            ModelExportSource::Ollama,
            name,
            vec![ModelExportArtifact::new("model.gguf", "a".repeat(64), 12).unwrap()],
        )
        .unwrap()
    }

    #[test]
    fn broker_denies_without_verified_authorization() {
        assert!(authorize_export(&AuthorizationDecision::Denied, &operation("export")).is_err());
    }

    #[test]
    fn broker_accepts_only_matching_operation() {
        let requested = operation("export");
        let decision = AuthorizationDecision::VerifiedForOperation(requested.canonical_bytes());

        assert!(authorize_export(&decision, &requested).is_ok());
        assert!(authorize_export(&decision, &operation("other-export")).is_err());
    }

    #[test]
    fn broker_rejects_untrusted_claims() {
        let requested = operation("export");
        for claim in [
            "authorized",
            "verified",
            "root",
            "pkexec",
            "org.daia.model-export",
            "--authorized",
            "--verified",
        ] {
            let decision = AuthorizationDecision::VerifiedForOperation(claim.as_bytes().to_vec());
            assert!(
                authorize_export(&decision, &requested).is_err(),
                "untrusted claim must not authorize: {claim}"
            );
        }
    }

    #[test]
    fn broker_denies_without_an_authorization_provider() {
        assert!(authorize_export(&AuthorizationDecision::Denied, &operation("export")).is_err());
    }
}
