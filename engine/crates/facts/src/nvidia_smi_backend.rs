//! Read-only NVIDIA SMI telemetry backend.
//!
//! Command execution is injectable for deterministic tests.
//! Queries never modify driver or GPU configuration.

use std::fs::{self, OpenOptions};
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::nvidia_gpu_telemetry::{NvidiaGpuMetrics, NvidiaTelemetryBackend};
use crate::nvidia_smi_telemetry::{normalize_pci_identifier, parse_nvidia_smi_output};

const QUERY: &str = "--query-gpu=pci.bus_id,utilization.gpu,memory.used,memory.total,temperature.gpu,clocks.current.graphics,power.draw";
const FORMAT: &str = "--format=csv,noheader,nounits";

/// Executes one fixed NVIDIA SMI telemetry query.
pub trait NvidiaSmiCommandRunner {
    fn query(&mut self) -> io::Result<String>;
}

const DEFAULT_NVIDIA_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
static NEXT_OUTPUT_FILE: AtomicU64 = AtomicU64::new(0);

struct TemporaryOutput {
    path: PathBuf,
}

impl TemporaryOutput {
    fn new() -> io::Result<(Self, std::fs::File)> {
        for _ in 0..32 {
            let sequence = NEXT_OUTPUT_FILE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "daia-nvidia-smi-{}-{sequence}.tmp",
                std::process::id()
            ));

            match OpenOptions::new()
                .write(true)
                .read(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => return Ok((Self { path }, file)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }

        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "unable to allocate NVIDIA telemetry output file",
        ))
    }
}

impl Drop for TemporaryOutput {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Executes a bounded, read-only command and captures stdout.
///
/// The child is terminated and reaped on timeout. Output is redirected
/// to a file to avoid blocking on an unread stdout pipe.
///
/// # Errors
/// Returns an error for spawn, timeout, execution, or output failures.
fn run_bounded_command(command: &mut Command, timeout: Duration) -> io::Result<String> {
    if timeout.is_zero() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command timeout must be positive",
        ));
    }

    let (_temporary, output_file) = TemporaryOutput::new()?;

    command
        .stdout(Stdio::from(output_file))
        .stderr(Stdio::null());

    let mut child = command.spawn()?;
    let started = Instant::now();

    loop {
        match child.try_wait()? {
            Some(status) => {
                if !status.success() {
                    return Err(io::Error::other("NVIDIA telemetry command failed"));
                }

                return fs::read_to_string(&_temporary.path);
            }
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                child.wait()?;

                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "NVIDIA telemetry command timed out",
                ));
            }
            None => {
                std::thread::sleep(POLL_INTERVAL.min(timeout.saturating_sub(started.elapsed())));
            }
        }
    }
}

/// System command runner for read-only NVIDIA SMI queries.
#[derive(Clone, Copy, Debug)]
pub struct SystemNvidiaSmiCommandRunner {
    timeout: Duration,
}

impl Default for SystemNvidiaSmiCommandRunner {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_NVIDIA_TIMEOUT,
        }
    }
}

impl SystemNvidiaSmiCommandRunner {
    #[must_use]
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl NvidiaSmiCommandRunner for SystemNvidiaSmiCommandRunner {
    fn query(&mut self) -> io::Result<String> {
        let mut command = Command::new("nvidia-smi");
        command.args([QUERY, FORMAT]);

        run_bounded_command(&mut command, self.timeout)
    }
}

/// NVIDIA telemetry backend using an injectable command runner.
pub struct NvidiaSmiBackend<R> {
    runner: R,
}

impl<R> NvidiaSmiBackend<R> {
    #[must_use]
    pub fn new(runner: R) -> Self {
        Self { runner }
    }
}

impl<R: NvidiaSmiCommandRunner> NvidiaTelemetryBackend for NvidiaSmiBackend<R> {
    type Error = io::Error;

