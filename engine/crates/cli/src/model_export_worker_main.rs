//! DAIA Model Export isolated worker.
//!
//! Fail-closed bootstrap only. No request handling, source access,
//! descriptor reception, or export publication is enabled.

use std::os::fd::AsFd;

/// Authenticate a connected broker before considering any export request.
///
/// This handler deliberately denies all operations. Root peer credentials
/// alone do not prove Polkit approval or operation-specific authorization.
#[cfg(target_os = "linux")]
fn handle_export_connection(connection: &std::os::fd::OwnedFd) -> std::io::Result<()> {
    engine::export_ipc::verify_export_root_peer(connection)?;
    engine::export_ipc::receive_export_ipc_probe(connection)?;

    Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "DAIA Model Export operations remain disabled",
    ))
}

#[cfg(all(test, target_os = "linux"))]
mod worker_connection_tests {
    use super::*;
    use std::io::ErrorKind;

    #[test]
    fn every_connection_remains_denied() {
        let (connection, _peer) = engine::export_ipc::create_export_ipc_pair().unwrap();

        let error = handle_export_connection(&connection).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::PermissionDenied);
    }

    #[test]
    fn unprivileged_peer_fails_root_identity_check() {
        if rustix::process::geteuid().as_raw() == 0 {
            return;
        }

        let (connection, _peer) = engine::export_ipc::create_export_ipc_pair().unwrap();

        let error = engine::export_ipc::verify_export_root_peer(&connection).unwrap_err();

        assert_eq!(error.kind(), ErrorKind::PermissionDenied);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod worker_staging_tests {
    use std::io::ErrorKind;

    #[test]
    fn missing_state_directory_is_rejected() {
        let error = super::validate_worker_state_directory_path(None).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::PermissionDenied);
    }

    #[test]
    fn relative_state_directory_is_rejected() {
        let error = super::validate_worker_state_directory_path(Some(std::path::Path::new(
            "relative/staging",
        )))
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::PermissionDenied);
    }

    #[test]
    fn unexpected_absolute_state_directory_is_rejected() {
        for path in [
            "/tmp/daia-model-export",
            "/var/tmp/daia-model-export",
            "/var/lib/private/daia-model-export",
            "/var/lib/other-export",
        ] {
            let error =
                super::validate_worker_state_directory_path(Some(std::path::Path::new(path)))
                    .unwrap_err();

            assert_eq!(
                error.kind(),
                ErrorKind::PermissionDenied,
                "unexpected path was not rejected: {path}"
            );
        }
    }

    #[test]
    fn parent_state_directory_is_rejected() {
        let error =
            super::validate_worker_state_directory_path(Some(std::path::Path::new("/var/lib")))
                .unwrap_err();

        assert_eq!(error.kind(), ErrorKind::PermissionDenied);
    }

    #[test]
    fn untrusted_state_directory_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let staging = directory.path().join("staging");
        std::fs::create_dir(&staging).unwrap();

        assert!(super::validate_worker_state_directory_path(Some(&staging)).is_err());
    }
}

