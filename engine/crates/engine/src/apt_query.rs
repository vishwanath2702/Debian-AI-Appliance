//! Read-only APT package metadata query construction.
//!
//! This module does not install packages, update indexes, or execute commands.

use std::path::Path;

/// Builds arguments for querying package metadata under an explicit rootfs.
///
/// The caller remains responsible for validating the rootfs, sanitizing
/// the subprocess environment, and interpreting command failures.
pub(crate) fn rootfs_policy_arguments(rootfs: &Path, package: &str) -> Vec<String> {
    let root = rootfs.display();

    vec![
        "-o".into(),
        format!("Dir={root}"),
        "-o".into(),
        format!("Dir::State::status={root}/var/lib/dpkg/status"),
        "-o".into(),
        format!("Dir::State::lists={root}/var/lib/apt/lists"),
        "-o".into(),
        "Dir::Cache::pkgcache=/dev/null".into(),
        "-o".into(),
        "Dir::Cache::srcpkgcache=/dev/null".into(),
        "-o".into(),
        format!("Dir::Etc::sourcelist={root}/etc/apt/sources.list"),
        "-o".into(),
        format!("Dir::Etc::sourceparts={root}/etc/apt/sources.list.d"),
        "-o".into(),
        format!("Dir::Etc::parts={root}/etc/apt/apt.conf.d"),
        "-o".into(),
        format!("Dir::Etc::main={root}/etc/apt/apt.conf"),
        "-o".into(),
        format!("Dir::Etc::preferences={root}/etc/apt/preferences"),
        "-o".into(),
        format!("Dir::Etc::preferencesparts={root}/etc/apt/preferences.d"),
        "policy".into(),
        package.into(),
    ]
}

/// Minimal, explicit environment for a future read-only APT subprocess.
///
/// The command runner must clear its inherited environment before applying
/// these entries. This helper does not execute a subprocess.
pub(crate) fn sanitized_apt_environment() -> Vec<(&'static str, &'static str)> {
    vec![
        ("PATH", "/usr/sbin:/usr/bin:/sbin:/bin"),
        ("LC_ALL", "C"),
        ("DEBIAN_FRONTEND", "noninteractive"),
    ]
}

/// Queries a package candidate from an explicitly selected rootfs.
///
/// The result is advisory only and never authorizes package installation.
/// Failures and insufficient metadata produce `Indeterminate`.
pub(crate) fn query_rootfs_package_availability(
    rootfs: &Path,
    package: &str,
) -> crate::PackageAvailability {
    use crate::PackageAvailability;

    if !rootfs.is_absolute()
        || !crate::rootfs_has_apt_metadata(rootfs)
        || package.is_empty()
        || !package
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
    {
        return PackageAvailability::Indeterminate;
    }

    // APT configuration must not escape the selected rootfs through symlinks.
    // Missing optional configuration paths are permitted.
    for relative in [
        "etc",
        "etc/apt",
        "etc/apt/apt.conf",
        "etc/apt/apt.conf.d",
        "etc/apt/sources.list",
        "etc/apt/sources.list.d",
        "etc/apt/preferences",
        "etc/apt/preferences.d",
    ] {
        if rootfs.join(relative).is_symlink() {
            return PackageAvailability::Indeterminate;
        }
    }

    // Reject configuration entries that could redirect APT outside the rootfs.
    for relative in [
        "etc/apt/apt.conf.d",
        "etc/apt/sources.list.d",
        "etc/apt/preferences.d",
    ] {
        let directory = rootfs.join(relative);

        if !directory.exists() {
            continue;
        }

        let Ok(entries) = std::fs::read_dir(&directory) else {
            return PackageAvailability::Indeterminate;
        };

        for entry in entries {
            let Ok(entry) = entry else {
                return PackageAvailability::Indeterminate;
            };

            let Ok(metadata) = std::fs::symlink_metadata(entry.path()) else {
                return PackageAvailability::Indeterminate;
            };

            if metadata.file_type().is_symlink() {
                return PackageAvailability::Indeterminate;
            }
        }
    }

    let mut command = std::process::Command::new("/usr/bin/apt-cache");
    command.env_clear();
    command.envs(sanitized_apt_environment());
    command.args(rootfs_policy_arguments(rootfs, package));

    let Ok(output) = command.output() else {
        return PackageAvailability::Indeterminate;
    };

    if !output.status.success() {
        return PackageAvailability::Indeterminate;
    }

    let Ok(stdout) = String::from_utf8(output.stdout) else {
        return PackageAvailability::Indeterminate;
    };

    crate::parse_apt_candidate_availability(package, &stdout)
}

