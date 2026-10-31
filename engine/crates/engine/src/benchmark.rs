//! Execution boundary for DAIA model benchmarks.

use model::{BenchmarkError, BenchmarkRequest, BenchmarkResult, BenchmarkRun};

/// Runs one inference request and returns its raw measurements.
///
/// Implementations may communicate with a local inference service.
/// Tests can substitute an in-memory implementation.
pub trait BenchmarkInferenceRunner {
    /// Performs a single inference run.
    ///
    /// # Errors
    ///
    /// Returns a benchmark error if inference cannot be completed.
    fn run(
        &mut self,
        request: &BenchmarkRequest,
        kind: model::BenchmarkRunKind,
    ) -> Result<BenchmarkRun, BenchmarkError>;
}

/// A monitoring session that can be cancelled and joined.
pub trait BenchmarkMonitoringSession {
    type Output;

    fn cancel(&mut self);

    /// Waits for monitoring to finish and returns its observations.
    ///
    /// # Errors
    /// Returns a benchmark error if monitoring cannot complete.
    fn join(self) -> Result<Self::Output, BenchmarkError>;
}

/// Starts an independently managed benchmark monitoring session.
pub trait BenchmarkMonitoringWorker {
    type Output;
    type Session: BenchmarkMonitoringSession<Output = Self::Output>;

    /// Starts monitoring before inference begins.
    ///
    /// # Errors
    /// Returns a benchmark error if monitoring cannot start.
    fn start(self) -> Result<Self::Session, BenchmarkError>;
}

/// Coordinates warm-up and measured benchmark runs.
pub struct BenchmarkExecutor<R> {
    runner: R,
}

