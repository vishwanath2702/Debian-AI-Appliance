//! Read-only verification of packages in a built root filesystem.

use std::{collections::HashSet, fs, io, path::Path};

pub(super) fn verify_installed_packages(rootfs: &Path, required: &[&str]) -> io::Result<()> {
    let status_path = rootfs.join("var/lib/dpkg/status");
    let contents = fs::read_to_string(&status_path)?;
    let mut installed = HashSet::new();

    let mut package = None;
    let mut status = None;

    for line in contents.lines().chain(std::iter::once("")) {
        let line = line.trim_end_matches('\r');

        if line.trim().is_empty() {
            if status == Some("install ok installed") {
                if let Some(package) = package {
                    installed.insert(package);
                }
            }

            package = None;
            status = None;
        } else if let Some(value) = line.strip_prefix("Package: ") {
            package = Some(value.trim());
        } else if let Some(value) = line.strip_prefix("Status: ") {
            status = Some(value.trim());
        }
    }

    let missing: Vec<_> = required
        .iter()
        .copied()
        .filter(|package| !installed.contains(package))
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "rootfs package verification failed; missing or not installed: {}",
            missing.join(", ")
        )))
    }
}

pub(super) fn verify_intel_8265_firmware(rootfs: &Path) -> io::Result<()> {
    for directory in ["usr/lib/firmware", "lib/firmware"] {
        let firmware_dir = rootfs.join(directory);

        if !firmware_dir.is_dir() {
            continue;
        }

        for entry in fs::read_dir(&firmware_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();

            if name.starts_with("iwlwifi-8265-")
                && name.ends_with(".ucode")
                && entry.file_type()?.is_file()
                && entry.metadata()?.len() > 0
            {
                return Ok(());
            }
        }
    }

    Err(io::Error::other(
        "rootfs firmware verification failed: missing Intel 8265 firmware (iwlwifi-8265-*.ucode)",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture(status: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "daia-rootfs-verification-{}-{unique}",
            std::process::id()
        ));
        let dpkg = root.join("var/lib/dpkg");
        fs::create_dir_all(&dpkg).expect("create fixture");
        fs::write(dpkg.join("status"), status).expect("write status");
        root
    }

    #[test]
    fn accepts_nonempty_intel_8265_firmware() {
        let root = fixture("");
        let firmware = root.join("usr/lib/firmware");
        fs::create_dir_all(&firmware).expect("firmware directory");
        fs::write(firmware.join("iwlwifi-8265-36.ucode"), b"test firmware")
            .expect("firmware fixture");

        assert!(verify_intel_8265_firmware(&root).is_ok());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_missing_or_empty_intel_8265_firmware() {
        let root = fixture("");
        let firmware = root.join("lib/firmware");
        fs::create_dir_all(&firmware).expect("firmware directory");
        fs::write(firmware.join("iwlwifi-8265-36.ucode"), b"").expect("empty firmware fixture");

        assert!(verify_intel_8265_firmware(&root).is_err());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn accepts_exact_installed_packages() {
        let root = fixture(
            "Package: firmware-iwlwifi\nStatus: install ok installed\n\n\
             Package: plasma-nm\nStatus: install ok installed\n",
        );
        assert!(verify_installed_packages(&root, &["firmware-iwlwifi", "plasma-nm"]).is_ok());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn accepts_crlf_and_whitespace_separators() {
        let root = fixture(
            "Package: firmware-iwlwifi\r\nStatus: install ok installed\r\n \r\n\
             Package: plasma-nm\r\nStatus: install ok installed\r\n",
        );

        assert!(verify_installed_packages(&root, &["firmware-iwlwifi", "plasma-nm"]).is_ok());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn does_not_mix_fields_between_package_records() {
        let root = fixture(
            "Package: firmware-iwlwifi\nStatus: deinstall ok config-files\n \n\
             Package: unrelated\nStatus: install ok installed\n",
        );

        assert!(verify_installed_packages(&root, &["firmware-iwlwifi"]).is_err());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_missing_or_uninstalled_packages() {
        let root = fixture(
            "Package: firmware-iwlwifi\nStatus: deinstall ok config-files\n\n\
             Package: plasma-nm-extra\nStatus: install ok installed\n",
        );
        let error = verify_installed_packages(&root, &["firmware-iwlwifi", "plasma-nm"])
            .expect_err("verification should fail");
        assert!(error.to_string().contains("firmware-iwlwifi"));
        assert!(error.to_string().contains("plasma-nm"));
        fs::remove_dir_all(root).expect("cleanup");
    }
}
