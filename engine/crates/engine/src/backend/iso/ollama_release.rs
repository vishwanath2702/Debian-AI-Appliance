//! Local integrity verification for the staged Ollama ISO archive.

use rustix::fs::{FileType, Mode, OFlags, RawDir, ResolveFlags};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read};
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::path::{Component, Path};

const ARCHIVE_NAME: &str = "ollama-linux-amd64.tar.zst";
const MANIFEST_NAME: &str = "ollama-release.json";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OllamaReleaseManifest {
    schema_version: u32,
    component: String,
    version: String,
    architecture: String,
    asset: String,
    sha256: String,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn open_directory_without_symlinks(path: &Path) -> io::Result<OwnedFd> {
    if !path.is_absolute() {
        return Err(invalid("Ollama directory path must be absolute"));
    }

    let mut directory = rustix::fs::open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    for component in path.components() {
        match component {
            Component::RootDir => continue,
            Component::Normal(name) => {
                directory = rustix::fs::openat2(
                    &directory,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                    Mode::empty(),
                    ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
                )?;
            }
            _ => return Err(invalid("Invalid Ollama directory path component")),
        }
    }

    Ok(directory)
}

fn regular_file(directory: &OwnedFd, name: &str) -> io::Result<File> {
    let descriptor = rustix::fs::openat2(
        directory,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
    )?;

    let metadata = rustix::fs::fstat(&descriptor)?;

    if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile || metadata.st_nlink != 1
    {
        return Err(invalid("Unsafe Ollama artifact file type or link count"));
    }

    Ok(File::from(descriptor))
}

/// Rejects an incomplete Ollama-enabled DAIA payload before ISO workspace creation.
///
/// A missing payload is permitted for the existing minimal ISO backend mode.
/// A supplied payload must contain the DAIA configuration contract.
pub fn preflight_daia_payload(payload: Option<&Path>) -> io::Result<()> {
    let Some(payload) = payload else {
        return Ok(());
    };

    if ollama_enabled(payload)? {
        verify_staged_ollama_archive(&payload.join("opt/daia/payload/packages/ollama"))?;
    }

    Ok(())
}

/// Enforces local integrity and, by default, official latest-release verification.
///
/// Offline approval skips only the live lookup, never local verification.
pub fn preflight_ollama_release(payload: Option<&Path>, offline_approved: bool) -> io::Result<()> {
    preflight_ollama_release_with(payload, offline_approved, fetch_official_release_response)
}

fn preflight_ollama_release_with<F>(
    payload: Option<&Path>,
    offline_approved: bool,
    fetch: F,
) -> io::Result<()>
where
    F: FnOnce() -> io::Result<Vec<u8>>,
{
    let Some(payload) = payload else {
        return Ok(());
    };

    preflight_daia_payload(Some(payload))?;

    if ollama_enabled(payload)? && !offline_approved {
        verify_official_latest_release_with(
            &payload.join("opt/daia/payload/packages/ollama"),
            fetch,
        )?;
    }

    Ok(())
}

fn ollama_enabled(payload: &Path) -> io::Result<bool> {
    let root = payload.join("opt/daia");
    let config_directory = open_directory_without_symlinks(&root.join("config"))?;
    let mut config_file = regular_file(&config_directory, "pragna.conf")?;
    let mut config_bytes = Vec::new();
    config_file
        .by_ref()
        .take(64 * 1024 + 1)
        .read_to_end(&mut config_bytes)?;

    if config_bytes.len() > 64 * 1024 {
        return Err(invalid("DAIA configuration exceeds size limit"));
    }

    let config =
        String::from_utf8(config_bytes).map_err(|_| invalid("DAIA configuration is not UTF-8"))?;

    let mut ollama_setting = None;

    for line in config.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(value) = line.strip_prefix("DAIA_ENABLE_OLLAMA=") {
            if ollama_setting.is_some() {
                return Err(invalid("Duplicate DAIA_ENABLE_OLLAMA setting"));
            }

            ollama_setting = match value {
                r#""true""# => Some("true"),
                r#""false""# => Some("false"),
                _ => return Err(invalid("Invalid DAIA_ENABLE_OLLAMA assignment")),
            };
        }
    }

    match ollama_setting {
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        _ => Err(invalid("Missing or invalid DAIA_ENABLE_OLLAMA setting")),
    }
}

