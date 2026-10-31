//! Concurrent hardware observations during model benchmarks.

use std::{
    path::PathBuf,
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use facts::{
    GpuFacts, discover_gpus,
    gpu_telemetry::{
        GpuVendor, sample_discovered_gpus, sample_discovered_gpus_with_nvidia_snapshot,
    },
    nvidia_smi_backend::{NvidiaSmiCommandRunner, SystemNvidiaSmiCommandRunner},
};

use model::BenchmarkError;

use crate::{
    benchmark::{BenchmarkMonitoringSession, BenchmarkMonitoringWorker},
    hardware_monitor::{
        HardwareMonitor, HardwareSampleWithGpus, HardwareTelemetrySource,
        LinuxHardwareTelemetrySource, MonitoringCancellation, monitor_gpus_interruptibly,
    },
};

/// Hardware observations captured during benchmark inference.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BenchmarkHardwareObservations {
    pub samples: Vec<HardwareSampleWithGpus>,
}

/// Configures a concurrent benchmark hardware monitoring session.
pub struct BenchmarkHardwareWorker {
    interval: Duration,
    maximum_samples: usize,
    gpus: Vec<GpuFacts>,
    drm_root: PathBuf,
}

impl BenchmarkHardwareWorker {
    /// Discovers GPUs and configures monitoring on the current Linux host.
    ///
    /// # Errors
    /// Returns an error if GPU discovery fails.
    pub fn local() -> Result<Self, BenchmarkError> {
        let gpus = discover_gpus().map_err(|error| {
            BenchmarkError::Execution(format!("benchmark GPU discovery failed: {error}"))
        })?;

        Ok(Self {
            interval: Duration::from_millis(250),
            maximum_samples: 3600,
            gpus,
            drm_root: PathBuf::from("/sys/class/drm"),
        })
    }
}

/// Owns the cancellation handle and monitoring thread.
pub struct BenchmarkHardwareSession {
    cancellation: MonitoringCancellation,
    thread: Option<JoinHandle<Result<BenchmarkHardwareObservations, BenchmarkError>>>,
}

impl BenchmarkMonitoringWorker for BenchmarkHardwareWorker {
    type Output = BenchmarkHardwareObservations;
    type Session = BenchmarkHardwareSession;

    fn start(self) -> Result<Self::Session, BenchmarkError> {
        if self.interval.is_zero() || self.maximum_samples == 0 {
            return Err(BenchmarkError::InvalidConfiguration(
                "hardware_monitoring_interval_or_sample_limit",
            ));
        }

        start_monitoring_thread(
            HardwareMonitor::new(LinuxHardwareTelemetrySource),
            SystemNvidiaSmiCommandRunner::default(),
            self.gpus,
            self.drm_root,
            self.interval,
            self.maximum_samples,
        )
    }
}

fn start_monitoring_thread<S, R>(
    mut monitor: HardwareMonitor<S>,
    mut nvidia: R,
    gpus: Vec<GpuFacts>,
    drm_root: PathBuf,
    interval: Duration,
    maximum_samples: usize,
) -> Result<BenchmarkHardwareSession, BenchmarkError>
where
    S: HardwareTelemetrySource + Send + 'static,
    R: NvidiaSmiCommandRunner + Send + 'static,
{
    let cancellation = MonitoringCancellation::new();
    let worker_cancellation = cancellation.clone();
    let (startup_sender, startup_receiver) = mpsc::sync_channel(1);

    let thread = thread::Builder::new()
        .name("daia-benchmark-monitor".into())
        .spawn(move || {
            collect_hardware_observations(
                &mut monitor,
                &gpus,
                &drm_root,
                &mut nvidia,
                interval,
                maximum_samples,
                &worker_cancellation,
                startup_sender,
            )
        })
        .map_err(|error| {
            BenchmarkError::Execution(format!(
                "benchmark monitoring thread failed to start: {error}"
            ))
        })?;

    match startup_receiver.recv() {
        Ok(Ok(())) => Ok(BenchmarkHardwareSession {
            cancellation,
            thread: Some(thread),
        }),
        Ok(Err(error)) => {
            cancellation.cancel();
            let _ = thread.join();
            Err(error)
        }
        Err(error) => {
            cancellation.cancel();
            let _ = thread.join();
            Err(BenchmarkError::Execution(format!(
                "benchmark monitoring startup handshake failed: {error}"
            )))
        }
    }
}

/// Determines whether a fresh NVIDIA query is due without reusing stale metrics.
fn nvidia_query_due(last_query: &mut Option<Instant>, now: Instant, interval: Duration) -> bool {
    let due = last_query.is_none_or(|previous| now.saturating_duration_since(previous) >= interval);

    if due {
        *last_query = Some(now);
    }

    due
}

