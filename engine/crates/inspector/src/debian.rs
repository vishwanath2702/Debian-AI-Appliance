// engine/crates/inspector/src/debian.rs

//! Parsing of Debian installation-media metadata.

use std::path::PathBuf;

use crate::{InspectError, IsoMetadata};

const OFFICIAL_SEPARATOR: &str = " - Official";

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

#[cfg(test)]
mod tests {
    use super::parse_disk_info;
    use crate::InspectError;

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