/// Verifies a staged Ollama release manifest and its single archive.
///
/// This is a local integrity check, not an official latest-release check.
pub fn verify_staged_ollama_archive(directory: &Path) -> io::Result<()> {
    let directory_fd = open_directory_without_symlinks(directory)?;

    let mut manifest_file = regular_file(&directory_fd, MANIFEST_NAME)?;
    let mut manifest_bytes = Vec::new();
    manifest_file
        .by_ref()
        .take(16 * 1024 + 1)
        .read_to_end(&mut manifest_bytes)?;

    if manifest_bytes.len() > 16 * 1024 {
        return Err(invalid("Ollama release manifest is oversized"));
    }

    let manifest: OllamaReleaseManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| invalid("Invalid Ollama release manifest"))?;

    if manifest.schema_version != 1
        || manifest.component != "ollama"
        || manifest.architecture != "amd64"
        || manifest.asset != ARCHIVE_NAME
        || !valid_version(&manifest.version)
        || manifest.sha256.len() != 64
        || !manifest
            .sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(invalid("Unsupported Ollama release metadata"));
    }

    let mut archive_count = 0;
    let mut entry_buffer = [MaybeUninit::<u8>::uninit(); 8192];
    let mut entries = RawDir::new(&directory_fd, &mut entry_buffer);

    while let Some(result) = entries.next() {
        let entry = result?;
        let name = entry.file_name().to_bytes();

        if name == b"." || name == b".." {
            continue;
        }

        if name.ends_with(b".tar.zst") {
            archive_count += 1;

            if name != ARCHIVE_NAME.as_bytes() {
                return Err(invalid("Unexpected Ollama archive"));
            }
        }
    }

    drop(entries);

    if archive_count != 1 {
        return Err(invalid("Expected exactly one Ollama archive"));
    }

    let mut archive = regular_file(&directory_fd, ARCHIVE_NAME)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];

    loop {
        let count = archive.read(&mut buffer)?;

        if count == 0 {
            break;
        }

        hasher.update(&buffer[..count]);
    }

    if format!("{:x}", hasher.finalize()) != manifest.sha256 {
        return Err(invalid("Ollama archive SHA256 mismatch"));
    }

    Ok(())
}

#[derive(Deserialize)]
struct OfficialRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<OfficialAsset>,
}

#[derive(Deserialize)]
struct OfficialAsset {
    name: String,
    digest: String,
}

fn validate_official_release_response(
    response: &[u8],
    expected_version: &str,
    expected_sha256: &str,
) -> io::Result<()> {
    if response.len() > 1024 * 1024 {
        return Err(invalid("Official Ollama response exceeds size limit"));
    }

    let release: OfficialRelease = serde_json::from_slice(response)
        .map_err(|_| invalid("Invalid official Ollama release response"))?;

    if release.draft || release.prerelease {
        return Err(invalid("Official Ollama release is not stable"));
    }

    if !valid_version(&release.tag_name) || release.tag_name != expected_version {
        return Err(invalid("Pinned Ollama version is not latest stable"));
    }

    let matching_assets: Vec<_> = release
        .assets
        .iter()
        .filter(|asset| asset.name == ARCHIVE_NAME)
        .collect();

    if matching_assets.len() != 1 {
        return Err(invalid(
            "Official Ollama archive asset is missing or ambiguous",
        ));
    }

    if matching_assets[0].digest != format!("sha256:{expected_sha256}") {
        return Err(invalid(
            "Pinned Ollama checksum differs from official release",
        ));
    }

    Ok(())
}

const OFFICIAL_RELEASE_API: &str = "https://api.github.com/repos/ollama/ollama/releases/latest";

