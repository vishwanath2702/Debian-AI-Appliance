//! Read-only hardware monitoring with injectable telemetry sources.

use std::{io, path::Path};

use facts::GpuFacts;
use facts::gpu_telemetry::{
    GpuTelemetrySnapshot, sample_discovered_gpus, sample_discovered_gpus_with_nvidia_snapshot,
};
use facts::intel_gpu_telemetry::{IntelGpuGtSample, sample_gt};
use facts::nvidia_smi_backend::NvidiaSmiCommandRunner;
use facts::telemetry::{
    CpuTimes, MemoryUsage, cpu_utilization_percent, sample_cpu_times, sample_memory_usage,
};

/// One hardware observation. CPU utilization is unavailable on the first sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HardwareSample {
    pub cpu_utilization_percent: Option<f64>,
    pub memory: MemoryUsage,
}

/// A CPU/RAM observation with optional Intel GT counters.
///
/// GT frequency and RC6 residency are not GPU utilization.
#[derive(Clone, Debug, PartialEq)]
pub struct HardwareSampleWithIntelGpu {
    pub hardware: HardwareSample,
    pub intel_gpu: IntelGpuGtSample,
}

/// One CPU/RAM observation with zero or more identified GPUs.
#[derive(Clone, Debug, PartialEq)]
pub struct HardwareSampleWithGpus {
    pub hardware: HardwareSample,
    pub gpus: GpuTelemetrySnapshot,
}

/// Source of CPU and memory telemetry.
pub trait HardwareTelemetrySource {
    /// Reads the current aggregate CPU counters.
    ///
    /// # Errors
    /// Returns an error if CPU telemetry cannot be collected.
    fn cpu_times(&mut self) -> io::Result<CpuTimes>;

    /// Reads the current memory usage.
    ///
    /// # Errors
    /// Returns an error if memory telemetry cannot be collected.
    fn memory_usage(&mut self) -> io::Result<MemoryUsage>;
}

/// Linux procfs-backed telemetry source.
#[derive(Clone, Copy, Debug, Default)]
pub struct LinuxHardwareTelemetrySource;

impl HardwareTelemetrySource for LinuxHardwareTelemetrySource {
    fn cpu_times(&mut self) -> io::Result<CpuTimes> {
        sample_cpu_times()
    }

    fn memory_usage(&mut self) -> io::Result<MemoryUsage> {
        sample_memory_usage()
    }
}

/// Stateful monitor that calculates CPU utilization between observations.
pub struct HardwareMonitor<S> {
    source: S,
    previous_cpu: Option<CpuTimes>,
}

impl<S: HardwareTelemetrySource> HardwareMonitor<S> {
    /// Creates a monitor without a previous CPU observation.
    #[must_use]
    pub fn new(source: S) -> Self {
        Self {
            source,
            previous_cpu: None,
        }
    }

    /// Collects one hardware sample.
    ///
    /// The previous CPU counters are updated only after both telemetry
    /// reads succeed. Invalid CPU intervals yield unavailable utilization.
    ///
    /// # Errors
    /// Returns an error if CPU or memory telemetry cannot be collected.
    pub fn sample(&mut self) -> io::Result<HardwareSample> {
        let current_cpu = self.source.cpu_times()?;
        let memory = self.source.memory_usage()?;

        let cpu_utilization_percent = self
            .previous_cpu
            .and_then(|previous| cpu_utilization_percent(previous, current_cpu));

        self.previous_cpu = Some(current_cpu);

        Ok(HardwareSample {
            cpu_utilization_percent,
            memory,
        })
    }

    /// Combines CPU/RAM telemetry with a caller-supplied GPU snapshot.
    ///
    /// GPU collection and device identity resolution are handled separately.
    ///
    /// # Errors
    /// Returns an error if required CPU or memory telemetry fails.
    pub fn sample_with_gpus(
        &mut self,
        gpus: GpuTelemetrySnapshot,
    ) -> io::Result<HardwareSampleWithGpus> {
        let hardware = self.sample()?;
        Ok(HardwareSampleWithGpus { hardware, gpus })
    }

