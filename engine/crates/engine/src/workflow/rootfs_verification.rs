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

pub(super) fn verify_grub_modules(rootfs: &Path) -> io::Result<()> {
    for (target, modules) in [
        (
            "i386-pc",
            &[
                "biosdisk.mod",
                "ext2.mod",
                "linux.mod",
                "normal.mod",
                "part_gpt.mod",
            ][..],
        ),
        (
            "x86_64-efi",
            &[
                "efi_gop.mod",
                "ext2.mod",
                "linux.mod",
                "normal.mod",
                "part_gpt.mod",
            ][..],
        ),
    ] {
        for module in modules {
            let path = rootfs.join("usr/lib/grub").join(target).join(module);
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                io::Error::other(format!(
                    "rootfs GRUB module verification failed: {}: {error}",
                    path.display()
                ))
            })?;

            if !metadata.file_type().is_file() || metadata.len() == 0 {
                return Err(io::Error::other(format!(
                    "rootfs GRUB module verification failed: {} is not a nonempty regular file",
                    path.display()
                )));
            }
        }
    }

    Ok(())
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

/// Verifies that NVIDIA compatibility databases are present in the live rootfs.
///
/// These are detection data only; no NVIDIA driver is installed here.
pub(super) fn verify_nvidia_pci_id_databases(rootfs: &Path) -> io::Result<()> {
    for name in [
        "nvidia.ids",
        "nvidia-tesla-535.ids",
        "nvidia-legacy-390xx.ids",
    ] {
        let path = rootfs.join("usr/share/nvidia").join(name);

        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            io::Error::other(format!(
                "rootfs NVIDIA database verification failed: {}: {error}",
                path.display()
            ))
        })?;

        if !metadata.file_type().is_file() || metadata.len() == 0 {
            return Err(io::Error::other(format!(
                "rootfs NVIDIA database verification failed: {} is not a nonempty regular file",
                path.display()
            )));
        }
    }

    Ok(())
}

pub(super) fn verify_setpriv(rootfs: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let path = rootfs.join("usr/bin/setpriv");

    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        io::Error::other(format!(
            "rootfs Ollama verification failed: /usr/bin/setpriv is unavailable: {error}"
        ))
    })?;

    if !metadata.file_type().is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(io::Error::other(
            "rootfs Ollama verification failed: /usr/bin/setpriv is not an executable regular file",
        ));
    }

    Ok(())
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

    fn create_nvidia_databases(root: &Path) {
        let directory = root.join("usr/share/nvidia");
        fs::create_dir_all(&directory).expect("create NVIDIA database directory");

        for name in [
            "nvidia.ids",
            "nvidia-tesla-535.ids",
            "nvidia-legacy-390xx.ids",
        ] {
            fs::write(directory.join(name), b"10DE1340\\n").expect("write NVIDIA database fixture");
        }
    }

    #[test]
    fn accepts_nonempty_nvidia_databases() {
        let root = fixture("");
        create_nvidia_databases(&root);

        assert!(verify_nvidia_pci_id_databases(&root).is_ok());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_missing_nvidia_database() {
        let root = fixture("");
        create_nvidia_databases(&root);

        fs::remove_file(root.join("usr/share/nvidia/nvidia-tesla-535.ids"))
            .expect("remove database fixture");

        let error = verify_nvidia_pci_id_databases(&root).expect_err("missing database must fail");

        assert!(error.to_string().contains("nvidia-tesla-535.ids"));

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_empty_nvidia_database() {
        let root = fixture("");
        create_nvidia_databases(&root);

        fs::write(root.join("usr/share/nvidia/nvidia.ids"), b"").expect("empty database fixture");

        let error = verify_nvidia_pci_id_databases(&root).expect_err("empty database must fail");

        assert!(error.to_string().contains("nvidia.ids"));

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_symlinked_nvidia_database() {
        let root = fixture("");
        create_nvidia_databases(&root);

        let path = root.join("usr/share/nvidia/nvidia-legacy-390xx.ids");
        fs::remove_file(&path).expect("remove database fixture");
        std::os::unix::fs::symlink("nvidia.ids", &path).expect("create database symlink");

        let error =
            verify_nvidia_pci_id_databases(&root).expect_err("symlinked database must fail");

        assert!(error.to_string().contains("nvidia-legacy-390xx.ids"));

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn accepts_executable_setpriv() {
        use std::os::unix::fs::PermissionsExt;

        let root = fixture("");
        let binary = root.join("usr/bin/setpriv");

        fs::create_dir_all(binary.parent().unwrap()).unwrap();
        fs::write(&binary, b"test executable").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();

        assert!(verify_setpriv(&root).is_ok());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_missing_setpriv() {
        let root = fixture("");

        let error = verify_setpriv(&root).expect_err("missing setpriv must fail");
        assert!(error.to_string().contains("/usr/bin/setpriv"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_nonexecutable_setpriv() {
        use std::os::unix::fs::PermissionsExt;

        let root = fixture("");
        let binary = root.join("usr/bin/setpriv");

        fs::create_dir_all(binary.parent().unwrap()).unwrap();
        fs::write(&binary, b"not executable").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o644)).unwrap();

        assert!(verify_setpriv(&root).is_err());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_setpriv_symlink() {
        let root = fixture("");
        let binary = root.join("usr/bin/setpriv");

        fs::create_dir_all(binary.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink("/usr/bin/true", &binary).unwrap();

        assert!(verify_setpriv(&root).is_err());

        fs::remove_dir_all(root).unwrap();
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

    fn create_grub_module_fixtures(root: &Path) {
        for (target, modules) in [
            (
                "i386-pc",
                &[
                    "biosdisk.mod",
                    "ext2.mod",
                    "linux.mod",
                    "normal.mod",
                    "part_gpt.mod",
                ][..],
            ),
            (
                "x86_64-efi",
                &[
                    "efi_gop.mod",
                    "ext2.mod",
                    "linux.mod",
                    "normal.mod",
                    "part_gpt.mod",
                ][..],
            ),
        ] {
            let directory = root.join("usr/lib/grub").join(target);
            fs::create_dir_all(&directory).expect("create GRUB directory");

            for module in modules {
                fs::write(directory.join(module), b"test module")
                    .expect("write GRUB module fixture");
            }
        }
    }

    #[test]
    fn accepts_bios_and_uefi_grub_modules() {
        let root = fixture("");
        create_grub_module_fixtures(&root);

        assert!(verify_grub_modules(&root).is_ok());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_missing_bios_grub_module() {
        let root = fixture("");
        create_grub_module_fixtures(&root);
        fs::remove_file(root.join("usr/lib/grub/i386-pc/biosdisk.mod"))
            .expect("remove BIOS module");

        let error = verify_grub_modules(&root).expect_err("missing BIOS module must fail");
        assert!(error.to_string().contains("biosdisk.mod"));

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_empty_uefi_grub_module() {
        let root = fixture("");
        create_grub_module_fixtures(&root);
        fs::write(root.join("usr/lib/grub/x86_64-efi/normal.mod"), b"").expect("empty UEFI module");

        let error = verify_grub_modules(&root).expect_err("empty UEFI module must fail");
        assert!(error.to_string().contains("normal.mod"));

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_symlinked_grub_module() {
        let root = fixture("");
        create_grub_module_fixtures(&root);
        let path = root.join("usr/lib/grub/i386-pc/normal.mod");
        fs::remove_file(&path).expect("remove module");
        std::os::unix::fs::symlink("linux.mod", &path).expect("create module symlink");

        let error = verify_grub_modules(&root).expect_err("symlink must fail");
        assert!(error.to_string().contains("normal.mod"));

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
