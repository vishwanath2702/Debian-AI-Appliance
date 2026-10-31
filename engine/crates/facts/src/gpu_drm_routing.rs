//! Read-only PCI-aware DRM routing for Intel GT telemetry.

use std::fs;
use std::path::{Path, PathBuf};

/// Returns true only for DRM card names such as `card0` or `card12`.
#[must_use]
pub fn is_drm_card_name(name: &str) -> bool {
    name.strip_prefix("card")
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()))
}

/// Finds the Intel GT directory belonging to the specified PCI GPU.
///
/// The DRM card's `device` symlink must resolve to the requested PCI
/// identity and report Intel's PCI vendor ID.
///
/// # Returns
/// `None` when no matching, readable GT directory exists.
#[must_use]
pub fn find_intel_gt_directory(drm_root: &Path, pci_identifier: &str) -> Option<PathBuf> {
    if pci_identifier.is_empty() || pci_identifier.contains('/') {
        return None;
    }

    let entries = fs::read_dir(drm_root).ok()?;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if !is_drm_card_name(&name) {
            continue;
        }

        let card = entry.path();
        let device = match fs::canonicalize(card.join("device")) {
            Ok(device) => device,
            Err(_) => continue,
        };

        if device.file_name().and_then(|value| value.to_str()) != Some(pci_identifier) {
            continue;
        }

        let vendor = match fs::read_to_string(device.join("vendor")) {
            Ok(vendor) => vendor,
            Err(_) => continue,
        };

        if !vendor.trim().eq_ignore_ascii_case("0x8086") {
            continue;
        }

        let gt = card.join("gt/gt0");
        if gt.is_dir() {
            return Some(gt);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("daia-gpu-drm-{}-{sequence}", std::process::id()));
            fs::create_dir(&path).expect("create GPU routing fixture");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove GPU routing fixture");
        }
    }

    fn create_card(drm: &Path, card_name: &str, pci: &str, vendor: &str, with_gt: bool) {
        let device = drm.join("devices").join(pci);
        fs::create_dir_all(&device).unwrap();
        fs::write(device.join("vendor"), vendor).unwrap();

        let card = drm.join(card_name);
        fs::create_dir_all(&card).unwrap();
        symlink(&device, card.join("device")).unwrap();

        if with_gt {
            fs::create_dir_all(card.join("gt/gt0")).unwrap();
        }
    }

    #[test]
    fn accepts_only_numeric_drm_card_names() {
        assert!(is_drm_card_name("card0"));
        assert!(is_drm_card_name("card12"));
        for name in [
            "card",
            "card0-DP-1",
            "card0-HDMI-A-1",
            "renderD128",
            "cardX",
        ] {
            assert!(!is_drm_card_name(name));
        }
    }

    #[test]
    fn routes_to_matching_pci_gpu_not_first_card() {
        let fixture = TestDirectory::new();
        let drm = fixture.path();

        create_card(drm, "card0", "0000:01:00.0", "0x10de", true);
        create_card(drm, "card1", "0000:00:02.0", "0x8086", true);

        let gt = find_intel_gt_directory(drm, "0000:00:02.0").unwrap();
        assert_eq!(gt, drm.join("card1/gt/gt0"));
        assert_eq!(find_intel_gt_directory(drm, "0000:01:00.0"), None);
    }

    #[test]
    fn missing_gt_and_wrong_vendor_remain_unavailable() {
        let fixture = TestDirectory::new();
        let drm = fixture.path();

        create_card(drm, "card0", "0000:00:02.0", "0x8086", false);
        create_card(drm, "card1", "0000:02:00.0", "0x1002", true);

        assert_eq!(find_intel_gt_directory(drm, "0000:00:02.0"), None);
        assert_eq!(find_intel_gt_directory(drm, "0000:02:00.0"), None);
    }

    #[test]
    fn connector_names_are_not_treated_as_cards() {
        let fixture = TestDirectory::new();
        let drm = fixture.path();

        create_card(drm, "card0-DP-1", "0000:00:02.0", "0x8086", true);

        assert_eq!(find_intel_gt_directory(drm, "0000:00:02.0"), None);
    }

    #[test]
    fn missing_drm_root_is_unavailable() {
        let fixture = TestDirectory::new();
        let missing = fixture.path().join("missing");

        assert_eq!(find_intel_gt_directory(&missing, "0000:00:02.0"), None);
        assert_eq!(find_intel_gt_directory(fixture.path(), "../invalid"), None);
    }
}
