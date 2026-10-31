//! Domain contracts for reproducible DAIA inference benchmarks.

use crate::InferenceEngineId;

/// Configuration applied to each inference run.
#[derive(Clone, Debug, PartialEq)]
pub struct BenchmarkConfiguration {
    pub context_length: u32,
    pub threads: u32,
    pub max_output_tokens: u32,
    pub temperature: f32,
    pub seed: u64,
    pub measured_repetitions: u32,
}

impl BenchmarkConfiguration {
    /// Validates settings before executing inference.
    pub fn validate(&self) -> Result<(), BenchmarkError> {
        if self.context_length == 0 {
            return Err(BenchmarkError::InvalidConfiguration("context_length"));
        }
        if self.threads == 0 {
            return Err(BenchmarkError::InvalidConfiguration("threads"));
        }
        if self.max_output_tokens == 0 {
            return Err(BenchmarkError::InvalidConfiguration("max_output_tokens"));
        }
        if !self.temperature.is_finite() || self.temperature < 0.0 {
            return Err(BenchmarkError::InvalidConfiguration("temperature"));
        }
        if self.measured_repetitions == 0 {
            return Err(BenchmarkError::InvalidConfiguration("measured_repetitions"));
        }
        Ok(())
    }
}

/// An inference benchmark requested for an installed model.
#[derive(Clone, Debug, PartialEq)]
pub struct BenchmarkRequest {
    pub model_name: String,
    pub engine: InferenceEngineId,
    pub prompt: String,
    pub configuration: BenchmarkConfiguration,
}

impl BenchmarkRequest {
    /// Validates the benchmark request and configuration.
    pub fn validate(&self) -> Result<(), BenchmarkError> {
        if self.model_name.trim().is_empty() {
            return Err(BenchmarkError::InvalidConfiguration("model_name"));
        }
        if self.prompt.trim().is_empty() {
            return Err(BenchmarkError::InvalidConfiguration("prompt"));
        }
        self.configuration.validate()
    }
}

/// Whether a run warms up the model or contributes to measurements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenchmarkRunKind {
    Warmup,
    Measured,
}

/// Ollama timing and token counters from one completed inference response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenchmarkRun {
    pub kind: BenchmarkRunKind,
    pub prompt_eval_count: u64,
    pub eval_count: u64,
    pub prompt_eval_duration_ns: u64,
    pub eval_duration_ns: u64,
    pub total_duration_ns: u64,
    pub load_duration_ns: u64,
    pub done_reason: Option<String>,
}

/// A benchmark result retaining its request and individual runs.
#[derive(Clone, Debug, PartialEq)]
pub struct BenchmarkResult {
    pub request: BenchmarkRequest,
    pub runs: Vec<BenchmarkRun>,
}

/// Errors that may be reported by benchmark validation or execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BenchmarkError {
    InvalidConfiguration(&'static str),
    Execution(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configuration() -> BenchmarkConfiguration {
        BenchmarkConfiguration {
            context_length: 8192,
            threads: 8,
            max_output_tokens: 96,
            temperature: 0.0,
            seed: 42,
            measured_repetitions: 2,
        }
    }

    #[test]
    fn accepts_physical_device_benchmark_configuration() {
        assert_eq!(configuration().validate(), Ok(()));
    }

    #[test]
    fn rejects_zero_context_length() {
        let mut config = configuration();
        config.context_length = 0;
        assert_eq!(
            config.validate(),
            Err(BenchmarkError::InvalidConfiguration("context_length"))
        );
    }

    #[test]
    fn rejects_invalid_temperature() {
        let mut config = configuration();
        config.temperature = f32::NAN;
        assert_eq!(
            config.validate(),
            Err(BenchmarkError::InvalidConfiguration("temperature"))
        );
    }

    #[test]
    fn rejects_zero_repetitions() {
        let mut config = configuration();
        config.measured_repetitions = 0;
        assert_eq!(
            config.validate(),
            Err(BenchmarkError::InvalidConfiguration("measured_repetitions"))
        );
    }

    #[test]
    fn rejects_empty_model_name() {
        let request = BenchmarkRequest {
            model_name: "  ".into(),
            engine: InferenceEngineId::ollama(),
            prompt: "Explain CPU scheduling".into(),
            configuration: configuration(),
        };
        assert_eq!(
            request.validate(),
            Err(BenchmarkError::InvalidConfiguration("model_name"))
        );
    }

    #[test]
    fn preserves_raw_ollama_measurements() {
        let run = BenchmarkRun {
            kind: BenchmarkRunKind::Measured,
            prompt_eval_count: 50,
            eval_count: 96,
            prompt_eval_duration_ns: 100,
            eval_duration_ns: 3_500_000_000,
            total_duration_ns: 3_600_000_000,
            load_duration_ns: 0,
            done_reason: Some("length".into()),
        };
        assert_eq!(run.eval_count, 96);
        assert_eq!(run.done_reason.as_deref(), Some("length"));
        assert_eq!(run.kind, BenchmarkRunKind::Measured);
    }
}
