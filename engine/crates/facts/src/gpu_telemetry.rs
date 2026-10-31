//! Vendor-neutral GPU telemetry observations.
//!
//! Missing metrics are represented by None, never by fabricated zeroes.

use crate::GpuFacts;
use crate::gpu_drm_routing::find_intel_gt_directory;
use crate::intel_gpu_telemetry::{IntelGpuGtSample, sample_gt};
use crate::nvidia_gpu_telemetry::{NvidiaTelemetryBackend, sample_nvidia_gpu};
use crate::nvidia_smi_backend::NvidiaSmiCommandRunner;
use crate::nvidia_smi_telemetry::{normalize_pci_identifier, parse_nvidia_smi_output};
use std::path::Path;

/// Known PCI GPU vendors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpuVendor {
    Intel,
    Nvidia,
    Amd,
    Other,
}

impl GpuVendor {
    #[must_use]
    pub fn from_pci_vendor_id(vendor_id: &str) -> Self {
        match vendor_id.to_ascii_lowercase().as_str() {
            "0x8086" => Self::Intel,
            "0x10de" => Self::Nvidia,
            "0x1002" => Self::Amd,
            _ => Self::Other,
        }
    }
}

/// One GPU's optional live measurements.
///
/// Utilization percentages range from 0 to 100 when available.
/// Memory measurements are expressed in bytes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuMetrics {
    pub utilization_percent: Option<f64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub temperature_celsius: Option<f64>,
    pub frequency_mhz: Option<u64>,
    pub power_watts: Option<f64>,
}

/// One identified GPU and its available measurements.
#[derive(Clone, Debug, PartialEq)]
pub struct GpuTelemetrySample {
    pub gpu: GpuFacts,
    pub vendor: GpuVendor,
    pub metrics: GpuMetrics,
}

impl GpuTelemetrySample {
    #[must_use]
    pub fn new(gpu: GpuFacts, metrics: GpuMetrics) -> Self {
        let vendor = GpuVendor::from_pci_vendor_id(gpu.vendor_id());
        Self {
            gpu,
            vendor,
            metrics,
        }
    }
}

/// Converts Intel GT counters into vendor-neutral measurements.
///
/// Frequency is a clock measurement, not GPU utilization.
#[must_use]
pub fn intel_gt_metrics(sample: &IntelGpuGtSample) -> GpuMetrics {
    GpuMetrics {
        frequency_mhz: sample.actual_frequency_mhz.or(sample.current_frequency_mhz),
        ..GpuMetrics::default()
    }
}

/// Associates Intel GT counters with an explicitly identified Intel GPU.
///
/// Other vendors are never assigned Intel-specific telemetry.
#[must_use]
pub fn intel_gt_observation(gpu: GpuFacts, sample: &IntelGpuGtSample) -> GpuTelemetrySample {
    let vendor = GpuVendor::from_pci_vendor_id(gpu.vendor_id());

    let metrics = if vendor == GpuVendor::Intel {
        intel_gt_metrics(sample)
    } else {
        GpuMetrics::default()
    };

    GpuTelemetrySample::new(gpu, metrics)
}

/// Collects available Intel GT measurements for explicitly identified GPUs.
///
/// Each Intel device is resolved through its PCI identity. NVIDIA, AMD,
/// and other devices are retained with unavailable metrics until their
/// respective collectors are implemented.
#[must_use]
pub fn sample_discovered_gpus(gpus: Vec<GpuFacts>, drm_root: &Path) -> GpuTelemetrySnapshot {
    let devices = gpus
        .into_iter()
        .map(|gpu| {
            if GpuVendor::from_pci_vendor_id(gpu.vendor_id()) == GpuVendor::Intel {
                if let Some(gt_directory) = find_intel_gt_directory(drm_root, gpu.identifier()) {
                    return intel_gt_observation(gpu, &sample_gt(&gt_directory));
                }
            }

            GpuTelemetrySample::new(gpu, GpuMetrics::default())
        })
        .collect();

    GpuTelemetrySnapshot { devices }
}