#[cfg(test)]
mod tests {
    use super::{
        query_rootfs_package_availability, rootfs_policy_arguments, sanitized_apt_environment,
    };
    use crate::PackageAvailability;
    use std::path::Path;

    fn snapshot_rootfs_tree(rootfs: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
        fn visit(
            rootfs: &Path,
            directory: &Path,
            entries: &mut Vec<(std::path::PathBuf, Vec<u8>)>,
        ) {
            for entry in std::fs::read_dir(directory).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                let relative = path.strip_prefix(rootfs).unwrap().to_path_buf();

                if path.is_dir() {
                    entries.push((relative, Vec::new()));
                    visit(rootfs, &path, entries);
                } else {
                    entries.push((relative, std::fs::read(&path).unwrap()));
                }
            }
        }

        let mut entries = Vec::new();
        visit(rootfs, rootfs, &mut entries);
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        entries
    }

    #[test]
    fn query_reads_synthetic_target_package_index() {
        use std::fs;

        let directory = tempfile::tempdir().unwrap();
        let rootfs = directory.path();

        for relative in [
            "etc/apt/apt.conf.d",
            "etc/apt/sources.list.d",
            "etc/apt/preferences.d",
            "var/lib/apt/lists",
            "var/lib/dpkg",
        ] {
            fs::create_dir_all(rootfs.join(relative)).unwrap();
        }

        fs::write(rootfs.join("var/lib/dpkg/status"), "").unwrap();

        fs::write(
            rootfs.join("etc/apt/sources.list"),
            "deb [trusted=yes] http://example.invalid/debian trixie main\n",
        )
        .unwrap();

        let index = rootfs.join(
            "var/lib/apt/lists/example.invalid_debian_dists_trixie_main_binary-amd64_Packages",
        );

        fs::write(
            &index,
            concat!(
                "Package: nvidia-driver\n",
                "Version: 999.0-1\n",
                "Architecture: amd64\n",
                "Maintainer: DAIA Test <test@example.invalid>\n",
                "Description: Synthetic package for isolated APT testing\n",
            ),
        )
        .unwrap();

        let before = fs::read(&index).unwrap();
        let tree_before = snapshot_rootfs_tree(rootfs);

        let result = query_rootfs_package_availability(rootfs, "nvidia-driver");

        assert_eq!(result, PackageAvailability::Available);
        assert_eq!(fs::read(&index).unwrap(), before);
        assert_eq!(snapshot_rootfs_tree(rootfs), tree_before);

        assert!(!rootfs.join("var/cache/apt/pkgcache.bin").exists());
        assert!(!rootfs.join("var/cache/apt/srcpkgcache.bin").exists());
    }

    #[test]
    fn absent_package_in_synthetic_index_is_indeterminate() {
        use std::fs;

        let directory = tempfile::tempdir().unwrap();
        let rootfs = directory.path();

        for relative in [
            "etc/apt/apt.conf.d",
            "etc/apt/sources.list.d",
            "etc/apt/preferences.d",
            "var/lib/apt/lists",
            "var/lib/dpkg",
        ] {
            fs::create_dir_all(rootfs.join(relative)).unwrap();
        }

        fs::write(rootfs.join("var/lib/dpkg/status"), "").unwrap();
        fs::write(
            rootfs.join("etc/apt/sources.list"),
            "deb [trusted=yes] http://example.invalid/debian trixie main\\n",
        )
        .unwrap();

        let index = rootfs.join(
            "var/lib/apt/lists/example.invalid_debian_dists_trixie_main_binary-amd64_Packages",
        );

        fs::write(
            &index,
            concat!(
                "Package: nvidia-driver\\n",
                "Version: 999.0-1\\n",
                "Architecture: amd64\\n",
                "Maintainer: DAIA Test <test@example.invalid>\\n",
                "Description: Synthetic package\\n",
            ),
        )
        .unwrap();

        let before = snapshot_rootfs_tree(rootfs);

        assert_eq!(
            query_rootfs_package_availability(rootfs, "daia-synthetic-absent-package"),
            PackageAvailability::Indeterminate
        );

        assert_eq!(snapshot_rootfs_tree(rootfs), before);
        assert!(!rootfs.join("var/cache/apt/pkgcache.bin").exists());
        assert!(!rootfs.join("var/cache/apt/srcpkgcache.bin").exists());
    }

    #[cfg(unix)]
    #[test]
    fn query_rejects_symlinked_apt_configuration_directory() {
        use std::fs;
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let rootfs = directory.path().join("rootfs");

        for relative in [
            "etc/apt/apt.conf.d",
            "etc/apt/sources.list.d",
            "etc/apt/preferences.d",
            "var/lib/apt/lists",
            "var/lib/dpkg",
        ] {
            fs::create_dir_all(rootfs.join(relative)).unwrap();
        }

        fs::write(rootfs.join("var/lib/dpkg/status"), "").unwrap();
        fs::write(
            rootfs.join("etc/apt/sources.list"),
            "deb [trusted=yes] http://example.invalid/debian trixie main\n",
        )
        .unwrap();

        fs::write(
            rootfs.join(
                "var/lib/apt/lists/example.invalid_debian_dists_trixie_main_binary-amd64_Packages",
            ),
            concat!(
                "Package: nvidia-driver\n",
                "Version: 999.0-1\n",
                "Architecture: amd64\n",
                "Maintainer: DAIA Test <test@example.invalid>\n",
                "Description: Synthetic package\n",
            ),
        )
        .unwrap();

        assert_eq!(
            super::query_rootfs_package_availability(&rootfs, "nvidia-driver"),
            crate::PackageAvailability::Available,
            "Control fixture must establish an available synthetic candidate"
        );

        fs::remove_dir(rootfs.join("etc/apt/apt.conf.d")).unwrap();

        let outside = directory.path().join("outside-config");
        fs::create_dir_all(&outside).unwrap();
        fs::write(
            outside.join("99-daia-test"),
            r#"Dir::State::lists "/var/lib/apt/lists";"#,
        )
        .unwrap();

        symlink(&outside, rootfs.join("etc/apt/apt.conf.d")).unwrap();

        assert_eq!(
            super::query_rootfs_package_availability(&rootfs, "nvidia-driver"),
            crate::PackageAvailability::Indeterminate,
            "APT query must reject symlinked configuration directories"
        );
    }

    #[cfg(unix)]
    #[test]
    fn query_rejects_symlinked_apt_configuration_file() {
        use std::fs;
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let rootfs = directory.path().join("rootfs");

        for relative in [
            "etc/apt/apt.conf.d",
            "etc/apt/sources.list.d",
            "etc/apt/preferences.d",
            "var/lib/apt/lists",
            "var/lib/dpkg",
        ] {
            fs::create_dir_all(rootfs.join(relative)).unwrap();
        }

        fs::write(rootfs.join("var/lib/dpkg/status"), "").unwrap();
        fs::write(
            rootfs.join("etc/apt/sources.list"),
            "deb [trusted=yes] http://example.invalid/debian trixie main\n",
        )
        .unwrap();
        fs::write(
            rootfs.join(
                "var/lib/apt/lists/example.invalid_debian_dists_trixie_main_binary-amd64_Packages",
            ),
            concat!(
                "Package: nvidia-driver\n",
                "Version: 999.0-1\n",
                "Architecture: amd64\n",
                "Maintainer: DAIA Test <test@example.invalid>\n",
                "Description: Synthetic package\n",
            ),
        )
        .unwrap();

        assert_eq!(
            super::query_rootfs_package_availability(&rootfs, "nvidia-driver"),
            crate::PackageAvailability::Available,
            "Control fixture must establish an available candidate"
        );

        let outside = directory.path().join("outside-apt.conf");
        fs::write(&outside, "APT::Get::Assume-Yes \"false\";\n").unwrap();

        symlink(&outside, rootfs.join("etc/apt/apt.conf.d/99-daia-test")).unwrap();

        assert_eq!(
            super::query_rootfs_package_availability(&rootfs, "nvidia-driver"),
            crate::PackageAvailability::Indeterminate,
            "APT query must reject symlinked configuration files"
        );
    }

    #[test]
    fn query_rejects_missing_rootfs() {
        let result = query_rootfs_package_availability(
            Path::new("/nonexistent-daia-apt-query-rootfs"),
            "nvidia-driver",
        );

        assert_eq!(result, PackageAvailability::Indeterminate);
    }

    #[test]
    fn query_rejects_relative_rootfs() {
        let result =
            query_rootfs_package_availability(Path::new("relative-daia-rootfs"), "nvidia-driver");

        assert_eq!(result, PackageAvailability::Indeterminate);
    }

    #[test]
    fn query_rejects_invalid_package_name() {
        let result = query_rootfs_package_availability(
            Path::new("/nonexistent-daia-apt-query-rootfs"),
            "nvidia-driver;apt-get install",
        );

        assert_eq!(result, PackageAvailability::Indeterminate);
    }

    #[test]
    fn apt_environment_contains_only_explicit_safe_values() {
        assert_eq!(
            sanitized_apt_environment(),
            vec![
                ("PATH", "/usr/sbin:/usr/bin:/sbin:/bin"),
                ("LC_ALL", "C"),
                ("DEBIAN_FRONTEND", "noninteractive"),
            ]
        );
    }

    #[test]
    fn apt_environment_excludes_inherited_configuration_variables() {
        let environment = sanitized_apt_environment();
        let names: Vec<&str> = environment.iter().map(|(name, _)| *name).collect();

        for forbidden in [
            "APT_CONFIG",
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "PYTHONPATH",
            "HOME",
        ] {
            assert!(!names.contains(&forbidden));
        }
    }

    #[test]
    fn rootfs_query_has_exact_argument_contract() {
        let args = rootfs_policy_arguments(Path::new("/tmp/daia-test-rootfs"), "nvidia-driver");

        assert_eq!(
            args,
            vec![
                "-o",
                "Dir=/tmp/daia-test-rootfs",
                "-o",
                "Dir::State::status=/tmp/daia-test-rootfs/var/lib/dpkg/status",
                "-o",
                "Dir::State::lists=/tmp/daia-test-rootfs/var/lib/apt/lists",
                "-o",
                "Dir::Cache::pkgcache=/dev/null",
                "-o",
                "Dir::Cache::srcpkgcache=/dev/null",
                "-o",
                "Dir::Etc::sourcelist=/tmp/daia-test-rootfs/etc/apt/sources.list",
                "-o",
                "Dir::Etc::sourceparts=/tmp/daia-test-rootfs/etc/apt/sources.list.d",
                "-o",
                "Dir::Etc::parts=/tmp/daia-test-rootfs/etc/apt/apt.conf.d",
                "-o",
                "Dir::Etc::main=/tmp/daia-test-rootfs/etc/apt/apt.conf",
                "-o",
                "Dir::Etc::preferences=/tmp/daia-test-rootfs/etc/apt/preferences",
                "-o",
                "Dir::Etc::preferencesparts=/tmp/daia-test-rootfs/etc/apt/preferences.d",
                "policy",
                "nvidia-driver",
            ]
        );
    }

    #[test]
    fn rootfs_query_disables_cache_files() {
        let args = rootfs_policy_arguments(Path::new("/tmp/daia-test-rootfs"), "nvidia-driver");

        assert!(args.contains(&"Dir::Cache::pkgcache=/dev/null".to_owned()));
        assert!(args.contains(&"Dir::Cache::srcpkgcache=/dev/null".to_owned()));
        assert_eq!(args.last().map(String::as_str), Some("nvidia-driver"));
    }

    #[test]
    fn rootfs_query_uses_explicit_target_paths() {
        let args = rootfs_policy_arguments(Path::new("/tmp/daia-test-rootfs"), "nvidia-driver");

        assert!(
            args.contains(
                &"Dir::State::status=/tmp/daia-test-rootfs/var/lib/dpkg/status".to_owned()
            )
        );
        assert!(
            args.contains(&"Dir::State::lists=/tmp/daia-test-rootfs/var/lib/apt/lists".to_owned())
        );
    }
}