    /// Samples CPU/RAM and optional Intel GT telemetry from an explicit path.
    ///
    /// Missing GPU counters do not interrupt CPU/RAM monitoring.
    ///
    /// # Errors
    /// Returns an error only if required CPU or memory telemetry fails.
    pub fn sample_with_intel_gpu(
        &mut self,
        gt_directory: &Path,
    ) -> io::Result<HardwareSampleWithIntelGpu> {
        let hardware = self.sample()?;
        let intel_gpu = sample_gt(gt_directory);
        Ok(HardwareSampleWithIntelGpu {
            hardware,
            intel_gpu,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct FakeSource {
        cpu: VecDeque<io::Result<CpuTimes>>,
        memory: VecDeque<io::Result<MemoryUsage>>,
    }

    impl HardwareTelemetrySource for FakeSource {
        fn cpu_times(&mut self) -> io::Result<CpuTimes> {
            self.cpu.pop_front().expect("CPU fixture exhausted")
        }

        fn memory_usage(&mut self) -> io::Result<MemoryUsage> {
            self.memory.pop_front().expect("memory fixture exhausted")
        }
    }

    fn cpu(total: u64, idle: u64) -> io::Result<CpuTimes> {
        Ok(CpuTimes { total, idle })
    }

    fn memory() -> io::Result<MemoryUsage> {
        Ok(MemoryUsage {
            total_bytes: 8192,
            available_bytes: 3072,
        })
    }

    #[test]
    fn first_sample_has_no_cpu_percentage() {
        let source = FakeSource {
            cpu: VecDeque::from([cpu(100, 60)]),
            memory: VecDeque::from([memory()]),
        };
        let mut monitor = HardwareMonitor::new(source);

        let sample = monitor.sample().expect("first sample");

        assert_eq!(sample.cpu_utilization_percent, None);
        assert_eq!(sample.memory.used_bytes(), 5120);
    }

    #[test]
    fn second_sample_calculates_cpu_percentage() {
        let source = FakeSource {
            cpu: VecDeque::from([cpu(100, 60), cpu(300, 110)]),
            memory: VecDeque::from([memory(), memory()]),
        };
        let mut monitor = HardwareMonitor::new(source);

        monitor.sample().expect("first sample");
        let second = monitor.sample().expect("second sample");

        assert_eq!(second.cpu_utilization_percent, Some(75.0));
    }

    #[test]
    fn failed_memory_read_does_not_advance_cpu_baseline() {
        let source = FakeSource {
            cpu: VecDeque::from([cpu(100, 60), cpu(200, 100), cpu(300, 110)]),
            memory: VecDeque::from([
                memory(),
                Err(io::Error::other("memory unavailable")),
                memory(),
            ]),
        };
        let mut monitor = HardwareMonitor::new(source);

        monitor.sample().expect("first sample");
        assert!(monitor.sample().is_err());

        let recovered = monitor.sample().expect("recovered sample");
        assert_eq!(recovered.cpu_utilization_percent, Some(75.0));
    }

    #[test]
    fn invalid_cpu_interval_is_unavailable_not_zero() {
        let source = FakeSource {
            cpu: VecDeque::from([cpu(100, 60), cpu(100, 60)]),
            memory: VecDeque::from([memory(), memory()]),
        };
        let mut monitor = HardwareMonitor::new(source);

        monitor.sample().expect("first sample");
        let second = monitor.sample().expect("second sample");

        assert_eq!(second.cpu_utilization_percent, None);
    }

    #[test]
    fn cpu_read_failure_does_not_consume_memory_sample() {
        let source = FakeSource {
            cpu: VecDeque::from([Err(io::Error::other("CPU unavailable")), cpu(100, 60)]),
            memory: VecDeque::from([memory()]),
        };
        let mut monitor = HardwareMonitor::new(source);

        assert!(monitor.sample().is_err());
        let recovered = monitor.sample().expect("recovered sample");

        assert_eq!(recovered.cpu_utilization_percent, None);
        assert_eq!(recovered.memory.used_bytes(), 5120);
    }
}

/// Result of a bounded monitoring session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MonitoringOutcome {
    Completed,
    Cancelled,
}

/// Runs a bounded series of hardware observations.
///
/// The callback receives each successful sample and returns `false` to stop.
/// No worker thread is created. The caller owns the execution context.
///
/// # Errors
/// Returns an error if the interval is zero, the sample limit is zero,
/// or hardware telemetry cannot be read.
pub fn monitor_periodically<S, F>(
    monitor: &mut HardwareMonitor<S>,
    interval: std::time::Duration,
    maximum_samples: usize,
    mut on_sample: F,
) -> io::Result<MonitoringOutcome>
where
    S: HardwareTelemetrySource,
    F: FnMut(HardwareSample) -> bool,
{
    if interval.is_zero() || maximum_samples == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "monitoring interval and sample limit must be positive",
        ));
    }

    for index in 0..maximum_samples {
        let sample = monitor.sample()?;

        if !on_sample(sample) {
            return Ok(MonitoringOutcome::Cancelled);
        }

        if index + 1 < maximum_samples {
            std::thread::sleep(interval);
        }
    }

    Ok(MonitoringOutcome::Completed)
}

#[cfg(test)]
mod periodic_tests {
    use super::*;
    use std::{collections::VecDeque, time::Duration};

    struct FakeSource {
        cpu: VecDeque<CpuTimes>,
    }

    impl HardwareTelemetrySource for FakeSource {
        fn cpu_times(&mut self) -> io::Result<CpuTimes> {
            Ok(self.cpu.pop_front().expect("CPU fixture exhausted"))
        }

        fn memory_usage(&mut self) -> io::Result<MemoryUsage> {
            Ok(MemoryUsage {
                total_bytes: 8192,
                available_bytes: 4096,
            })
        }
    }

