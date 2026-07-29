// engine/crates/inspector/src/debian.rs

//! Parsing of Debian installation-media metadata.

use std::path::PathBuf;

use crate::{BootMode, InspectError, IsoMetadata, IsoReader};

const OFFICIAL_SEPARATOR: &str = " - Official";
const BIOS_BOOT_PATH: &str = "/isolinux/isolinux.bin";
const UEFI_BOOT_PATH: &str = "/EFI/BOOT/BOOTX64.EFI";

fn detect_boot_modes(reader: &impl IsoReader) -> Result<Vec<BootMode>, InspectError> {
    let mut boot_modes = Vec::new();

    if reader.path_exists(BIOS_BOOT_PATH)? {
        boot_modes.push(BootMode::Bios);
    }

    if reader.path_exists(UEFI_BOOT_PATH)? {
        boot_modes.push(BootMode::Uefi);
    }

    Ok(boot_modes)
}
/// Parses the contents of Debian's `/.disk/info` file.
///
/// The supported format is similar to:
///
/// `Debian GNU/Linux 13.1.0 "Trixie" - Official amd64 NETINST`
///
/// # Errors
///
/// Returns [`InspectError::InvalidDiskInfo`] when required metadata cannot be
/// extracted.
pub fn parse_disk_info(contents: &str) -> Result<IsoMetadata, InspectError> {
    let line = contents
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .ok_or(InspectError::InvalidDiskInfo {
            reason: "metadata is empty",
        })?;

    let (release, media) =
        line.split_once(OFFICIAL_SEPARATOR)
            .ok_or(InspectError::InvalidDiskInfo {
                reason: "missing official-media separator",
            })?;

    let codename_start = release.find('"').ok_or(InspectError::InvalidDiskInfo {
        reason: "missing opening codename quote",
    })?;

    let codename_end_relative =
        release[codename_start + 1..]
            .find('"')
            .ok_or(InspectError::InvalidDiskInfo {
                reason: "missing closing codename quote",
            })?;

    let codename_end = codename_start + 1 + codename_end_relative;

    let codename = &release[codename_start + 1..codename_end];

    if codename.is_empty() {
        return Err(InspectError::InvalidDiskInfo {
            reason: "codename is empty",
        });
    }

    let release_prefix = release[..codename_start].trim();

    let (distribution, version) =
        release_prefix
            .rsplit_once(' ')
            .ok_or(InspectError::InvalidDiskInfo {
                reason: "missing distribution or version",
            })?;

    if distribution.is_empty() {
        return Err(InspectError::InvalidDiskInfo {
            reason: "distribution is empty",
        });
    }

    if version.is_empty() {
        return Err(InspectError::InvalidDiskInfo {
            reason: "version is empty",
        });
    }

    let mut media_fields = media.split_whitespace();

    let architecture = media_fields.next().ok_or(InspectError::InvalidDiskInfo {
        reason: "missing architecture",
    })?;

    let media_type = media_fields.next().ok_or(InspectError::InvalidDiskInfo {
        reason: "missing media type",
    })?;

    Ok(IsoMetadata::new(
        PathBuf::new(),
        distribution.to_owned(),
        version.to_owned(),
        codename.to_owned(),
        architecture.to_owned(),
        media_type.to_owned(),
        Vec::new(),
    ))
}
/// Inspects Debian installation media using `xorriso`.
#[derive(Clone, Debug, Default)]
pub struct DebianIsoInspector;

impl DebianIsoInspector {
    /// Creates a Debian ISO inspector.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl crate::IsoInspector for DebianIsoInspector {
    fn inspect(&self, path: &std::path::Path) -> Result<IsoMetadata, InspectError> {
        use crate::XorrisoReader;

        let reader = XorrisoReader::new(path);
        let contents = reader.read_file("/.disk/info")?;
        let contents = std::str::from_utf8(&contents)?;

        let boot_modes = detect_boot_modes(&reader)?;

        let mut metadata = parse_disk_info(contents)?;
        metadata.set_path(path.to_path_buf());
        metadata.set_boot_modes(boot_modes);

        Ok(metadata)
    }
}
#[cfg(test)]
mod tests {
    use super::{
        BIOS_BOOT_PATH, DebianIsoInspector, UEFI_BOOT_PATH, detect_boot_modes, parse_disk_info,
    };
    use crate::{BootMode, InspectError, IsoReader};
    struct MockIsoReader {
        bios: bool,
        uefi: bool,
    }

    impl MockIsoReader {
        const fn new(bios: bool, uefi: bool) -> Self {
            Self { bios, uefi }
        }
    }

