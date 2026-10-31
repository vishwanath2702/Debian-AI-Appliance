//! Ollama inference adapter for the generic DAIA benchmark executor.

use crate::benchmark::BenchmarkInferenceRunner;
use crate::benchmark_ollama::{decode_generate_response, encode_generate_request};
use crate::benchmark_ollama_transport::{post_generate, post_generate_to_port};
use model::{BenchmarkError, BenchmarkRequest, BenchmarkRun, BenchmarkRunKind, InferenceEngineId};

/// Runs Ollama benchmarks through a replaceable transport function.
pub struct OllamaBenchmarkRunner<T> {
    transport: T,
}

impl OllamaBenchmarkRunner<fn(&[u8]) -> Result<Vec<u8>, BenchmarkError>> {
    /// Uses the local Ollama HTTP transport.
    #[must_use]
    pub fn local() -> Self {
        Self::new(post_generate)
    }
}

impl OllamaBenchmarkRunner<Box<dyn FnMut(&[u8]) -> Result<Vec<u8>, BenchmarkError>>> {
    /// Uses a selected local IPv4 loopback port.
    #[must_use]
    pub fn local_port(port: u16) -> Self {
        Self::new(Box::new(move |body| post_generate_to_port(port, body)))
    }
}

impl<T> OllamaBenchmarkRunner<T> {
    /// Creates an adapter with a supplied transport.
    #[must_use]
    pub const fn new(transport: T) -> Self {
        Self { transport }
    }
}

impl<T> BenchmarkInferenceRunner for OllamaBenchmarkRunner<T>
where
    T: FnMut(&[u8]) -> Result<Vec<u8>, BenchmarkError>,
{
    fn run(
        &mut self,
        request: &BenchmarkRequest,
        kind: BenchmarkRunKind,
    ) -> Result<BenchmarkRun, BenchmarkError> {
        request.validate()?;

        if request.engine != InferenceEngineId::ollama() {
            return Err(BenchmarkError::InvalidConfiguration("engine"));
        }

        let body = encode_generate_request(request);
        let response = (self.transport)(&body)?;
        decode_generate_response(&response, kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::BenchmarkExecutor;
    use model::BenchmarkConfiguration;
    use serde_json::{Value, json};

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

    fn response() -> Vec<u8> {
        serde_json::to_vec(&json!({
            "done": true,
            "done_reason": "length",
            "prompt_eval_count": 50,
            "eval_count": 96,
            "prompt_eval_duration": 100,
            "eval_duration": 3_500_000_000_u64,
            "total_duration": 3_600_000_000_u64,
            "load_duration": 0
        }))
        .unwrap()
    }

    #[test]
    fn executes_warmup_and_measured_runs_with_mock_transport() {
        let mut calls = 0;

        let runner = OllamaBenchmarkRunner::new(|body: &[u8]| {
            let request: Value = serde_json::from_slice(body).unwrap();
            assert_eq!(request["model"], "example:latest");
            assert_eq!(request["stream"], false);
            assert_eq!(request["options"]["num_ctx"], 8192);
            assert_eq!(request["options"]["num_thread"], 8);
            assert_eq!(request["options"]["num_predict"], 96);
            assert_eq!(request["options"]["seed"], 42);
            calls += 1;
            Ok(response())
        });

        let mut executor = BenchmarkExecutor::new(runner);
        let result = executor.execute(&request()).unwrap();

        assert_eq!(calls, 3);
        assert_eq!(result.runs.len(), 3);
        assert_eq!(result.runs[0].kind, BenchmarkRunKind::Warmup);
        assert_eq!(result.runs[1].kind, BenchmarkRunKind::Measured);
        assert_eq!(result.runs[2].kind, BenchmarkRunKind::Measured);
        assert_eq!(result.runs[2].eval_count, 96);
        assert_eq!(result.runs[2].eval_duration_ns, 3_500_000_000);
    }

    #[test]
    fn rejects_non_ollama_engine_without_transport_call() {
        let mut request = request();
        request.engine = InferenceEngineId::llama_cpp();

        let runner = OllamaBenchmarkRunner::new(|_: &[u8]| panic!("transport must not be called"));

        let mut executor = BenchmarkExecutor::new(runner);

        assert_eq!(
            executor.execute(&request),
            Err(BenchmarkError::InvalidConfiguration("engine"))
        );
    }

    #[test]
    fn propagates_transport_failure_without_fabricating_runs() {
        let runner = OllamaBenchmarkRunner::new(|_: &[u8]| {
            Err(BenchmarkError::Execution("mock connection failed".into()))
        });

        let mut executor = BenchmarkExecutor::new(runner);

        assert_eq!(
            executor.execute(&request()),
            Err(BenchmarkError::Execution("mock connection failed".into()))
        );
    }

    #[test]
    fn rejects_incomplete_ollama_response() {
        let runner = OllamaBenchmarkRunner::new(|_: &[u8]| Ok(br#"{"done":false}"#.to_vec()));

        let mut executor = BenchmarkExecutor::new(runner);

        assert_eq!(
            executor.execute(&request()),
            Err(BenchmarkError::Execution(
                "Ollama response is not complete".into()
            ))
        );
    }
}