    #[test]
    fn cancellation_stops_after_first_callback() {
        let mut monitor = HardwareMonitor::new(FakeSource {
            cpu: VecDeque::from([CpuTimes {
                total: 100,
                idle: 60,
            }]),
        });

        let mut calls = 0;
        let outcome = monitor_periodically(&mut monitor, Duration::from_secs(1), 100, |_| {
            calls += 1;
            false
        })
        .expect("cancel monitoring");

        assert_eq!(outcome, MonitoringOutcome::Cancelled);
        assert_eq!(calls, 1);
    }

    #[test]
    fn single_sample_completes_without_sleeping() {
        let mut monitor = HardwareMonitor::new(FakeSource {
            cpu: VecDeque::from([CpuTimes {
                total: 100,
                idle: 60,
            }]),
        });

        let outcome = monitor_periodically(&mut monitor, Duration::from_secs(1), 1, |_| true)
            .expect("complete monitoring");

        assert_eq!(outcome, MonitoringOutcome::Completed);
    }

    #[test]
    fn invalid_configuration_does_not_read_hardware() {
        let mut monitor = HardwareMonitor::new(FakeSource {
            cpu: VecDeque::new(),
        });

        assert_eq!(
            monitor_periodically(&mut monitor, Duration::ZERO, 1, |_| true,)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );

        assert_eq!(
            monitor_periodically(&mut monitor, Duration::from_secs(1), 0, |_| true,)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
}

/// Shareable cancellation handle for an interruptible monitoring session.
#[derive(Clone, Debug, Default)]
pub struct MonitoringCancellation {
    state: std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
}

impl MonitoringCancellation {
    /// Creates a cancellation handle in the active state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation and wakes a waiting monitor.
    pub fn cancel(&self) {
        let (lock, wake) = &*self.state;
        let mut cancelled = lock.lock().unwrap_or_else(|e| e.into_inner());
        *cancelled = true;
        wake.notify_all();
    }

    /// Returns whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        let (lock, _) = &*self.state;
        *lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn wait_or_cancel(&self, interval: std::time::Duration) -> bool {
        let (lock, wake) = &*self.state;
        let cancelled = lock.lock().unwrap_or_else(|e| e.into_inner());

        if *cancelled {
            return true;
        }

        let (cancelled, _) = wake
            .wait_timeout_while(cancelled, interval, |value| !*value)
            .unwrap_or_else(|e| e.into_inner());

        *cancelled
    }
}

/// Runs bounded monitoring with cancellation that interrupts interval waits.
///
/// The caller owns the execution context; this function creates no threads.
/// Cancellation can be requested through a cloned handle from another thread.
///
/// # Errors
/// Returns an error for invalid configuration or failed telemetry reads.
pub fn monitor_interruptibly<S, F>(
    monitor: &mut HardwareMonitor<S>,
    interval: std::time::Duration,
    maximum_samples: usize,
    cancellation: &MonitoringCancellation,
    mut on_sample: F,
) -> io::Result<MonitoringOutcome>
where
    S: HardwareTelemetrySource,
    F: FnMut(HardwareSample) -> bool,
{
    if interval.is_zero() || maximum_samples == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "monitoring interval and sample limit must be positive",
        ));
    }

    for index in 0..maximum_samples {
        if cancellation.is_cancelled() {
            return Ok(MonitoringOutcome::Cancelled);
        }

        let sample = monitor.sample()?;

        if !on_sample(sample) || cancellation.is_cancelled() {
            return Ok(MonitoringOutcome::Cancelled);
        }

        if index + 1 < maximum_samples && cancellation.wait_or_cancel(interval) {
            return Ok(MonitoringOutcome::Cancelled);
        }
    }

    Ok(MonitoringOutcome::Completed)
}

/// Runs bounded, interruptible CPU/RAM and multi-GPU monitoring.
///
/// The GPU snapshot provider is called once per successful CPU/RAM sample.
/// GPU collection policy is supplied by the caller, independently of inference.
///
/// # Errors
/// Returns an error for invalid configuration or failed CPU/RAM telemetry.
pub fn monitor_gpus_interruptibly<S, G, F>(
    monitor: &mut HardwareMonitor<S>,
    interval: std::time::Duration,
    maximum_samples: usize,
    cancellation: &MonitoringCancellation,
    mut gpu_snapshot: G,
    mut on_sample: F,
) -> io::Result<MonitoringOutcome>
where
    S: HardwareTelemetrySource,
    G: FnMut() -> GpuTelemetrySnapshot,
    F: FnMut(HardwareSampleWithGpus) -> bool,
{
    if interval.is_zero() || maximum_samples == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "monitoring interval and sample limit must be positive",
        ));
    }

    for index in 0..maximum_samples {
        if cancellation.is_cancelled() {
            return Ok(MonitoringOutcome::Cancelled);
        }

        let hardware = monitor.sample()?;
        if cancellation.is_cancelled() {
            return Ok(MonitoringOutcome::Cancelled);
        }

        let gpus = gpu_snapshot();
        if cancellation.is_cancelled() {
            return Ok(MonitoringOutcome::Cancelled);
        }

        if !on_sample(HardwareSampleWithGpus { hardware, gpus }) || cancellation.is_cancelled() {
            return Ok(MonitoringOutcome::Cancelled);
        }

        if index + 1 < maximum_samples && cancellation.wait_or_cancel(interval) {
            return Ok(MonitoringOutcome::Cancelled);
        }
    }

    Ok(MonitoringOutcome::Completed)
}