    impl IsoReader for MockIsoReader {
        fn read_file(&self, _iso_path: &str) -> Result<Vec<u8>, InspectError> {
            Ok(Vec::new())
        }

        fn path_exists(&self, iso_path: &str) -> Result<bool, InspectError> {
            match iso_path {
                BIOS_BOOT_PATH => Ok(self.bios),
                UEFI_BOOT_PATH => Ok(self.uefi),
                _ => Ok(false),
            }
        }
        fn list_files(&self, _iso_path: &str) -> Result<Vec<String>, InspectError> {
            Ok(Vec::new())
        }
    }
    #[test]
    fn parses_netinst_metadata() {
        let metadata =
            parse_disk_info(r#"Debian GNU/Linux 13.1.0 "Trixie" - Official amd64 NETINST"#)
                .expect("valid metadata should be parsed");

        assert_eq!(metadata.distribution(), "Debian GNU/Linux");
        assert_eq!(metadata.version(), "13.1.0");
        assert_eq!(metadata.codename(), "Trixie");
        assert_eq!(metadata.architecture(), "amd64");
        assert_eq!(metadata.media_type(), "NETINST");
        assert!(metadata.path().as_os_str().is_empty());
        assert!(metadata.boot_modes().is_empty());
    }
    #[test]
    fn creates_debian_iso_inspector() {
        let _inspector = DebianIsoInspector::new();
    }
    #[test]
    fn detects_bios_boot_mode() {
        let reader = MockIsoReader::new(true, false);

        let boot_modes = detect_boot_modes(&reader).expect("boot-mode detection should succeed");

        assert_eq!(boot_modes, vec![BootMode::Bios]);
    }

    #[test]
    fn detects_uefi_boot_mode() {
        let reader = MockIsoReader::new(false, true);

        let boot_modes = detect_boot_modes(&reader).expect("boot-mode detection should succeed");

        assert_eq!(boot_modes, vec![BootMode::Uefi]);
    }

    #[test]
    fn detects_hybrid_boot_modes() {
        let reader = MockIsoReader::new(true, true);

        let boot_modes = detect_boot_modes(&reader).expect("boot-mode detection should succeed");

        assert_eq!(boot_modes, vec![BootMode::Bios, BootMode::Uefi]);
    }
    #[test]
    fn parses_dvd_metadata() {
        let metadata =
            parse_disk_info(r#"Debian GNU/Linux 13.1.0 "Trixie" - Official amd64 DVD Binary-1"#)
                .expect("valid DVD metadata should be parsed");

        assert_eq!(metadata.distribution(), "Debian GNU/Linux");
        assert_eq!(metadata.version(), "13.1.0");
        assert_eq!(metadata.codename(), "Trixie");
        assert_eq!(metadata.architecture(), "amd64");
        assert_eq!(metadata.media_type(), "DVD");
    }

    #[test]
    fn parses_arm64_metadata() {
        let metadata =
            parse_disk_info(r#"Debian GNU/Linux 12.11.0 "Bookworm" - Official arm64 NETINST"#)
                .expect("valid arm64 metadata should be parsed");

        assert_eq!(metadata.version(), "12.11.0");
        assert_eq!(metadata.codename(), "Bookworm");
        assert_eq!(metadata.architecture(), "arm64");
    }

    #[test]
    fn rejects_missing_codename_quotes() {
        let error = parse_disk_info("Debian GNU/Linux 13.1.0 Trixie - Official amd64 NETINST")
            .expect_err("missing quotes should be rejected");

        assert!(matches!(
            error,
            InspectError::InvalidDiskInfo {
                reason: "missing opening codename quote"
            }
        ));
    }

    #[test]
    fn rejects_missing_architecture() {
        let error = parse_disk_info(r#"Debian GNU/Linux 13.1.0 "Trixie" - Official"#)
            .expect_err("missing architecture should be rejected");

        assert!(matches!(
            error,
            InspectError::InvalidDiskInfo {
                reason: "missing architecture"
            }
        ));
    }

    #[test]
    fn rejects_invalid_metadata() {
        let error = parse_disk_info("not Debian metadata").expect_err("invalid data should fail");

        assert!(matches!(
            error,
            InspectError::InvalidDiskInfo {
                reason: "missing official-media separator"
            }
        ));
    }

    #[test]
    fn parses_only_the_first_line() {
        let metadata =
            parse_disk_info("Debian GNU/Linux 13.1.0 \"Trixie\" - Official amd64 NETINST\nignored")
                .expect("first line should be parsed");

        assert_eq!(metadata.version(), "13.1.0");
    }
}