/// Collects Intel GPU telemetry and injectable NVIDIA observations.
///
/// Intel devices use PCI-aware DRM routing. NVIDIA devices are queried
/// through the supplied backend using their discovered PCI identities.
/// AMD and other devices remain identified with unavailable metrics.
///
/// NVIDIA backend errors do not interrupt observations of other GPUs.
#[must_use]
pub fn sample_discovered_gpus_with_nvidia<B: NvidiaTelemetryBackend>(
    gpus: Vec<GpuFacts>,
    drm_root: &Path,
    nvidia_backend: &mut B,
) -> GpuTelemetrySnapshot {
    let devices = gpus
        .into_iter()
        .map(|gpu| match GpuVendor::from_pci_vendor_id(gpu.vendor_id()) {
            GpuVendor::Intel => {
                if let Some(gt_directory) = find_intel_gt_directory(drm_root, gpu.identifier()) {
                    intel_gt_observation(gpu, &sample_gt(&gt_directory))
                } else {
                    GpuTelemetrySample::new(gpu, GpuMetrics::default())
                }
            }
            GpuVendor::Nvidia => sample_nvidia_gpu(gpu, nvidia_backend),
            GpuVendor::Amd | GpuVendor::Other => {
                GpuTelemetrySample::new(gpu, GpuMetrics::default())
            }
        })
        .collect();

    GpuTelemetrySnapshot { devices }
}

/// Collects all discovered GPU telemetry with one NVIDIA SMI query.
///
/// NVIDIA measurements are matched by normalized PCI identity. One command
/// is executed per snapshot, regardless of the number of NVIDIA GPUs.
/// Intel uses its existing PCI-aware DRM collector. AMD and unsupported
/// devices retain unavailable metrics.
///
/// Failed NVIDIA queries do not prevent CPU/RAM or Intel observations.
#[must_use]
pub fn sample_discovered_gpus_with_nvidia_snapshot<R: NvidiaSmiCommandRunner>(
    gpus: Vec<GpuFacts>,
    drm_root: &Path,
    runner: &mut R,
) -> GpuTelemetrySnapshot {
    let has_nvidia = gpus
        .iter()
        .any(|gpu| GpuVendor::from_pci_vendor_id(gpu.vendor_id()) == GpuVendor::Nvidia);

    let nvidia_observations = if has_nvidia {
        runner
            .query()
            .ok()
            .map(|output| parse_nvidia_smi_output(&output))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let devices = gpus
        .into_iter()
        .map(|gpu| match GpuVendor::from_pci_vendor_id(gpu.vendor_id()) {
            GpuVendor::Intel => {
                if let Some(gt_directory) = find_intel_gt_directory(drm_root, gpu.identifier()) {
                    intel_gt_observation(gpu, &sample_gt(&gt_directory))
                } else {
                    GpuTelemetrySample::new(gpu, GpuMetrics::default())
                }
            }
            GpuVendor::Nvidia => {
                let metrics = normalize_pci_identifier(gpu.identifier())
                    .and_then(|identifier| {
                        nvidia_observations
                            .iter()
                            .find(|observation| observation.pci_identifier == identifier)
                    })
                    .map(|observation| {
                        crate::nvidia_gpu_telemetry::nvidia_metrics(observation.metrics.clone())
                    })
                    .unwrap_or_default();

                GpuTelemetrySample::new(gpu, metrics)
            }
            GpuVendor::Amd | GpuVendor::Other => {
                GpuTelemetrySample::new(gpu, GpuMetrics::default())
            }
        })
        .collect();

    GpuTelemetrySnapshot { devices }
}

/// A complete GPU observation supporting zero or more devices.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuTelemetrySnapshot {
    pub devices: Vec<GpuTelemetrySample>,
}