#[cfg(test)]
mod interruptible_tests {
    use super::*;
    use std::{collections::VecDeque, time::Duration};

    struct FakeSource {
        cpu: VecDeque<CpuTimes>,
    }

    impl HardwareTelemetrySource for FakeSource {
        fn cpu_times(&mut self) -> io::Result<CpuTimes> {
            Ok(self.cpu.pop_front().expect("unexpected CPU read"))
        }

        fn memory_usage(&mut self) -> io::Result<MemoryUsage> {
            Ok(MemoryUsage {
                total_bytes: 8192,
                available_bytes: 4096,
            })
        }
    }

    fn monitor(cpu: Vec<CpuTimes>) -> HardwareMonitor<FakeSource> {
        HardwareMonitor::new(FakeSource { cpu: cpu.into() })
    }

    #[test]
    fn pre_cancelled_monitor_does_not_read_hardware() {
        let cancellation = MonitoringCancellation::new();
        cancellation.cancel();

        let outcome = monitor_interruptibly(
            &mut monitor(vec![]),
            Duration::from_secs(1),
            10,
            &cancellation,
            |_| panic!("callback must not run"),
        )
        .expect("cancelled session");

        assert_eq!(outcome, MonitoringOutcome::Cancelled);
    }

    #[test]
    fn callback_can_cancel_session() {
        let cancellation = MonitoringCancellation::new();
        let callback_handle = cancellation.clone();

        let outcome = monitor_interruptibly(
            &mut monitor(vec![CpuTimes {
                total: 100,
                idle: 60,
            }]),
            Duration::from_secs(1),
            10,
            &cancellation,
            move |_| {
                callback_handle.cancel();
                true
            },
        )
        .expect("cancelled session");

        assert_eq!(outcome, MonitoringOutcome::Cancelled);
    }

    #[test]
    fn cross_thread_cancellation_interrupts_long_wait() {
        use std::sync::mpsc;
        use std::time::Instant;

        let cancellation = MonitoringCancellation::new();
        let worker_cancellation = cancellation.clone();
        let (sample_sent, sample_received) = mpsc::channel();

        let worker = std::thread::spawn(move || {
            let mut monitor = monitor(vec![
                CpuTimes {
                    total: 100,
                    idle: 60,
                },
                CpuTimes {
                    total: 200,
                    idle: 100,
                },
            ]);

            monitor_interruptibly(
                &mut monitor,
                Duration::from_secs(5),
                10,
                &worker_cancellation,
                |_| {
                    sample_sent.send(()).expect("send first sample");
                    true
                },
            )
        });

        sample_received
            .recv_timeout(Duration::from_secs(2))
            .expect("first sample should arrive");

        // Allow the worker to enter the interval wait.
        std::thread::sleep(Duration::from_millis(50));

        let start = Instant::now();
        cancellation.cancel();

        let outcome = worker
            .join()
            .expect("monitor worker panicked")
            .expect("monitoring failed");

        assert_eq!(outcome, MonitoringOutcome::Cancelled);
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "cancellation did not promptly interrupt the wait"
        );
    }

    #[test]
    fn cancellation_handle_is_shared() {
        let first = MonitoringCancellation::new();
        let second = first.clone();

        assert!(!first.is_cancelled());
        second.cancel();
        assert!(first.is_cancelled());
    }

    #[test]
    fn one_sample_completes_without_waiting() {
        let cancellation = MonitoringCancellation::new();

        let outcome = monitor_interruptibly(
            &mut monitor(vec![CpuTimes {
                total: 100,
                idle: 60,
            }]),
            Duration::from_secs(1),
            1,
            &cancellation,
            |_| true,
        )
        .expect("completed session");

        assert_eq!(outcome, MonitoringOutcome::Completed);
    }

    #[test]
    fn invalid_configuration_does_not_read_hardware() {
        let cancellation = MonitoringCancellation::new();

        assert_eq!(
            monitor_interruptibly(
                &mut monitor(vec![]),
                Duration::ZERO,
                1,
                &cancellation,
                |_| true,
            )
            .unwrap_err()
            .kind(),
            io::ErrorKind::InvalidInput
        );
    }
}

#[cfg(test)]
mod intel_gpu_monitor_tests {
    use super::*;
    use std::fs;

    struct FixedSource;

    impl HardwareTelemetrySource for FixedSource {
        fn cpu_times(&mut self) -> io::Result<CpuTimes> {
            Ok(CpuTimes {
                total: 100,
                idle: 50,
            })
        }