#[cfg(all(test, target_os = "linux"))]
mod systemd_worker_state_integration_tests {
    #[test]
    fn systemd_worker_state_staging_lifecycle() {
        use std::os::fd::AsFd;

        let state =
            std::env::var_os("STATE_DIRECTORY").expect("test requires systemd STATE_DIRECTORY");

        assert_eq!(
            std::path::Path::new(&state),
            std::path::Path::new("/var/lib/daia-model-export"),
            "test requires the production worker state-directory path"
        );

        let directory = super::validate_worker_state_directory()
            .expect("systemd worker state directory must validate");

        engine::export_ipc::validate_export_worker_staging_directory(directory.as_fd())
            .expect("worker staging descriptor must be private");

        let staging = engine::ollama_export::create_worker_private_export_staging(&directory)
            .expect("worker staging creation must succeed");

        let name = staging.name().to_owned();

        let path = std::path::Path::new("/var/lib/private/daia-model-export").join(&name);

        assert!(path.is_dir(), "worker staging directory must exist");

        // Optional synchronization for an external isolation test.
        // The normal test path does not wait for an external controller.
        if std::env::var_os("DAIA_EXPORT_TEST_READY_MARKER").is_some() {
            let runtime = std::env::var_os("RUNTIME_DIRECTORY")
                .expect("test requires systemd RUNTIME_DIRECTORY");

            let runtime = std::path::PathBuf::from(runtime);

            assert_eq!(
                runtime,
                std::path::Path::new("/run/daia-export-test-3489"),
                "unexpected test runtime directory"
            );

            // stdout is observed through systemd-run --pipe.
            // Do not expose the private directory through permissions.
            println!("DAIA_TEST_READY:{}", path.display());

            let release = runtime.join("release");

            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);

            while !release.exists() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "adversarial test release timed out"
                );

                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }

        drop(staging);

        assert!(!path.exists(), "worker staging directory must be removed");
    }
}

#[cfg(target_os = "linux")]
fn validate_worker_state_directory_path(
    path: Option<&std::path::Path>,
) -> std::io::Result<std::os::fd::OwnedFd> {
    let path = path.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "missing systemd STATE_DIRECTORY",
        )
    })?;

    if path != std::path::Path::new("/var/lib/daia-model-export") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "unexpected systemd STATE_DIRECTORY path",
        ));
    }

    engine::export_ipc::open_systemd_export_worker_staging_directory()
}

#[cfg(target_os = "linux")]
fn validate_worker_state_directory() -> std::io::Result<std::os::fd::OwnedFd> {
    let state_directory = std::env::var_os("STATE_DIRECTORY");
    validate_worker_state_directory_path(state_directory.as_deref().map(std::path::Path::new))
}

fn main() {
    let stdin = std::io::stdin();

    #[cfg(target_os = "linux")]
    let staging_directory = match validate_worker_state_directory() {
        Ok(directory) => directory,
        Err(error) => {
            eprintln!("DAIA Model Export worker: invalid staging directory: {error}");
            std::process::exit(1);
        }
    };

    #[cfg(target_os = "linux")]
    {
        // Exercise private staging lifecycle without authorizing exports.
        // The worker remains fail-closed at the connection boundary.
        let staging =
            match engine::ollama_export::create_worker_private_export_staging(&staging_directory) {
                Ok(staging) => staging,
                Err(error) => {
                    eprintln!("DAIA Model Export worker: staging creation failed: {error}");
                    std::process::exit(1);
                }
            };

        drop(staging);
    }

    // Validate systemd activation metadata before accepting a connection.
    // Environment metadata is not independent proof of trusted launch.
    if let Err(error) = engine::export_ipc::validate_export_ipc_activation_metadata(
        std::env::var("LISTEN_PID").ok().as_deref(),
        std::env::var("LISTEN_FDS").ok().as_deref(),
        std::env::var("LISTEN_FDNAMES").ok().as_deref(),
        std::process::id(),
    ) {
        eprintln!("DAIA Model Export worker: invalid activation metadata: {error}");
        std::process::exit(1);
    }

    if let Err(error) = engine::export_ipc::validate_export_ipc_listener(stdin.as_fd()) {
        eprintln!("DAIA Model Export worker: invalid activation listener: {error}");
        std::process::exit(1);
    }

    // The worker remains fail-closed: one connection is authenticated,
    // then rejected without reading requests or accepting descriptors.
    let connection = match rustix::net::accept(stdin.as_fd()) {
        Ok(connection) => connection,
        Err(error) => {
            eprintln!("DAIA Model Export worker: accept failed: {error}");
            std::process::exit(1);
        }
    };

    if let Err(error) = handle_export_connection(&connection) {
        eprintln!("DAIA Model Export worker: {error}");
        std::process::exit(1);
    }

    // No successful operation path is currently permitted.
    eprintln!("DAIA Model Export worker: operations disabled");
    std::process::exit(1);
}