fn fetch_official_release_response() -> io::Result<Vec<u8>> {
    use std::process::{Command, Stdio};

    let response = tempfile::tempfile()?;
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--connect-timeout",
            "10",
            "--max-time",
            "30",
            "--max-redirs",
            "3",
            "--max-filesize",
            "1048576",
            "--header",
            "Accept: application/vnd.github+json",
            "--header",
            "User-Agent: DAIA-Ollama-Release-Preflight",
            OFFICIAL_RELEASE_API,
        ])
        .stdout(Stdio::from(response.try_clone()?))
        .stderr(Stdio::null())
        .status()?;

    if !output.success() {
        return Err(io::Error::other("Official Ollama release lookup failed"));
    }

    let mut reader = response;
    use std::io::Seek;
    reader.rewind()?;

    let mut bytes = Vec::new();
    reader.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;

    if bytes.len() > 1024 * 1024 {
        return Err(invalid(
            "Official Ollama release response exceeds size limit",
        ));
    }

    Ok(bytes)
}

/// Compares the staged Ollama manifest with the official latest stable release.
///
/// Network failure, malformed metadata, or a stale pin rejects the build.
fn verify_official_latest_release(directory: &Path) -> io::Result<()> {
    verify_official_latest_release_with(directory, fetch_official_release_response)
}

fn verify_official_latest_release_with<F>(directory: &Path, fetch: F) -> io::Result<()>
where
    F: FnOnce() -> io::Result<Vec<u8>>,
{
    let directory_fd = open_directory_without_symlinks(directory)?;
    let mut manifest_file = regular_file(&directory_fd, MANIFEST_NAME)?;

    let mut bytes = Vec::new();
    manifest_file
        .by_ref()
        .take(16 * 1024 + 1)
        .read_to_end(&mut bytes)?;

    if bytes.len() > 16 * 1024 {
        return Err(invalid("Ollama release manifest exceeds size limit"));
    }

    let manifest: OllamaReleaseManifest =
        serde_json::from_slice(&bytes).map_err(|_| invalid("Invalid Ollama release manifest"))?;

    if manifest.schema_version != 1
        || manifest.component != "ollama"
        || manifest.architecture != "amd64"
        || manifest.asset != ARCHIVE_NAME
        || !valid_version(&manifest.version)
        || manifest.sha256.len() != 64
        || !manifest
            .sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(invalid("Unsupported Ollama release metadata"));
    }

    let response = fetch()?;

    validate_official_release_response(&response, &manifest.version, &manifest.sha256)
}