fn collect_hardware_observations<S, R>(
    monitor: &mut HardwareMonitor<S>,
    gpus: &[GpuFacts],
    drm_root: &std::path::Path,
    nvidia: &mut R,
    interval: Duration,
    maximum_samples: usize,
    cancellation: &MonitoringCancellation,
    startup_sender: mpsc::SyncSender<Result<(), BenchmarkError>>,
) -> Result<BenchmarkHardwareObservations, BenchmarkError>
where
    S: HardwareTelemetrySource,
    R: NvidiaSmiCommandRunner,
{
    let mut samples = Vec::new();
    let mut startup_sender = Some(startup_sender);

    let has_nvidia = gpus
        .iter()
        .any(|gpu| GpuVendor::from_pci_vendor_id(gpu.vendor_id()) == GpuVendor::Nvidia);
    let nvidia_interval = Duration::from_secs(1);
    let mut last_nvidia_query: Option<Instant> = None;

    let result = monitor_gpus_interruptibly(
        monitor,
        interval,
        maximum_samples,
        cancellation,
        || {
            if !has_nvidia {
                return sample_discovered_gpus(gpus.to_vec(), drm_root);
            }

            let now = Instant::now();

            if nvidia_query_due(&mut last_nvidia_query, now, nvidia_interval) {
                // Query time is recorded before the external command runs.
                sample_discovered_gpus_with_nvidia_snapshot(gpus.to_vec(), drm_root, nvidia)
            } else {
                // Preserve identities but do not present cached NVIDIA
                // measurements as fresh observations.
                sample_discovered_gpus(gpus.to_vec(), drm_root)
            }
        },
        |sample| {
            samples.push(sample);

            if let Some(sender) = startup_sender.take() {
                let _ = sender.send(Ok(()));
            }

            true
        },
    );

    match result {
        Ok(_) => {
            if let Some(sender) = startup_sender.take() {
                let _ = sender.send(Err(BenchmarkError::Execution(
                    "benchmark monitoring ended before its first sample".into(),
                )));
            }

            Ok(BenchmarkHardwareObservations { samples })
        }
        Err(error) => {
            let error =
                BenchmarkError::Execution(format!("benchmark hardware monitoring failed: {error}"));

            if let Some(sender) = startup_sender.take() {
                let _ = sender.send(Err(error.clone()));
            }

            Err(error)
        }
    }
}

#[cfg(test)]
mod collector_tests {
    use super::*;
    use facts::telemetry::{CpuTimes, MemoryUsage};
    use std::io;

    #[test]
    fn nvidia_polling_waits_one_second_between_queries() {
        let start = Instant::now();
        let mut last_query = None;
        let interval = Duration::from_secs(1);

        assert!(nvidia_query_due(&mut last_query, start, interval));

        for milliseconds in [250, 500, 750, 999] {
            assert!(
                !nvidia_query_due(
                    &mut last_query,
                    start + Duration::from_millis(milliseconds),
                    interval,
                ),
                "query must not run at {milliseconds} ms"
            );
        }

        assert!(nvidia_query_due(
            &mut last_query,
            start + Duration::from_secs(1),
            interval,
        ));

        assert!(!nvidia_query_due(
            &mut last_query,
            start + Duration::from_millis(1250),
            interval,
        ));

        assert!(nvidia_query_due(
            &mut last_query,
            start + Duration::from_secs(2),
            interval,
        ));
    }

    #[test]
    fn nvidia_polling_uses_actual_query_start_time() {
        let start = Instant::now();
        let mut last_query = None;
        let interval = Duration::from_secs(1);

        assert!(nvidia_query_due(&mut last_query, start, interval));

        let delayed = start + Duration::from_millis(1750);
        assert!(nvidia_query_due(&mut last_query, delayed, interval));

        assert!(!nvidia_query_due(
            &mut last_query,
            start + Duration::from_secs(2),
            interval,
        ));

        assert!(nvidia_query_due(
            &mut last_query,
            delayed + interval,
            interval,
        ));
    }

    struct FakeSource {
        fail: bool,
        reads: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl HardwareTelemetrySource for FakeSource {
        fn cpu_times(&mut self) -> io::Result<CpuTimes> {
            self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            if self.fail {
                Err(io::Error::other("synthetic CPU failure"))
            } else {
                Ok(CpuTimes {
                    total: 100,
                    idle: 60,
                })
            }
        }

        fn memory_usage(&mut self) -> io::Result<MemoryUsage> {
            Ok(MemoryUsage {
                total_bytes: 8192,
                available_bytes: 4096,
            })
        }
    }

    struct FakeNvidia;

    impl NvidiaSmiCommandRunner for FakeNvidia {
        fn query(&mut self) -> io::Result<String> {
            panic!("NVIDIA must not be queried without a discovered NVIDIA GPU")
        }
    }

    #[test]
    fn collects_cpu_and_memory_without_discovered_gpus() {
        let mut monitor = HardwareMonitor::new(FakeSource {
            fail: false,
            reads: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });
        let cancellation = MonitoringCancellation::new();
        let (sender, receiver) = mpsc::sync_channel(1);

        let observations = collect_hardware_observations(
            &mut monitor,
            &[],
            std::path::Path::new("/nonexistent-daia-drm"),
            &mut FakeNvidia,
            Duration::from_millis(1),
            1,
            &cancellation,
            sender,
        )
        .expect("collect hardware");

        assert_eq!(receiver.recv().unwrap(), Ok(()));
        assert_eq!(observations.samples.len(), 1);
        assert!(observations.samples[0].gpus.devices.is_empty());
        assert_eq!(observations.samples[0].hardware.memory.used_bytes(), 4096);
        assert_eq!(
            observations.samples[0].hardware.cpu_utilization_percent,
            None
        );
    }