impl GpuTelemetrySnapshot {
    #[must_use]
    pub fn from_discovered_gpus(gpus: Vec<GpuFacts>) -> Self {
        Self {
            devices: gpus
                .into_iter()
                .map(|gpu| GpuTelemetrySample::new(gpu, GpuMetrics::default()))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_collection_preserves_unavailable_gpu_metrics() {
        let fixture =
            std::env::temp_dir().join(format!("daia-gpu-telemetry-missing-{}", std::process::id()));
        // The fixture path need not exist: missing DRM is a supported state.
        let missing = fixture.join("definitely-missing-drm");

        let snapshot = sample_discovered_gpus(
            vec![
                GpuFacts::new("0000:00:02.0", "0x8086", "0x5912"),
                GpuFacts::new("0000:01:00.0", "0x10de", "0x1fb2"),
                GpuFacts::new("0000:02:00.0", "0x1002", "0x1234"),
            ],
            &missing,
        );

        assert_eq!(snapshot.devices.len(), 3);
        assert_eq!(snapshot.devices[0].vendor, GpuVendor::Intel);
        assert_eq!(snapshot.devices[1].vendor, GpuVendor::Nvidia);
        assert_eq!(snapshot.devices[2].vendor, GpuVendor::Amd);
        assert!(
            snapshot
                .devices
                .iter()
                .all(|device| device.metrics == GpuMetrics::default())
        );
    }

    #[test]
    fn automatic_collection_does_not_reassign_intel_counters() {
        use std::os::unix::fs::symlink;
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(0);
        let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
        let fixture = std::env::temp_dir().join(format!(
            "daia-gpu-telemetry-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&fixture).unwrap();

        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).unwrap();
            }
        }
        let _cleanup = Cleanup(fixture.clone());

        let pci = fixture.join("devices/0000:00:02.0");
        let intel_card = fixture.join("card2");
        std::fs::create_dir_all(&pci).unwrap();
        std::fs::create_dir_all(intel_card.join("gt/gt0")).unwrap();
        std::fs::write(pci.join("vendor"), "0x8086\n").unwrap();
        symlink(&pci, intel_card.join("device")).unwrap();
        std::fs::write(intel_card.join("gt/gt0/rps_act_freq_mhz"), "625\n").unwrap();

        let snapshot = sample_discovered_gpus(
            vec![
                GpuFacts::new("0000:01:00.0", "0x10de", "0x1fb2"),
                GpuFacts::new("0000:00:02.0", "0x8086", "0x5912"),
                GpuFacts::new("0000:02:00.0", "0x8086", "0x1234"),
            ],
            &fixture,
        );

        assert_eq!(snapshot.devices.len(), 3);
        assert_eq!(snapshot.devices[0].metrics.frequency_mhz, None);
        assert_eq!(snapshot.devices[1].metrics.frequency_mhz, Some(625));
        assert_eq!(snapshot.devices[2].metrics.frequency_mhz, None);
        assert_eq!(snapshot.devices[1].metrics.utilization_percent, None);
    }

    #[test]
    fn recognizes_known_gpu_vendors() {
        assert_eq!(GpuVendor::from_pci_vendor_id("0x8086"), GpuVendor::Intel);
        assert_eq!(GpuVendor::from_pci_vendor_id("0X10DE"), GpuVendor::Nvidia);
        assert_eq!(GpuVendor::from_pci_vendor_id("0x1002"), GpuVendor::Amd);
        assert_eq!(GpuVendor::from_pci_vendor_id("0x1234"), GpuVendor::Other);
    }

    #[test]
    fn missing_metrics_are_unavailable_not_zero() {
        let metrics = GpuMetrics::default();
        assert_eq!(metrics.utilization_percent, None);
        assert_eq!(metrics.memory_used_bytes, None);
        assert_eq!(metrics.temperature_celsius, None);
        assert_eq!(metrics.power_watts, None);
    }

    #[test]
    fn preserves_multiple_gpu_identities_and_order() {
        let intel = GpuFacts::new("0000:00:02.0", "0x8086", "0x5912");
        let nvidia = GpuFacts::new("0000:01:00.0", "0x10de", "0x1fb2");

        let snapshot = GpuTelemetrySnapshot::from_discovered_gpus(vec![intel, nvidia]);

        assert_eq!(snapshot.devices.len(), 2);
        assert_eq!(snapshot.devices[0].vendor, GpuVendor::Intel);
        assert_eq!(snapshot.devices[1].vendor, GpuVendor::Nvidia);
        assert_eq!(snapshot.devices[0].gpu.identifier(), "0000:00:02.0");
        assert_eq!(snapshot.devices[1].gpu.identifier(), "0000:01:00.0");
    }

    #[test]
    fn intel_gt_frequency_maps_without_inventing_utilization() {
        let gpu = GpuFacts::new("0000:00:02.0", "0x8086", "0x5912");
        let sample = IntelGpuGtSample {
            actual_frequency_mhz: Some(500),
            current_frequency_mhz: Some(350),
            rc6_residency_ms: Some(1000),
            ..IntelGpuGtSample::default()
        };

        let observation = intel_gt_observation(gpu, &sample);

        assert_eq!(observation.vendor, GpuVendor::Intel);
        assert_eq!(observation.metrics.frequency_mhz, Some(500));
        assert_eq!(observation.metrics.utilization_percent, None);
        assert_eq!(observation.metrics.power_watts, None);
    }

    #[test]
    fn intel_gt_falls_back_to_current_frequency() {
        let sample = IntelGpuGtSample {
            current_frequency_mhz: Some(350),
            ..IntelGpuGtSample::default()
        };

        assert_eq!(intel_gt_metrics(&sample).frequency_mhz, Some(350));
    }

    #[test]
    fn intel_counters_are_not_assigned_to_other_vendors() {
        let sample = IntelGpuGtSample {
            actual_frequency_mhz: Some(750),
            ..IntelGpuGtSample::default()
        };

        for vendor in ["0x10de", "0x1002", "0x1234"] {
            let gpu = GpuFacts::new("0000:01:00.0", vendor, "0x0001");
            let observation = intel_gt_observation(gpu, &sample);

            assert_eq!(observation.metrics, GpuMetrics::default());
        }
    }

    #[test]
    fn empty_gpu_list_produces_empty_snapshot() {
        let snapshot = GpuTelemetrySnapshot::from_discovered_gpus(vec![]);
        assert!(snapshot.devices.is_empty());
    }
}

#[cfg(test)]
mod nvidia_multi_gpu_integration_tests {
    use super::*;
    use crate::nvidia_gpu_telemetry::NvidiaGpuMetrics;