fn valid_version(version: &str) -> bool {
    let Some(numbers) = version.strip_prefix('v') else {
        return false;
    };

    let parts: Vec<_> = numbers.split('.').collect();

    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::verify_staged_ollama_archive;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    const ARCHIVE: &str = "ollama-linux-amd64.tar.zst";

    fn fixture() -> tempfile::TempDir {
        let temp = tempdir().unwrap();
        let bytes = b"isolated ollama test archive";
        let digest = format!("{:x}", Sha256::digest(bytes));

        fs::write(temp.path().join(ARCHIVE), bytes).unwrap();

        fs::write(
            temp.path().join("ollama-release.json"),
            format!(
                r#"{{"schema_version":1,"component":"ollama","version":"v0.40.15","architecture":"amd64","asset":"{ARCHIVE}","sha256":"{digest}"}}"#
            ),
        )
        .unwrap();

        temp
    }

    #[test]
    fn release_policy_respects_explicit_offline_approval() {
        use super::preflight_ollama_release_with;
        use std::io;

        let temp = preflight_fixture("DAIA_ENABLE_OLLAMA=\"true\"\n");
        let archive_dir = temp.path().join("opt/daia/payload/packages/ollama");
        fs::create_dir_all(&archive_dir).unwrap();

        let bytes = b"isolated ollama test archive";
        let digest = format!("{:x}", Sha256::digest(bytes));

        fs::write(archive_dir.join(ARCHIVE), bytes).unwrap();
        fs::write(
            archive_dir.join("ollama-release.json"),
            format!(
                r#"{{"schema_version":1,"component":"ollama","version":"v0.40.15","architecture":"amd64","asset":"{ARCHIVE}","sha256":"{digest}"}}"#
            ),
        )
        .unwrap();

        let response = serde_json::to_vec(&serde_json::json!({
            "tag_name": "v0.40.15",
            "draft": false,
            "prerelease": false,
            "assets": [{
                "name": ARCHIVE,
                "digest": format!("sha256:{digest}")
            }]
        }))
        .unwrap();

        assert!(preflight_ollama_release_with(Some(temp.path()), false, || Ok(response)).is_ok());

        assert!(
            preflight_ollama_release_with(Some(temp.path()), false, || {
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "network unavailable",
                ))
            })
            .is_err()
        );

        assert!(
            preflight_ollama_release_with(Some(temp.path()), true, || {
                panic!("approved offline mode must not access network")
            })
            .is_ok()
        );

        fs::write(archive_dir.join(ARCHIVE), b"tampered").unwrap();

        assert!(
            preflight_ollama_release_with(Some(temp.path()), true, || {
                panic!("invalid archive must fail before network access")
            })
            .is_err()
        );

        let disabled = preflight_fixture("DAIA_ENABLE_OLLAMA=\"false\"\n");

        assert!(
            preflight_ollama_release_with(Some(disabled.path()), false, || {
                panic!("disabled Ollama must not access network")
            })
            .is_ok()
        );

        assert!(
            preflight_ollama_release_with(None, false, || {
                panic!("missing optional payload must not access network")
            })
            .is_ok()
        );
    }

    #[test]
    fn official_release_response_rejects_invalid_metadata() {
        use super::validate_official_release_response;

        let version = "v0.40.15";
        let digest = "a".repeat(64);

        let response = |tag: &str, checksum: &str, draft: bool, prerelease: bool, assets: usize| {
            let assets: Vec<_> = (0..assets)
                .map(|_| {
                    serde_json::json!({
                        "name": "ollama-linux-amd64.tar.zst",
                        "digest": format!("sha256:{checksum}")
                    })
                })
                .collect();

            serde_json::to_vec(&serde_json::json!({
                "tag_name": tag,
                "draft": draft,
                "prerelease": prerelease,
                "assets": assets
            }))
            .unwrap()
        };

        assert!(
            validate_official_release_response(
                &response(version, &digest, false, false, 1),
                version,
                &digest
            )
            .is_ok()
        );

        assert!(
            validate_official_release_response(
                &response("v0.40.16", &digest, false, false, 1),
                version,
                &digest
            )
            .is_err()
        );

        assert!(
            validate_official_release_response(
                &response(version, &"b".repeat(64), false, false, 1),
                version,
                &digest
            )
            .is_err()
        );

        for count in [0, 2] {
            assert!(
                validate_official_release_response(
                    &response(version, &digest, false, false, count),
                    version,
                    &digest
                )
                .is_err()
            );
        }

        assert!(
            validate_official_release_response(
                &response(version, &digest, true, false, 1),
                version,
                &digest
            )
            .is_err()
        );

        assert!(
            validate_official_release_response(
                &response(version, &digest, false, true, 1),
                version,
                &digest
            )
            .is_err()
        );

        assert!(validate_official_release_response(b"not-json", version, &digest).is_err());

        assert!(
            validate_official_release_response(&vec![b'x'; 1024 * 1024 + 1], version, &digest)
                .is_err()
        );
    }

    #[test]
    fn mocked_official_release_verification() {
        use super::verify_official_latest_release_with;
        use std::io;

        let temp = fixture();
        let digest = format!("{:x}", Sha256::digest(b"isolated ollama test archive"));

        let response = |version: &str, checksum: &str| {
            serde_json::to_vec(&serde_json::json!({
                "tag_name": version,
                "draft": false,
                "prerelease": false,
                "assets": [{
                    "name": "ollama-linux-amd64.tar.zst",
                    "digest": format!("sha256:{checksum}")
                }]
            }))
            .unwrap()
        };

        assert!(
            verify_official_latest_release_with(temp.path(), || Ok(response("v0.40.15", &digest)))
                .is_ok()
        );

        assert!(
            verify_official_latest_release_with(temp.path(), || Ok(response("v0.40.16", &digest)))
                .is_err()
        );

        assert!(
            verify_official_latest_release_with(temp.path(), || Ok(response(
                "v0.40.15",
                &"b".repeat(64)
            )))
            .is_err()
        );

        assert!(
            verify_official_latest_release_with(temp.path(), || Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "simulated GitHub timeout"
            )))
            .is_err()
        );
    }

    #[test]
    fn accepts_matching_manifest_and_archive() {
        let temp = fixture();
        verify_staged_ollama_archive(temp.path()).unwrap();
    }

    #[test]
    fn rejects_missing_manifest() {
        let temp = fixture();
        fs::remove_file(temp.path().join("ollama-release.json")).unwrap();
        assert!(verify_staged_ollama_archive(temp.path()).is_err());
    }

    #[test]
    fn rejects_archive_tampered_after_copy() {
        let source = fixture();
        let destination = tempfile::tempdir().unwrap();

        let archive_name = "ollama-linux-amd64.tar.zst";
        let manifest_name = "ollama-release.json";

        verify_staged_ollama_archive(source.path())
            .expect("original payload should pass verification");

        fs::copy(
            source.path().join(archive_name),
            destination.path().join(archive_name),
        )
        .unwrap();

        fs::copy(
            source.path().join(manifest_name),
            destination.path().join(manifest_name),
        )
        .unwrap();

        verify_staged_ollama_archive(destination.path())
            .expect("copied payload should initially pass");

        fs::write(
            destination.path().join(archive_name),
            b"modified-after-copy",
        )
        .unwrap();

        assert!(
            verify_staged_ollama_archive(destination.path()).is_err(),
            "modified copied archive must be rejected"
        );

        verify_staged_ollama_archive(source.path()).expect("original source should remain valid");
    }

    #[test]
    fn rejects_tampered_archive() {
        let temp = fixture();
        fs::write(temp.path().join(ARCHIVE), b"tampered").unwrap();
        assert!(verify_staged_ollama_archive(temp.path()).is_err());
    }

    #[test]
    fn rejects_duplicate_archive() {
        let temp = fixture();
        fs::write(temp.path().join("duplicate.tar.zst"), b"duplicate").unwrap();
        assert!(verify_staged_ollama_archive(temp.path()).is_err());
    }

    #[test]
    fn rejects_symlinked_archive() {
        let temp = fixture();
        let archive = temp.path().join(ARCHIVE);
        fs::remove_file(&archive).unwrap();
        symlink(temp.path().join("ollama-release.json"), archive).unwrap();

        assert!(verify_staged_ollama_archive(temp.path()).is_err());
    }

    fn preflight_fixture(config: &str) -> tempfile::TempDir {
        let temp = tempdir().unwrap();
        let config_dir = temp.path().join("opt/daia/config");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("pragna.conf"), config).unwrap();
        temp
    }

    #[test]
    fn preflight_accepts_disabled_ollama() {
        let temp = preflight_fixture("DAIA_ENABLE_OLLAMA=\"false\"\n");
        assert!(super::preflight_daia_payload(Some(temp.path())).is_ok());
    }

    #[test]
    fn preflight_rejects_missing_manifest() {
        let temp = preflight_fixture("DAIA_ENABLE_OLLAMA=\"true\"\n");
        assert!(super::preflight_daia_payload(Some(temp.path())).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn preflight_rejects_symlinked_config() {
        use std::os::unix::fs::symlink;

        let temp = preflight_fixture("DAIA_ENABLE_OLLAMA=\"false\"\n");
        let config = temp.path().join("opt/daia/config/pragna.conf");
        let actual = temp.path().join("actual-config");

        fs::rename(&config, &actual).unwrap();
        symlink(&actual, &config).unwrap();

        assert!(super::preflight_daia_payload(Some(temp.path())).is_err());
    }

    #[test]
    fn preflight_rejects_oversized_config() {
        let temp = preflight_fixture("DAIA_ENABLE_OLLAMA=\"false\"\n");
        let config = temp.path().join("opt/daia/config/pragna.conf");

        fs::write(&config, "x".repeat(65 * 1024)).unwrap();

        assert!(super::preflight_daia_payload(Some(temp.path())).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn preflight_rejects_symlinked_config_directory() {
        use std::os::unix::fs::symlink;

        let temp = preflight_fixture("DAIA_ENABLE_OLLAMA=\"false\"\n");
        let config_dir = temp.path().join("opt/daia/config");
        let actual = temp.path().join("actual-config-dir");

        fs::rename(&config_dir, &actual).unwrap();
        symlink(&actual, &config_dir).unwrap();

        assert!(super::preflight_daia_payload(Some(temp.path())).is_err());
    }

    #[test]
    fn preflight_rejects_missing_config() {
        let temp = tempdir().unwrap();
        assert!(super::preflight_daia_payload(Some(temp.path())).is_err());
    }

    #[test]
    fn preflight_rejects_missing_setting() {
        let temp = preflight_fixture("DAIA_ENABLE_DOCKER=\"true\"\n");
        assert!(super::preflight_daia_payload(Some(temp.path())).is_err());
    }

    #[test]
    fn preflight_rejects_duplicate_setting() {
        let temp = preflight_fixture("DAIA_ENABLE_OLLAMA=\"true\"\nDAIA_ENABLE_OLLAMA=\"false\"\n");
        assert!(super::preflight_daia_payload(Some(temp.path())).is_err());
    }

    #[test]
    fn preflight_rejects_malformed_quoting() {
        for assignment in [
            "DAIA_ENABLE_OLLAMA=true\n",
            "DAIA_ENABLE_OLLAMA=\"true\n",
            "DAIA_ENABLE_OLLAMA=true\"\n",
            "DAIA_ENABLE_OLLAMA=\"\"true\"\"\n",
            "DAIA_ENABLE_OLLAMA=\"true\" extra\n",
            "DAIA_ENABLE_OLLAMA =\"true\"\n",
        ] {
            let temp = preflight_fixture(assignment);
            assert!(
                super::preflight_daia_payload(Some(temp.path())).is_err(),
                "malformed assignment accepted: {assignment:?}"
            );
        }
    }

    #[test]
    fn preflight_rejects_invalid_setting() {
        let temp = preflight_fixture("DAIA_ENABLE_OLLAMA=\"maybe\"\n");
        assert!(super::preflight_daia_payload(Some(temp.path())).is_err());
    }

    #[test]
    fn rejects_invalid_release_version() {
        let temp = fixture();
        let manifest = temp.path().join("ollama-release.json");
        let original = fs::read_to_string(&manifest).unwrap();

        fs::write(&manifest, original.replace("v0.40.15", "v0.40.15-beta")).unwrap();

        assert!(verify_staged_ollama_archive(temp.path()).is_err());
    }

    #[test]
    fn rejects_symlinked_ancestor_directory() {
        let temp = fixture();
        let parent = temp.path().parent().unwrap();
        let alias = parent.join(format!("daia-ollama-test-alias-{}", std::process::id()));

        // Use a separate temporary directory to avoid fixed-name collisions.
        let wrapper = tempdir().unwrap();
        let link = wrapper.path().join("linked-parent");
        symlink(parent, &link).unwrap();

        let name = temp.path().file_name().unwrap();
        let linked_path = link.join(name);

        assert!(verify_staged_ollama_archive(&linked_path).is_err());

        // Keep the compiler from treating the diagnostic alias as an
        // unused variable; no filesystem entry is created at this path.
        assert!(alias.is_absolute());
    }

    #[test]
    fn rejects_symlinked_archive_directory() {
        let temp = fixture();
        let wrapper = tempdir().unwrap();
        let link = wrapper.path().join("ollama");
        symlink(temp.path(), &link).unwrap();

        assert!(verify_staged_ollama_archive(&link).is_err());
    }

    #[test]
    fn rejects_hardlinked_archive() {
        let temp = fixture();
        let archive = temp.path().join(ARCHIVE);
        let second = temp.path().join("archive-hardlink");

        fs::hard_link(&archive, &second).unwrap();

        assert!(verify_staged_ollama_archive(temp.path()).is_err());
    }

    #[test]
    fn rejects_symlinked_manifest() {
        let temp = fixture();
        let manifest = temp.path().join("ollama-release.json");
        let actual = temp.path().join("actual-manifest.json");

        fs::rename(&manifest, &actual).unwrap();
        symlink(&actual, &manifest).unwrap();

        assert!(verify_staged_ollama_archive(temp.path()).is_err());
    }
}
