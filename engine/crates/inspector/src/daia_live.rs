//! Inspection of an existing DAIA live ISO for offline refresh.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::{BootMode, InspectError, IsoInspector, IsoMetadata, IsoReader, XorrisoReader};

const REQUIRED_PATHS: &[&str] = &[
    "/live/vmlinuz",
    "/live/initrd.img",
    "/live/filesystem.squashfs",
    "/boot/grub/grub.cfg",
];

/// Inspects a DAIA live ISO using release metadata from its prepared rootfs.
#[derive(Clone, Debug)]
pub struct DaiaLiveIsoInspector {
    rootfs: PathBuf,
}

impl DaiaLiveIsoInspector {
    #[must_use]
    pub fn new(rootfs: impl Into<PathBuf>) -> Self {
        Self {
            rootfs: rootfs.into(),
        }
    }

    fn inspect_with(
        &self,
        path: &Path,
        reader: &impl IsoReader,
    ) -> Result<IsoMetadata, InspectError> {
        for required in REQUIRED_PATHS {
            if !reader.path_exists(required)? {
                return Err(InspectError::Io(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("DAIA live ISO is missing {required}"),
                )));
            }
        }

        let bios = reader.path_exists("/boot/grub/i386-pc/eltorito.img")?;
        let uefi = reader.path_exists("/efi.img")?;

        let mut boot_modes = Vec::new();
        if bios {
            boot_modes.push(BootMode::Bios);
        }
        if uefi {
            boot_modes.push(BootMode::Uefi);
        }
        if boot_modes.is_empty() {
            return Err(InspectError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "DAIA live ISO has no recognized boot image",
            )));
        }

        let architecture_path = self.rootfs.join("var/lib/dpkg/arch");
        let architecture = fs::read_to_string(&architecture_path)?;
        let architecture = architecture.trim();

        if architecture != "amd64" {
            return Err(InspectError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "prepared rootfs architecture is not amd64",
            )));
        }

        let release_path = self.rootfs.join("etc/os-release");
        let release = fs::read_to_string(&release_path)?;
        let field = |key: &str| -> Option<String> {
            release.lines().find_map(|line| {
                let (name, value) = line.split_once('=')?;
                (name == key).then(|| value.trim().trim_matches('"').to_owned())
            })
        };

        let distribution = field("NAME").ok_or_else(|| {
            InspectError::Io(io::Error::new(io::ErrorKind::InvalidData, "missing NAME"))
        })?;
        let version = field("VERSION_ID").ok_or_else(|| {
            InspectError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "missing VERSION_ID",
            ))
        })?;
        let codename = field("VERSION_CODENAME").ok_or_else(|| {
            InspectError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "missing VERSION_CODENAME",
            ))
        })?;

        if field("ID").as_deref() != Some("debian") || codename != "trixie" {
            return Err(InspectError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "prepared rootfs is not Debian Trixie",
            )));
        }

        Ok(IsoMetadata::new(
            path.to_path_buf(),
            distribution,
            version,
            codename,
            architecture.to_owned(),
            "DAIA-LIVE".to_owned(),
            boot_modes,
        ))
    }
}

impl IsoInspector for DaiaLiveIsoInspector {
    fn inspect(&self, path: &Path) -> Result<IsoMetadata, InspectError> {
        self.inspect_with(path, &XorrisoReader::new(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestReader {
        missing: Option<&'static str>,
    }

    impl IsoReader for TestReader {
        fn read_file(&self, _: &str) -> Result<Vec<u8>, InspectError> {
            unreachable!()
        }

        fn path_exists(&self, path: &str) -> Result<bool, InspectError> {
            Ok(self.missing != Some(path))
        }

        fn list_files(&self, _: &str) -> Result<Vec<String>, InspectError> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn accepts_valid_live_layout() {
        let directory = tempfile::tempdir().unwrap();
        let etc = directory.path().join("etc");
        fs::create_dir(&etc).unwrap();
        fs::write(
            etc.join("os-release"),
            "NAME=\"Debian GNU/Linux\"\nID=debian\nVERSION_ID=\"13\"\nVERSION_CODENAME=trixie\n",
        )
        .unwrap();

        let dpkg = directory.path().join("var/lib/dpkg");
        fs::create_dir_all(&dpkg).unwrap();
        fs::write(dpkg.join("arch"), "amd64\n").unwrap();

        let inspector = DaiaLiveIsoInspector::new(directory.path());
        let metadata = inspector
            .inspect_with(Path::new("/source.iso"), &TestReader { missing: None })
            .unwrap();

        assert_eq!(metadata.codename(), "trixie");
        assert_eq!(metadata.media_type(), "DAIA-LIVE");
    }

    #[test]
    fn inspects_existing_daia_live_iso_when_configured() {
        let (Ok(iso), Ok(rootfs)) = (
            std::env::var("DAIA_TEST_LIVE_ISO"),
            std::env::var("DAIA_TEST_ROOTFS"),
        ) else {
            return;
        };

        let metadata = DaiaLiveIsoInspector::new(rootfs)
            .inspect(Path::new(&iso))
            .expect("existing DAIA live ISO must be inspectable");

        assert_eq!(metadata.distribution(), "Debian GNU/Linux");
        assert_eq!(metadata.version(), "13");
        assert_eq!(metadata.codename(), "trixie");
        assert_eq!(metadata.architecture(), "amd64");
        assert!(metadata.boot_modes().contains(&BootMode::Bios));
        assert!(metadata.boot_modes().contains(&BootMode::Uefi));
    }

    #[test]
    fn rejects_missing_live_kernel() {
        let directory = tempfile::tempdir().unwrap();
        let inspector = DaiaLiveIsoInspector::new(directory.path());

        assert!(
            inspector
                .inspect_with(
                    Path::new("/source.iso"),
                    &TestReader {
                        missing: Some("/live/vmlinuz")
                    },
                )
                .is_err()
        );
    }
}