impl<R> BenchmarkExecutor<R>
where
    R: BenchmarkInferenceRunner,
{
    /// Creates a benchmark executor with a supplied inference runner.
    #[must_use]
    pub const fn new(runner: R) -> Self {
        Self { runner }
    }

    /// Runs inference while an independently supplied monitoring worker executes.
    ///
    /// The worker is cancelled and joined on both successful and failed inference.
    /// Inference errors take precedence over monitoring errors.
    ///
    /// # Errors
    /// Returns benchmark validation, inference, or monitoring errors.
    pub fn execute_with_monitoring<M>(
        &mut self,
        request: &BenchmarkRequest,
        monitor: M,
    ) -> Result<(BenchmarkResult, M::Output), BenchmarkError>
    where
        M: BenchmarkMonitoringWorker,
    {
        request.validate()?;

        let mut monitoring = monitor.start()?;
        let inference = self.execute(request);

        monitoring.cancel();
        let monitoring_result = monitoring.join();

        match inference {
            Err(error) => Err(error),
            Ok(result) => monitoring_result.map(|observations| (result, observations)),
        }
    }

    /// Runs one warm-up followed by configured measured repetitions.
    ///
    /// # Errors
    ///
    /// Returns validation or inference errors without fabricating results.
    pub fn execute(
        &mut self,
        request: &BenchmarkRequest,
    ) -> Result<BenchmarkResult, BenchmarkError> {
        request.validate()?;

        let mut runs = Vec::new();

        for kind in std::iter::once(model::BenchmarkRunKind::Warmup).chain(std::iter::repeat_n(
            model::BenchmarkRunKind::Measured,
            request.configuration.measured_repetitions as usize,
        )) {
            let run = self.runner.run(request, kind)?;
            if run.kind != kind {
                return Err(BenchmarkError::Execution(
                    "inference runner returned an unexpected run kind".into(),
                ));
            }
            runs.push(run);
        }

        Ok(BenchmarkResult {
            request: request.clone(),
            runs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::{BenchmarkConfiguration, BenchmarkRunKind, InferenceEngineId};

    struct FakeRunner {
        calls: Vec<BenchmarkRunKind>,
    }

    impl BenchmarkInferenceRunner for FakeRunner {
        fn run(
            &mut self,
            _request: &BenchmarkRequest,
            kind: BenchmarkRunKind,
        ) -> Result<BenchmarkRun, BenchmarkError> {
            self.calls.push(kind);

            Ok(BenchmarkRun {
                kind,
                prompt_eval_count: 10,
                eval_count: 96,
                prompt_eval_duration_ns: 100,
                eval_duration_ns: 1_000_000_000,
                total_duration_ns: 1_100_000_000,
                load_duration_ns: 0,
                done_reason: Some("length".into()),
            })
        }
    }

    fn request() -> BenchmarkRequest {
        BenchmarkRequest {
            model_name: "example:latest".into(),
            engine: InferenceEngineId::ollama(),
            prompt: "Explain CPU scheduling".into(),
            configuration: BenchmarkConfiguration {
                context_length: 8192,
                threads: 8,
                max_output_tokens: 96,
                temperature: 0.0,
                seed: 42,
                measured_repetitions: 2,
            },
        }
    }

    #[derive(Default)]
    struct MonitorState {
        started: bool,
        cancelled: bool,
        joined: bool,
    }

    struct FakeMonitor {
        state: std::sync::Arc<std::sync::Mutex<MonitorState>>,
        fail_join: bool,
    }

    struct FakeMonitorSession {
        state: std::sync::Arc<std::sync::Mutex<MonitorState>>,
        fail_join: bool,
    }

    impl BenchmarkMonitoringWorker for FakeMonitor {
        type Output = usize;
        type Session = FakeMonitorSession;

        fn start(self) -> Result<Self::Session, BenchmarkError> {
            self.state.lock().unwrap().started = true;
            Ok(FakeMonitorSession {
                state: self.state,
                fail_join: self.fail_join,
            })
        }
    }

    impl BenchmarkMonitoringSession for FakeMonitorSession {
        type Output = usize;

        fn cancel(&mut self) {
            self.state.lock().unwrap().cancelled = true;
        }

        fn join(self) -> Result<Self::Output, BenchmarkError> {
            let mut state = self.state.lock().unwrap();
            assert!(state.cancelled, "monitor must be cancelled before join");
            state.joined = true;

            if self.fail_join {
                Err(BenchmarkError::Execution("monitor failed".into()))
            } else {
                Ok(3)
            }
        }
    }

    fn fake_monitor(
        fail_join: bool,
    ) -> (FakeMonitor, std::sync::Arc<std::sync::Mutex<MonitorState>>) {
        let state = std::sync::Arc::new(std::sync::Mutex::new(MonitorState::default()));
        (
            FakeMonitor {
                state: state.clone(),
                fail_join,
            },
            state,
        )
    }

    #[test]
    fn monitored_execution_cancels_and_joins_on_success() {
        let mut executor = BenchmarkExecutor::new(FakeRunner { calls: Vec::new() });
        let (monitor, state) = fake_monitor(false);

        let (result, observations) = executor
            .execute_with_monitoring(&request(), monitor)
            .unwrap();

        assert_eq!(result.runs.len(), 3);
        assert_eq!(observations, 3);

        let state = state.lock().unwrap();
        assert!(state.started && state.cancelled && state.joined);
    }

    #[test]
    fn monitored_execution_cancels_and_joins_on_inference_failure() {
        struct FailingRunner;

        impl BenchmarkInferenceRunner for FailingRunner {
            fn run(
                &mut self,
                _request: &BenchmarkRequest,
                _kind: BenchmarkRunKind,
            ) -> Result<BenchmarkRun, BenchmarkError> {
                Err(BenchmarkError::Execution("inference failed".into()))
            }
        }

        let mut executor = BenchmarkExecutor::new(FailingRunner);
        let (monitor, state) = fake_monitor(true);

        assert_eq!(
            executor.execute_with_monitoring(&request(), monitor),
            Err(BenchmarkError::Execution("inference failed".into()))
        );

        let state = state.lock().unwrap();
        assert!(state.started && state.cancelled && state.joined);
    }

    #[test]
    fn monitored_execution_reports_monitoring_failure() {
        let mut executor = BenchmarkExecutor::new(FakeRunner { calls: Vec::new() });
        let (monitor, state) = fake_monitor(true);

        assert_eq!(
            executor.execute_with_monitoring(&request(), monitor),
            Err(BenchmarkError::Execution("monitor failed".into()))
        );

        let state = state.lock().unwrap();
        assert!(state.started && state.cancelled && state.joined);
    }

    #[test]
    fn invalid_request_does_not_start_monitoring() {
        let mut executor = BenchmarkExecutor::new(FakeRunner { calls: Vec::new() });
        let (monitor, state) = fake_monitor(false);
        let mut invalid = request();
        invalid.model_name.clear();

        assert_eq!(
            executor.execute_with_monitoring(&invalid, monitor),
            Err(BenchmarkError::InvalidConfiguration("model_name"))
        );

        let state = state.lock().unwrap();
        assert!(!state.started && !state.cancelled && !state.joined);
    }

    #[test]
    fn runs_warmup_then_measured_repetitions() {
        let runner = FakeRunner { calls: Vec::new() };
        let mut executor = BenchmarkExecutor::new(runner);

        let result = executor.execute(&request()).unwrap();

        assert_eq!(result.runs.len(), 3);
        assert_eq!(result.runs[0].kind, BenchmarkRunKind::Warmup);
        assert_eq!(result.runs[1].kind, BenchmarkRunKind::Measured);
        assert_eq!(result.runs[2].kind, BenchmarkRunKind::Measured);
    }

    #[test]
    fn rejects_incorrect_run_classification() {
        struct IncorrectRunner;

        impl BenchmarkInferenceRunner for IncorrectRunner {
            fn run(
                &mut self,
                _request: &BenchmarkRequest,
                _kind: BenchmarkRunKind,
            ) -> Result<BenchmarkRun, BenchmarkError> {
                Ok(BenchmarkRun {
                    kind: BenchmarkRunKind::Measured,
                    prompt_eval_count: 1,
                    eval_count: 1,
                    prompt_eval_duration_ns: 1,
                    eval_duration_ns: 1,
                    total_duration_ns: 1,
                    load_duration_ns: 0,
                    done_reason: None,
                })
            }
        }

        let mut executor = BenchmarkExecutor::new(IncorrectRunner);
        assert_eq!(
            executor.execute(&request()),
            Err(BenchmarkError::Execution(
                "inference runner returned an unexpected run kind".into()
            ))
        );
    }

    #[test]
    fn rejects_invalid_request_before_inference() {
        let runner = FakeRunner { calls: Vec::new() };
        let mut executor = BenchmarkExecutor::new(runner);

        let mut invalid = request();
        invalid.model_name.clear();

        assert_eq!(
            executor.execute(&invalid),
            Err(BenchmarkError::InvalidConfiguration("model_name"))
        );
    }
}