        fn memory_usage(&mut self) -> io::Result<MemoryUsage> {
            Ok(MemoryUsage {
                total_bytes: 1000,
                available_bytes: 600,
            })
        }
    }

    #[test]
    fn missing_intel_gpu_does_not_block_cpu_and_memory() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing-gt");
        let mut monitor = HardwareMonitor::new(FixedSource);

        let sample = monitor.sample_with_intel_gpu(&missing).unwrap();

        assert_eq!(sample.hardware.cpu_utilization_percent, None);
        assert_eq!(sample.hardware.memory.total_bytes, 1000);
        assert_eq!(sample.intel_gpu, IntelGpuGtSample::default());
    }

    #[test]
    fn available_intel_gpu_counters_join_hardware_sample() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("rps_cur_freq_mhz"), "350\n").unwrap();
        fs::write(directory.path().join("rc6_residency_ms"), "900\n").unwrap();

        let mut monitor = HardwareMonitor::new(FixedSource);
        let sample = monitor.sample_with_intel_gpu(directory.path()).unwrap();

        assert_eq!(sample.hardware.memory.available_bytes, 600);
        assert_eq!(sample.intel_gpu.current_frequency_mhz, Some(350));
        assert_eq!(sample.intel_gpu.rc6_residency_ms, Some(900));
        assert_eq!(sample.intel_gpu.actual_frequency_mhz, None);
    }
}

#[cfg(test)]
mod multi_gpu_monitor_tests {
    use super::*;
    use facts::GpuFacts;
    use facts::gpu_telemetry::{GpuMetrics, GpuTelemetrySample};

    struct FixedSource;

    impl HardwareTelemetrySource for FixedSource {
        fn cpu_times(&mut self) -> io::Result<CpuTimes> {
            Ok(CpuTimes {
                total: 100,
                idle: 50,
            })
        }

        fn memory_usage(&mut self) -> io::Result<MemoryUsage> {
            Ok(MemoryUsage {
                total_bytes: 1000,
                available_bytes: 600,
            })
        }
    }

    #[test]
    fn combined_sample_preserves_two_gpu_identities() {
        let intel = GpuTelemetrySample::new(
            GpuFacts::new("0000:00:02.0", "0x8086", "0x5912"),
            GpuMetrics {
                frequency_mhz: Some(350),
                ..GpuMetrics::default()
            },
        );
        let nvidia = GpuTelemetrySample::new(
            GpuFacts::new("0000:01:00.0", "0x10de", "0x1fb2"),
            GpuMetrics::default(),
        );

        let snapshot = GpuTelemetrySnapshot {
            devices: vec![intel, nvidia],
        };

        let mut monitor = HardwareMonitor::new(FixedSource);
        let combined = monitor.sample_with_gpus(snapshot).unwrap();

        assert_eq!(combined.hardware.memory.total_bytes, 1000);
        assert_eq!(combined.gpus.devices.len(), 2);
        assert_eq!(combined.gpus.devices[0].gpu.identifier(), "0000:00:02.0");
        assert_eq!(combined.gpus.devices[1].gpu.identifier(), "0000:01:00.0");
        assert_eq!(combined.gpus.devices[0].metrics.frequency_mhz, Some(350));
        assert_eq!(combined.gpus.devices[1].metrics.utilization_percent, None);
    }

    #[test]
    fn empty_gpu_snapshot_preserves_cpu_and_memory() {
        let mut monitor = HardwareMonitor::new(FixedSource);

        let combined = monitor
            .sample_with_gpus(GpuTelemetrySnapshot::default())
            .unwrap();

        assert!(combined.gpus.devices.is_empty());
        assert_eq!(combined.hardware.memory.available_bytes, 600);
        assert_eq!(combined.hardware.cpu_utilization_percent, None);
    }
}

/// Periodically samples CPU/RAM and PCI-aware GPU telemetry.
///
/// GPU identities are supplied by the caller and measurements are refreshed
/// on each interval. Unsupported GPU metrics remain unavailable.
///
/// # Errors
/// Returns an error for invalid configuration or failed CPU/RAM sampling.
pub fn monitor_discovered_gpus_interruptibly<S, F>(
    monitor: &mut HardwareMonitor<S>,
    gpus: &[GpuFacts],
    drm_root: &Path,
    interval: std::time::Duration,
    maximum_samples: usize,
    cancellation: &MonitoringCancellation,
    on_sample: F,
) -> io::Result<MonitoringOutcome>
where
    S: HardwareTelemetrySource,
    F: FnMut(HardwareSampleWithGpus) -> bool,
{
    monitor_gpus_interruptibly(
        monitor,
        interval,
        maximum_samples,
        cancellation,
        || sample_discovered_gpus(gpus.to_vec(), drm_root),
        on_sample,
    )
}