    fn sample(&mut self, pci_identifier: &str) -> Result<NvidiaGpuMetrics, Self::Error> {
        let requested = normalize_pci_identifier(pci_identifier).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "invalid NVIDIA PCI identifier")
        })?;

        let output = self.runner.query()?;
        let observations = parse_nvidia_smi_output(&output);

        observations
            .into_iter()
            .find(|observation| observation.pci_identifier == requested)
            .map(|observation| observation.metrics)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "requested NVIDIA GPU not present in telemetry",
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeRunner {
        output: io::Result<String>,
        calls: usize,
    }

    impl NvidiaSmiCommandRunner for FakeRunner {
        fn query(&mut self) -> io::Result<String> {
            self.calls += 1;

            match &self.output {
                Ok(output) => Ok(output.clone()),
                Err(error) => Err(io::Error::new(error.kind(), error.to_string())),
            }
        }
    }

    #[test]
    fn bounded_command_rejects_zero_timeout() {
        let mut command = Command::new("sh");
        command.args(["-c", "exit 0"]);

        let error = run_bounded_command(&mut command, Duration::ZERO).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn bounded_command_returns_successful_stdout() {
        let mut command = Command::new("sh");
        command.args(["-c", "printf 'gpu telemetry\n'"]);

        let output = run_bounded_command(&mut command, Duration::from_secs(2)).unwrap();

        assert_eq!(output, "gpu telemetry\n");
    }

    #[test]
    fn bounded_command_reports_nonzero_exit() {
        let mut command = Command::new("sh");
        command.args(["-c", "exit 7"]);

        let error = run_bounded_command(&mut command, Duration::from_secs(2)).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn bounded_command_times_out() {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 2"]);

        let started = Instant::now();
        let error = run_bounded_command(&mut command, Duration::from_millis(50)).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn matches_requested_pci_gpu_not_first_gpu() {
        let output = concat!(
            "00000000:01:00.0, 20, 100, 4000, 50, 900, 20\n",
            "00000000:02:00.0, 70, 200, 8000, 65, 1500, 60\n",
        );

        let mut backend = NvidiaSmiBackend::new(FakeRunner {
            output: Ok(output.to_owned()),
            calls: 0,
        });

        let result = backend.sample("0000:02:00.0").unwrap();

        assert_eq!(result.utilization_percent, Some(70.0));
        assert_eq!(result.frequency_mhz, Some(1500));
        assert_eq!(backend.runner.calls, 1);
    }

    #[test]
    fn missing_gpu_is_not_assigned_another_gpu_metrics() {
        let mut backend = NvidiaSmiBackend::new(FakeRunner {
            output: Ok("00000000:01:00.0, 20, 100, 4000, 50, 900, 20\n".to_owned()),
            calls: 0,
        });

        let error = backend.sample("0000:03:00.0").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn command_failure_is_reported() {
        let mut backend = NvidiaSmiBackend::new(FakeRunner {
            output: Err(io::Error::new(
                io::ErrorKind::NotFound,
                "nvidia-smi unavailable",
            )),
            calls: 0,
        });

        assert!(backend.sample("0000:01:00.0").is_err());
        assert_eq!(backend.runner.calls, 1);
    }

    #[test]
    fn invalid_pci_identifier_does_not_execute_command() {
        let mut backend = NvidiaSmiBackend::new(FakeRunner {
            output: Ok(String::new()),
            calls: 0,
        });

        let error = backend.sample("invalid").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(backend.runner.calls, 0);
    }

    #[test]
    fn unavailable_metrics_remain_none() {
        let mut backend = NvidiaSmiBackend::new(FakeRunner {
            output: Ok("00000000:01:00.0, N/A, N/A, 4096, N/A, 350, N/A\n".to_owned()),
            calls: 0,
        });

        let result = backend.sample("0000:01:00.0").unwrap();

        assert_eq!(result.utilization_percent, None);
        assert_eq!(result.memory_used_bytes, None);
        assert_eq!(result.memory_total_bytes, Some(4096 * 1024 * 1024));
        assert_eq!(result.frequency_mhz, Some(350));
    }
}