    struct FakeNvidia {
        requests: Vec<String>,
        fail: bool,
    }

    impl NvidiaTelemetryBackend for FakeNvidia {
        type Error = &'static str;

        fn sample(&mut self, pci_identifier: &str) -> Result<NvidiaGpuMetrics, Self::Error> {
            self.requests.push(pci_identifier.to_owned());

            if self.fail {
                Err("backend unavailable")
            } else {
                Ok(NvidiaGpuMetrics {
                    utilization_percent: Some(55.0),
                    memory_used_bytes: Some(2048),
                    memory_total_bytes: Some(4096),
                    ..NvidiaGpuMetrics::default()
                })
            }
        }
    }

    fn devices() -> Vec<GpuFacts> {
        vec![
            GpuFacts::new("0000:00:02.0", "0x8086", "0x5912"),
            GpuFacts::new("0000:01:00.0", "0x10de", "0x1fb2"),
            GpuFacts::new("0000:02:00.0", "0x1002", "0x1234"),
            GpuFacts::new("0000:03:00.0", "0x10de", "0x2684"),
        ]
    }

    #[test]
    fn routes_only_nvidia_devices_to_backend() {
        let mut backend = FakeNvidia {
            requests: Vec::new(),
            fail: false,
        };

        let snapshot = sample_discovered_gpus_with_nvidia(
            devices(),
            Path::new("/sys/daia-missing-drm"),
            &mut backend,
        );

        assert_eq!(snapshot.devices.len(), 4);
        assert_eq!(backend.requests, ["0000:01:00.0", "0000:03:00.0"]);

        assert_eq!(snapshot.devices[0].vendor, GpuVendor::Intel);
        assert_eq!(snapshot.devices[1].vendor, GpuVendor::Nvidia);
        assert_eq!(snapshot.devices[2].vendor, GpuVendor::Amd);
        assert_eq!(snapshot.devices[3].vendor, GpuVendor::Nvidia);

        assert_eq!(snapshot.devices[1].metrics.utilization_percent, Some(55.0));
        assert_eq!(snapshot.devices[3].metrics.memory_used_bytes, Some(2048));
        assert_eq!(snapshot.devices[0].metrics, GpuMetrics::default());
        assert_eq!(snapshot.devices[2].metrics, GpuMetrics::default());
    }

    #[test]
    fn nvidia_failure_preserves_all_gpu_identities() {
        let mut backend = FakeNvidia {
            requests: Vec::new(),
            fail: true,
        };

        let snapshot = sample_discovered_gpus_with_nvidia(
            devices(),
            Path::new("/sys/daia-missing-drm"),
            &mut backend,
        );

        assert_eq!(snapshot.devices.len(), 4);
        assert_eq!(snapshot.devices[1].gpu.identifier(), "0000:01:00.0");
        assert_eq!(snapshot.devices[3].gpu.identifier(), "0000:03:00.0");
        assert_eq!(snapshot.devices[1].metrics, GpuMetrics::default());
        assert_eq!(snapshot.devices[3].metrics, GpuMetrics::default());
    }