/// Periodically monitors CPU/RAM and PCI-aware Intel/NVIDIA GPUs.
///
/// Each interval performs at most one NVIDIA telemetry query, regardless
/// of how many NVIDIA GPUs are present. The NVIDIA query is skipped when
/// no NVIDIA GPU was discovered.
///
/// Unsupported metrics and failed NVIDIA queries remain unavailable.
/// CPU/RAM failures and invalid monitoring configuration are reported.
///
/// Cancellation is checked between observations and interrupts interval
/// waits. An already-running external command cannot be interrupted by
/// this cancellation mechanism.
///
/// # Errors
/// Returns an error for invalid configuration or failed CPU/RAM telemetry.
pub fn monitor_discovered_gpus_with_nvidia_interruptibly<S, R, F>(
    monitor: &mut HardwareMonitor<S>,
    gpus: &[GpuFacts],
    drm_root: &Path,
    nvidia_runner: &mut R,
    interval: std::time::Duration,
    maximum_samples: usize,
    cancellation: &MonitoringCancellation,
    on_sample: F,
) -> io::Result<MonitoringOutcome>
where
    S: HardwareTelemetrySource,
    R: NvidiaSmiCommandRunner,
    F: FnMut(HardwareSampleWithGpus) -> bool,
{
    monitor_gpus_interruptibly(
        monitor,
        interval,
        maximum_samples,
        cancellation,
        || sample_discovered_gpus_with_nvidia_snapshot(gpus.to_vec(), drm_root, nvidia_runner),
        on_sample,
    )
}

#[cfg(test)]
mod periodic_multi_gpu_tests {
    use super::*;
    use facts::{GpuFacts, gpu_telemetry::GpuTelemetrySnapshot};
    use std::time::{Duration, Instant};

    struct FixedSource;

    impl HardwareTelemetrySource for FixedSource {
        fn cpu_times(&mut self) -> io::Result<CpuTimes> {
            Ok(CpuTimes {
                total: 100,
                idle: 50,
            })
        }

        fn memory_usage(&mut self) -> io::Result<MemoryUsage> {
            Ok(MemoryUsage {
                total_bytes: 1000,
                available_bytes: 600,
            })
        }
    }

    fn snapshot() -> GpuTelemetrySnapshot {
        GpuTelemetrySnapshot::from_discovered_gpus(vec![
            GpuFacts::new("0000:00:02.0", "0x8086", "0x5912"),
            GpuFacts::new("0000:01:00.0", "0x10de", "0x1fb2"),
        ])
    }

    #[test]
    fn periodic_samples_include_multiple_gpu_identities() {
        let mut monitor = HardwareMonitor::new(FixedSource);
        let cancellation = MonitoringCancellation::new();
        let mut provider_calls = 0;
        let mut callback_calls = 0;

        let outcome = monitor_gpus_interruptibly(
            &mut monitor,
            Duration::from_millis(1),
            2,
            &cancellation,
            || {
                provider_calls += 1;
                snapshot()
            },
            |sample| {
                callback_calls += 1;
                assert_eq!(sample.hardware.memory.available_bytes, 600);
                assert_eq!(sample.gpus.devices.len(), 2);
                assert_eq!(sample.gpus.devices[1].gpu.identifier(), "0000:01:00.0");
                true
            },
        )
        .unwrap();

        assert_eq!(outcome, MonitoringOutcome::Completed);
        assert_eq!(provider_calls, 2);
        assert_eq!(callback_calls, 2);
    }

    #[test]
    fn pre_cancelled_session_does_not_collect_gpu_data() {
        let mut monitor = HardwareMonitor::new(FixedSource);
        let cancellation = MonitoringCancellation::new();
        cancellation.cancel();

        let outcome = monitor_gpus_interruptibly(
            &mut monitor,
            Duration::from_secs(1),
            10,
            &cancellation,
            || panic!("GPU provider must not run"),
            |_| panic!("callback must not run"),
        )
        .unwrap();

        assert_eq!(outcome, MonitoringOutcome::Cancelled);
    }

    #[test]
    fn callback_cancellation_interrupts_session() {
        let mut monitor = HardwareMonitor::new(FixedSource);
        let cancellation = MonitoringCancellation::new();
        let handle = cancellation.clone();
        let mut count = 0;

        let outcome = monitor_gpus_interruptibly(
            &mut monitor,
            Duration::from_secs(5),
            10,
            &cancellation,
            snapshot,
            |_| {
                count += 1;
                handle.cancel();
                true
            },
        )
        .unwrap();

        assert_eq!(outcome, MonitoringOutcome::Cancelled);
        assert_eq!(count, 1);
    }

