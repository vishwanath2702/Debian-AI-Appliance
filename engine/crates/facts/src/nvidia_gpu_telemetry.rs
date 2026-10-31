//! Injectable NVIDIA GPU telemetry observations.
//!
//! This module does not load NVIDIA libraries or invoke vendor commands.
//! The backend supplies measurements for an explicitly identified PCI GPU.

use crate::GpuFacts;
use crate::gpu_telemetry::{GpuMetrics, GpuTelemetrySample, GpuVendor};

/// Raw measurements supplied by an NVIDIA telemetry backend.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NvidiaGpuMetrics {
    pub utilization_percent: Option<f64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub temperature_celsius: Option<f64>,
    pub frequency_mhz: Option<u64>,
    pub power_watts: Option<f64>,
}

/// Source of NVIDIA GPU measurements.
///
/// Implementations must associate observations with the requested PCI identity.
pub trait NvidiaTelemetryBackend {
    type Error;

    fn sample(&mut self, pci_identifier: &str) -> Result<NvidiaGpuMetrics, Self::Error>;
}

/// Converts optional NVIDIA measurements into vendor-neutral metrics.
#[must_use]
pub fn nvidia_metrics(metrics: NvidiaGpuMetrics) -> GpuMetrics {
    GpuMetrics {
        utilization_percent: metrics.utilization_percent,
        memory_used_bytes: metrics.memory_used_bytes,
        memory_total_bytes: metrics.memory_total_bytes,
        temperature_celsius: metrics.temperature_celsius,
        frequency_mhz: metrics.frequency_mhz,
        power_watts: metrics.power_watts,
    }
}

/// Collects one NVIDIA GPU observation.
///
/// A missing backend, unsupported metric, or failed query does not invent
/// values. Non-NVIDIA devices are never passed to the NVIDIA backend.
#[must_use]
pub fn sample_nvidia_gpu<B: NvidiaTelemetryBackend>(
    gpu: GpuFacts,
    backend: &mut B,
) -> GpuTelemetrySample {
    if GpuVendor::from_pci_vendor_id(gpu.vendor_id()) != GpuVendor::Nvidia {
        return GpuTelemetrySample::new(gpu, GpuMetrics::default());
    }

    let metrics = backend
        .sample(gpu.identifier())
        .map(nvidia_metrics)
        .unwrap_or_default();

    GpuTelemetrySample::new(gpu, metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeBackend {
        result: Result<NvidiaGpuMetrics, &'static str>,
        requested: Vec<String>,
    }

    impl NvidiaTelemetryBackend for FakeBackend {
        type Error = &'static str;

        fn sample(&mut self, pci_identifier: &str) -> Result<NvidiaGpuMetrics, Self::Error> {
            self.requested.push(pci_identifier.to_owned());
            self.result.clone()
        }
    }

    fn nvidia() -> GpuFacts {
        GpuFacts::new("0000:01:00.0", "0x10de", "0x1fb2")
    }

    #[test]
    fn preserves_pci_identity_and_available_metrics() {
        let mut backend = FakeBackend {
            result: Ok(NvidiaGpuMetrics {
                utilization_percent: Some(42.0),
                memory_used_bytes: Some(1024),
                memory_total_bytes: Some(4096),
                temperature_celsius: Some(58.0),
                frequency_mhz: Some(1200),
                power_watts: Some(35.0),
            }),
            requested: Vec::new(),
        };

        let result = sample_nvidia_gpu(nvidia(), &mut backend);

        assert_eq!(backend.requested, ["0000:01:00.0"]);
        assert_eq!(result.gpu.identifier(), "0000:01:00.0");
        assert_eq!(result.vendor, GpuVendor::Nvidia);
        assert_eq!(result.metrics.utilization_percent, Some(42.0));
        assert_eq!(result.metrics.memory_used_bytes, Some(1024));
        assert_eq!(result.metrics.memory_total_bytes, Some(4096));
        assert_eq!(result.metrics.temperature_celsius, Some(58.0));
        assert_eq!(result.metrics.frequency_mhz, Some(1200));
        assert_eq!(result.metrics.power_watts, Some(35.0));
    }

    #[test]
    fn missing_metrics_remain_unavailable() {
        let mut backend = FakeBackend {
            result: Ok(NvidiaGpuMetrics::default()),
            requested: Vec::new(),
        };

        let result = sample_nvidia_gpu(nvidia(), &mut backend);

        assert_eq!(result.metrics, GpuMetrics::default());
        assert_eq!(backend.requested.len(), 1);
    }

    #[test]
    fn failed_backend_does_not_fabricate_metrics() {
        let mut backend = FakeBackend {
            result: Err("NVML unavailable"),
            requested: Vec::new(),
        };

        let result = sample_nvidia_gpu(nvidia(), &mut backend);

        assert_eq!(result.vendor, GpuVendor::Nvidia);
        assert_eq!(result.metrics, GpuMetrics::default());
        assert_eq!(backend.requested, ["0000:01:00.0"]);
    }

    #[test]
    fn non_nvidia_gpu_is_never_queried() {
        let mut backend = FakeBackend {
            result: Ok(NvidiaGpuMetrics {
                utilization_percent: Some(99.0),
                ..NvidiaGpuMetrics::default()
            }),
            requested: Vec::new(),
        };

        let intel = GpuFacts::new("0000:00:02.0", "0x8086", "0x5912");
        let result = sample_nvidia_gpu(intel, &mut backend);

        assert_eq!(result.vendor, GpuVendor::Intel);
        assert_eq!(result.metrics, GpuMetrics::default());
        assert!(backend.requested.is_empty());
    }
}