    #[test]
    fn reports_sampling_failure_before_first_sample() {
        let mut monitor = HardwareMonitor::new(FakeSource {
            fail: true,
            reads: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });
        let cancellation = MonitoringCancellation::new();
        let (sender, receiver) = mpsc::sync_channel(1);

        let error = collect_hardware_observations(
            &mut monitor,
            &[],
            std::path::Path::new("/nonexistent-daia-drm"),
            &mut FakeNvidia,
            Duration::from_millis(1),
            1,
            &cancellation,
            sender,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            BenchmarkError::Execution(message)
                if message.contains("synthetic CPU failure")
        ));
        assert!(receiver.recv().unwrap().is_err());
    }

    #[test]
    fn pre_cancelled_collector_does_not_read_hardware() {
        let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut monitor = HardwareMonitor::new(FakeSource {
            fail: false,
            reads: reads.clone(),
        });
        let cancellation = MonitoringCancellation::new();
        cancellation.cancel();
        let (sender, receiver) = mpsc::sync_channel(1);

        let observations = collect_hardware_observations(
            &mut monitor,
            &[],
            std::path::Path::new("/nonexistent-daia-drm"),
            &mut FakeNvidia,
            Duration::from_secs(1),
            10,
            &cancellation,
            sender,
        )
        .expect("cancelled collector");

        assert!(observations.samples.is_empty());
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(receiver.recv().unwrap().is_err());
    }

    #[test]
    fn threaded_monitor_returns_first_sample_before_inference() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let reads = std::sync::Arc::new(AtomicUsize::new(0));
        let monitor = HardwareMonitor::new(FakeSource {
            fail: false,
            reads: reads.clone(),
        });

        let mut session = start_monitoring_thread(
            monitor,
            FakeNvidia,
            vec![],
            PathBuf::from("/nonexistent-daia-drm"),
            Duration::from_secs(60),
            100,
        )
        .expect("monitor must produce its first sample before startup completes");

        assert_eq!(reads.load(Ordering::SeqCst), 1);

        session.cancel();
        let observations = session.join().expect("cancel and join monitor");

        assert_eq!(observations.samples.len(), 1);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert_eq!(observations.samples[0].hardware.memory.used_bytes(), 4096);
    }

    #[test]
    fn dropping_session_cancels_and_joins_monitoring_thread() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let reads = std::sync::Arc::new(AtomicUsize::new(0));
        let monitor = HardwareMonitor::new(FakeSource {
            fail: false,
            reads: reads.clone(),
        });

        let session = start_monitoring_thread(
            monitor,
            FakeNvidia,
            vec![],
            PathBuf::from("/nonexistent-daia-drm"),
            Duration::from_secs(60),
            100,
        )
        .expect("monitor startup");

        assert_eq!(reads.load(Ordering::SeqCst), 1);

        drop(session);

        assert_eq!(
            reads.load(Ordering::SeqCst),
            1,
            "dropping the session must stop monitoring without another sample"
        );
    }

    #[test]
    fn threaded_monitor_propagates_first_sample_failure() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let reads = std::sync::Arc::new(AtomicUsize::new(0));
        let monitor = HardwareMonitor::new(FakeSource {
            fail: true,
            reads: reads.clone(),
        });

        let result = start_monitoring_thread(
            monitor,
            FakeNvidia,
            vec![],
            PathBuf::from("/nonexistent-daia-drm"),
            Duration::from_secs(60),
            100,
        );

        assert!(matches!(
            result,
            Err(BenchmarkError::Execution(message))
                if message.contains("synthetic CPU failure")
        ));
        assert_eq!(reads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn threaded_monitor_joins_after_reaching_sample_limit() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let reads = std::sync::Arc::new(AtomicUsize::new(0));
        let monitor = HardwareMonitor::new(FakeSource {
            fail: false,
            reads: reads.clone(),
        });

        let session = start_monitoring_thread(
            monitor,
            FakeNvidia,
            vec![],
            PathBuf::from("/nonexistent-daia-drm"),
            Duration::from_millis(1),
            2,
        )
        .expect("monitor startup");

        let observations = session.join().expect("monitor must complete");

        assert_eq!(observations.samples.len(), 2);
        assert_eq!(reads.load(Ordering::SeqCst), 2);
    }
}

impl BenchmarkMonitoringSession for BenchmarkHardwareSession {
    type Output = BenchmarkHardwareObservations;

    fn cancel(&mut self) {
        self.cancellation.cancel();
    }

    fn join(mut self) -> Result<Self::Output, BenchmarkError> {
        self.thread
            .take()
            .expect("monitoring thread must be owned until joined")
            .join()
            .map_err(|_| BenchmarkError::Execution("benchmark monitoring thread panicked".into()))?
    }
}

impl Drop for BenchmarkHardwareSession {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