    #[test]
    fn invalid_configuration_does_not_sample_gpu() {
        let mut monitor = HardwareMonitor::new(FixedSource);
        let cancellation = MonitoringCancellation::new();

        let error = monitor_gpus_interruptibly(
            &mut monitor,
            Duration::ZERO,
            1,
            &cancellation,
            || panic!("GPU provider must not run"),
            |_| true,
        )
        .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn cancellation_interrupts_wait_between_gpu_samples() {
        let cancellation = MonitoringCancellation::new();
        let worker_handle = cancellation.clone();
        let (tx, rx) = std::sync::mpsc::channel();

        let worker = std::thread::spawn(move || {
            let mut monitor = HardwareMonitor::new(FixedSource);
            monitor_gpus_interruptibly(
                &mut monitor,
                Duration::from_secs(5),
                10,
                &worker_handle,
                snapshot,
                |_| {
                    tx.send(()).unwrap();
                    true
                },
            )
            .unwrap()
        });

        rx.recv_timeout(Duration::from_secs(2))
            .expect("first GPU sample");

        let started = Instant::now();
        cancellation.cancel();
        let outcome = worker.join().expect("monitor worker");

        assert_eq!(outcome, MonitoringOutcome::Cancelled);
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}

#[cfg(test)]
mod discovered_gpu_monitor_tests {
    use super::*;
    use std::time::Duration;

    struct FixedSource;

    impl HardwareTelemetrySource for FixedSource {
        fn cpu_times(&mut self) -> io::Result<CpuTimes> {
            Ok(CpuTimes {
                total: 100,
                idle: 50,
            })
        }

        fn memory_usage(&mut self) -> io::Result<MemoryUsage> {
            Ok(MemoryUsage {
                total_bytes: 1000,
                available_bytes: 600,
            })
        }
    }

    fn devices() -> Vec<GpuFacts> {
        vec![
            GpuFacts::new("0000:00:02.0", "0x8086", "0x5912"),
            GpuFacts::new("0000:01:00.0", "0x10de", "0x1fb2"),
        ]
    }

    #[test]
    fn missing_drm_preserves_gpu_identities() {
        let mut monitor = HardwareMonitor::new(FixedSource);
        let cancellation = MonitoringCancellation::new();
        let mut observations = 0;

        let outcome = monitor_discovered_gpus_interruptibly(
            &mut monitor,
            &devices(),
            Path::new("/sys/daia-nonexistent-drm-root"),
            Duration::from_millis(1),
            2,
            &cancellation,
            |sample| {
                observations += 1;
                assert_eq!(sample.hardware.memory.available_bytes, 600);
                assert_eq!(sample.gpus.devices.len(), 2);
                assert_eq!(sample.gpus.devices[0].gpu.identifier(), "0000:00:02.0");
                assert_eq!(sample.gpus.devices[1].gpu.identifier(), "0000:01:00.0");
                assert!(
                    sample
                        .gpus
                        .devices
                        .iter()
                        .all(|gpu| gpu.metrics == facts::gpu_telemetry::GpuMetrics::default())
                );
                true
            },
        )
        .unwrap();

        assert_eq!(outcome, MonitoringOutcome::Completed);
        assert_eq!(observations, 2);
    }

    #[test]
    fn pre_cancelled_session_emits_no_observations() {
        let mut monitor = HardwareMonitor::new(FixedSource);
        let cancellation = MonitoringCancellation::new();
        cancellation.cancel();

        let outcome = monitor_discovered_gpus_interruptibly(
            &mut monitor,
            &devices(),
            Path::new("/sys/class/drm"),
            Duration::from_secs(1),
            10,
            &cancellation,
            |_| panic!("cancelled session must not emit observations"),
        )
        .unwrap();

        assert_eq!(outcome, MonitoringOutcome::Cancelled);
    }

    #[test]
    fn callback_cancels_automatic_gpu_monitoring() {
        let mut monitor = HardwareMonitor::new(FixedSource);
        let cancellation = MonitoringCancellation::new();
        let handle = cancellation.clone();
        let mut observations = 0;

        let outcome = monitor_discovered_gpus_interruptibly(
            &mut monitor,
            &devices(),
            Path::new("/sys/daia-nonexistent-drm-root"),
            Duration::from_secs(5),
            10,
            &cancellation,
            |_| {
                observations += 1;
                handle.cancel();
                true
            },
        )
        .unwrap();

        assert_eq!(outcome, MonitoringOutcome::Cancelled);
        assert_eq!(observations, 1);
    }
}

#[cfg(test)]
mod nvidia_periodic_monitor_tests {
    use super::*;
    use std::time::Duration;

    struct FixedSource;

    impl HardwareTelemetrySource for FixedSource {
        fn cpu_times(&mut self) -> io::Result<CpuTimes> {
            Ok(CpuTimes {
                total: 100,
                idle: 50,
            })
        }

        fn memory_usage(&mut self) -> io::Result<MemoryUsage> {
            Ok(MemoryUsage {
                total_bytes: 1000,
                available_bytes: 600,
            })
        }
    }

    struct FakeNvidiaRunner {
        calls: usize,
        fail: bool,
    }

    impl NvidiaSmiCommandRunner for FakeNvidiaRunner {
        fn query(&mut self) -> io::Result<String> {
            self.calls += 1;

            if self.fail {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "NVIDIA utility unavailable",
                ));
            }

            Ok(concat!(
                "00000000:02:00.0, 70, 200, 8000, 65, 1500, 60\n",
                "00000000:01:00.0, 25, 100, 4000, 50, 900, 20\n",
            )
            .to_owned())
        }
    }