    #[test]
    fn no_nvidia_devices_never_calls_backend() {
        let mut backend = FakeNvidia {
            requests: Vec::new(),
            fail: false,
        };

        let snapshot = sample_discovered_gpus_with_nvidia(
            vec![
                GpuFacts::new("0000:00:02.0", "0x8086", "0x5912"),
                GpuFacts::new("0000:02:00.0", "0x1002", "0x1234"),
            ],
            Path::new("/sys/daia-missing-drm"),
            &mut backend,
        );

        assert_eq!(snapshot.devices.len(), 2);
        assert!(backend.requests.is_empty());
    }
}

#[cfg(test)]
mod nvidia_single_query_snapshot_tests {
    use super::*;
    use std::io;

    struct FakeRunner {
        output: io::Result<String>,
        calls: usize,
    }

    impl NvidiaSmiCommandRunner for FakeRunner {
        fn query(&mut self) -> io::Result<String> {
            self.calls += 1;
            match &self.output {
                Ok(value) => Ok(value.clone()),
                Err(error) => Err(io::Error::new(error.kind(), error.to_string())),
            }
        }
    }

    fn devices() -> Vec<GpuFacts> {
        vec![
            GpuFacts::new("0000:00:02.0", "0x8086", "0x5912"),
            GpuFacts::new("0000:01:00.0", "0x10de", "0x1fb2"),
            GpuFacts::new("0000:02:00.0", "0x1002", "0x1234"),
            GpuFacts::new("0000:03:00.0", "0x10de", "0x2684"),
        ]
    }

    #[test]
    fn two_nvidia_gpus_use_one_query_and_preserve_identity() {
        let mut runner = FakeRunner {
            output: Ok(concat!(
                "00000000:03:00.0, 75, 300, 8000, 65, 1500, 60\n",
                "00000000:01:00.0, 25, 100, 4000, 50, 900, 20\n"
            )
            .to_owned()),
            calls: 0,
        };

        let snapshot = sample_discovered_gpus_with_nvidia_snapshot(
            devices(),
            Path::new("/sys/daia-missing-drm"),
            &mut runner,
        );

        assert_eq!(runner.calls, 1);
        assert_eq!(snapshot.devices.len(), 4);
        assert_eq!(snapshot.devices[0].vendor, GpuVendor::Intel);
        assert_eq!(snapshot.devices[1].metrics.utilization_percent, Some(25.0));
        assert_eq!(snapshot.devices[2].vendor, GpuVendor::Amd);
        assert_eq!(snapshot.devices[3].metrics.utilization_percent, Some(75.0));
        assert_eq!(snapshot.devices[1].gpu.identifier(), "0000:01:00.0");
        assert_eq!(snapshot.devices[3].gpu.identifier(), "0000:03:00.0");
    }

    #[test]
    fn no_nvidia_gpu_skips_command() {
        let mut runner = FakeRunner {
            output: Ok(String::new()),
            calls: 0,
        };

        let snapshot = sample_discovered_gpus_with_nvidia_snapshot(
            vec![GpuFacts::new("0000:00:02.0", "0x8086", "0x5912")],
            Path::new("/sys/daia-missing-drm"),
            &mut runner,
        );

        assert_eq!(runner.calls, 0);
        assert_eq!(snapshot.devices.len(), 1);
    }

    #[test]
    fn failed_query_preserves_all_gpu_identities() {
        let mut runner = FakeRunner {
            output: Err(io::Error::new(
                io::ErrorKind::NotFound,
                "missing nvidia-smi",
            )),
            calls: 0,
        };

        let snapshot = sample_discovered_gpus_with_nvidia_snapshot(
            devices(),
            Path::new("/sys/daia-missing-drm"),
            &mut runner,
        );

        assert_eq!(runner.calls, 1);
        assert_eq!(snapshot.devices.len(), 4);
        assert_eq!(snapshot.devices[1].metrics, GpuMetrics::default());
        assert_eq!(snapshot.devices[3].metrics, GpuMetrics::default());
    }

    #[test]
    fn unmatched_nvidia_identity_is_not_reassigned() {
        let mut runner = FakeRunner {
            output: Ok("00000000:04:00.0, 99, 100, 4000, 50, 900, 20\n".to_owned()),
            calls: 0,
        };

        let snapshot = sample_discovered_gpus_with_nvidia_snapshot(
            devices(),
            Path::new("/sys/daia-missing-drm"),
            &mut runner,
        );

        assert_eq!(runner.calls, 1);
        assert_eq!(snapshot.devices[1].metrics, GpuMetrics::default());
        assert_eq!(snapshot.devices[3].metrics, GpuMetrics::default());
    }
}
