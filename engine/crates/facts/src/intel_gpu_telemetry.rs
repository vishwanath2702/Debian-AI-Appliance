//! Read-only Intel GPU GT telemetry.
//! Frequency and RC6 residency are not GPU utilization.

use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IntelGpuGtSample {
    pub current_frequency_mhz: Option<u64>,
    pub actual_frequency_mhz: Option<u64>,
    pub minimum_frequency_mhz: Option<u64>,
    pub maximum_frequency_mhz: Option<u64>,
    pub rc6_residency_ms: Option<u64>,
}

fn read_counter(directory: &Path, name: &str) -> Option<u64> {
    fs::read_to_string(directory.join(name))
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
}

#[must_use]
pub fn sample_gt(directory: &Path) -> IntelGpuGtSample {
    IntelGpuGtSample {
        current_frequency_mhz: read_counter(directory, "rps_cur_freq_mhz"),
        actual_frequency_mhz: read_counter(directory, "rps_act_freq_mhz"),
        minimum_frequency_mhz: read_counter(directory, "rps_min_freq_mhz"),
        maximum_frequency_mhz: read_counter(directory, "rps_max_freq_mhz"),
        rc6_residency_ms: read_counter(directory, "rc6_residency_ms"),
    }
}

#[must_use]
pub fn sample_default_intel_gt() -> IntelGpuGtSample {
    sample_gt(Path::new("/sys/class/drm/card0/gt/gt0"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();

        let path =
            std::env::temp_dir().join(format!("daia-intel-gt-{}-{unique}", std::process::id()));

        fs::create_dir(&path).expect("create fixture");
        path
    }

    #[test]
    fn reads_available_gt_counters() {
        let path = fixture();

        fs::write(path.join("rps_cur_freq_mhz"), "350\n").unwrap();
        fs::write(path.join("rps_act_freq_mhz"), "400\n").unwrap();
        fs::write(path.join("rps_min_freq_mhz"), "350\n").unwrap();
        fs::write(path.join("rps_max_freq_mhz"), "1100\n").unwrap();
        fs::write(path.join("rc6_residency_ms"), "4855063\n").unwrap();

        let sample = sample_gt(&path);

        assert_eq!(sample.current_frequency_mhz, Some(350));
        assert_eq!(sample.actual_frequency_mhz, Some(400));
        assert_eq!(sample.minimum_frequency_mhz, Some(350));
        assert_eq!(sample.maximum_frequency_mhz, Some(1100));
        assert_eq!(sample.rc6_residency_ms, Some(4855063));

        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn missing_metrics_remain_unavailable() {
        let path = fixture();
        fs::write(path.join("rps_cur_freq_mhz"), "350\n").unwrap();

        let sample = sample_gt(&path);

        assert_eq!(sample.current_frequency_mhz, Some(350));
        assert_eq!(sample.actual_frequency_mhz, None);
        assert_eq!(sample.rc6_residency_ms, None);

        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn malformed_metrics_remain_unavailable() {
        let path = fixture();

        fs::write(path.join("rps_cur_freq_mhz"), "invalid\n").unwrap();
        fs::write(path.join("rc6_residency_ms"), "-1\n").unwrap();

        let sample = sample_gt(&path);

        assert_eq!(sample.current_frequency_mhz, None);
        assert_eq!(sample.rc6_residency_ms, None);

        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn missing_directory_returns_unavailable_metrics() {
        let path = fixture();
        fs::remove_dir_all(&path).unwrap();

        assert_eq!(sample_gt(&path), IntelGpuGtSample::default());
    }
}