    fn devices() -> Vec<GpuFacts> {
        vec![
            GpuFacts::new("0000:00:02.0", "0x8086", "0x5912"),
            GpuFacts::new("0000:01:00.0", "0x10de", "0x1fb2"),
            GpuFacts::new("0000:02:00.0", "0x10de", "0x2684"),
        ]
    }

    #[test]
    fn each_interval_queries_nvidia_once_for_multiple_gpus() {
        let mut monitor = HardwareMonitor::new(FixedSource);
        let cancellation = MonitoringCancellation::new();
        let mut runner = FakeNvidiaRunner {
            calls: 0,
            fail: false,
        };
        let mut observations = 0;

        let outcome = monitor_discovered_gpus_with_nvidia_interruptibly(
            &mut monitor,
            &devices(),
            Path::new("/sys/daia-missing-drm"),
            &mut runner,
            Duration::from_millis(1),
            3,
            &cancellation,
            |sample| {
                observations += 1;
                assert_eq!(sample.hardware.memory.available_bytes, 600);
                assert_eq!(sample.gpus.devices.len(), 3);
                assert_eq!(
                    sample.gpus.devices[1].metrics.utilization_percent,
                    Some(25.0)
                );
                assert_eq!(
                    sample.gpus.devices[2].metrics.utilization_percent,
                    Some(70.0)
                );
                true
            },
        )
        .unwrap();

        assert_eq!(outcome, MonitoringOutcome::Completed);
        assert_eq!(observations, 3);
        assert_eq!(runner.calls, 3);
    }

    #[test]
    fn unavailable_nvidia_does_not_interrupt_monitoring() {
        let mut monitor = HardwareMonitor::new(FixedSource);
        let cancellation = MonitoringCancellation::new();
        let mut runner = FakeNvidiaRunner {
            calls: 0,
            fail: true,
        };

        let outcome = monitor_discovered_gpus_with_nvidia_interruptibly(
            &mut monitor,
            &devices(),
            Path::new("/sys/daia-missing-drm"),
            &mut runner,
            Duration::from_millis(1),
            2,
            &cancellation,
            |sample| {
                assert_eq!(sample.gpus.devices.len(), 3);
                assert_eq!(
                    sample.gpus.devices[1].metrics,
                    facts::gpu_telemetry::GpuMetrics::default()
                );
                assert_eq!(
                    sample.gpus.devices[2].metrics,
                    facts::gpu_telemetry::GpuMetrics::default()
                );
                true
            },
        )
        .unwrap();

        assert_eq!(outcome, MonitoringOutcome::Completed);
        assert_eq!(runner.calls, 2);
    }

    #[test]
    fn cancellation_prevents_nvidia_query() {
        let mut monitor = HardwareMonitor::new(FixedSource);
        let cancellation = MonitoringCancellation::new();
        cancellation.cancel();

        let mut runner = FakeNvidiaRunner {
            calls: 0,
            fail: false,
        };

        let outcome = monitor_discovered_gpus_with_nvidia_interruptibly(
            &mut monitor,
            &devices(),
            Path::new("/sys/daia-missing-drm"),
            &mut runner,
            Duration::from_secs(1),
            10,
            &cancellation,
            |_| panic!("cancelled session must not emit observations"),
        )
        .unwrap();

        assert_eq!(outcome, MonitoringOutcome::Cancelled);
        assert_eq!(runner.calls, 0);
    }

    #[test]
    fn callback_cancellation_stops_further_nvidia_queries() {
        let mut monitor = HardwareMonitor::new(FixedSource);
        let cancellation = MonitoringCancellation::new();
        let handle = cancellation.clone();

        let mut runner = FakeNvidiaRunner {
            calls: 0,
            fail: false,
        };

        let outcome = monitor_discovered_gpus_with_nvidia_interruptibly(
            &mut monitor,
            &devices(),
            Path::new("/sys/daia-missing-drm"),
            &mut runner,
            Duration::from_secs(5),
            10,
            &cancellation,
            |_| {
                handle.cancel();
                true
            },
        )
        .unwrap();

        assert_eq!(outcome, MonitoringOutcome::Cancelled);
        assert_eq!(runner.calls, 1);
    }

    #[test]
    fn intel_only_monitor_skips_nvidia_command() {
        let mut monitor = HardwareMonitor::new(FixedSource);
        let cancellation = MonitoringCancellation::new();

        let mut runner = FakeNvidiaRunner {
            calls: 0,
            fail: false,
        };

        let intel = [GpuFacts::new("0000:00:02.0", "0x8086", "0x5912")];

        let outcome = monitor_discovered_gpus_with_nvidia_interruptibly(
            &mut monitor,
            &intel,
            Path::new("/sys/daia-missing-drm"),
            &mut runner,
            Duration::from_millis(1),
            2,
            &cancellation,
            |sample| {
                assert_eq!(sample.gpus.devices.len(), 1);
                true
            },
        )
        .unwrap();

        assert_eq!(outcome, MonitoringOutcome::Completed);
        assert_eq!(runner.calls, 0);
    }
}
