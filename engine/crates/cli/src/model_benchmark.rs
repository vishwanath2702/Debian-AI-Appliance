//! CLI entry point and presentation for Ollama model benchmarks.

use engine::OllamaBenchmarkRunner;
use engine::{BenchmarkExecutor, BenchmarkHardwareObservations, BenchmarkHardwareWorker};
use model::{
    BenchmarkConfiguration, BenchmarkError, BenchmarkRequest, BenchmarkResult, BenchmarkRunKind,
    InferenceEngineId,
};
use std::process::ExitCode;

const DEFAULT_PROMPT: &str = "Explain how a computer operating system manages CPU scheduling, memory, and storage. Use three short paragraphs.";

fn benchmark_request(model_name: &str) -> BenchmarkRequest {
    BenchmarkRequest {
        model_name: model_name.to_owned(),
        engine: InferenceEngineId::ollama(),
        prompt: DEFAULT_PROMPT.to_owned(),
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

fn tokens_per_second(tokens: u64, duration_ns: u64) -> Option<f64> {
    if duration_ns == 0 {
        return None;
    }
    Some(tokens as f64 * 1_000_000_000.0 / duration_ns as f64)
}

fn format_rate(rate: Option<f64>) -> String {
    match rate {
        Some(rate) => format!("{rate:.2} tok/s"),
        None => "N/A".to_owned(),
    }
}

fn format_result(result: &BenchmarkResult) -> String {
    let configuration = &result.request.configuration;
    let mut output = format!(
        "DAIA Model Benchmark\n\
         Model: {}\n\
         Engine: {}\n\
         Context: {}\n\
         Threads: {}\n\
         Max output tokens: {}\n\
         Temperature: {}\n\
         Seed: {}\n\
         Measured repetitions: {}\n",
        result.request.model_name,
        result.request.engine,
        configuration.context_length,
        configuration.threads,
        configuration.max_output_tokens,
        configuration.temperature,
        configuration.seed,
        configuration.measured_repetitions,
    );

    let mut measured_generation_rates = Vec::new();

    for (index, run) in result.runs.iter().enumerate() {
        let generation_rate = tokens_per_second(run.eval_count, run.eval_duration_ns);
        let prompt_rate = tokens_per_second(run.prompt_eval_count, run.prompt_eval_duration_ns);

        if run.kind == BenchmarkRunKind::Measured {
            if let Some(rate) = generation_rate {
                measured_generation_rates.push(rate);
            }
        }

        let label = match run.kind {
            BenchmarkRunKind::Warmup => "warmup",
            BenchmarkRunKind::Measured => "measured",
        };

        output.push_str(&format!(
            "\nRun {} ({})\n\
             Prompt tokens: {}\n\
             Generated tokens: {}\n\
             Prompt evaluation (ns): {}\n\
             Generation evaluation (ns): {}\n\
             Total duration (ns): {}\n\
             Load duration (ns): {}\n\
             Done reason: {}\n\
             Prompt throughput: {}\n\
             Generation throughput: {}\n\
             Token-limit stop: {}\n",
            index + 1,
            label,
            run.prompt_eval_count,
            run.eval_count,
            run.prompt_eval_duration_ns,
            run.eval_duration_ns,
            run.total_duration_ns,
            run.load_duration_ns,
            run.done_reason.as_deref().unwrap_or("not reported"),
            format_rate(prompt_rate),
            format_rate(generation_rate),
            if run.done_reason.as_deref() == Some("length") {
                "yes"
            } else {
                "no"
            },
        ));
    }

    let average = if measured_generation_rates.is_empty() {
        None
    } else {
        Some(measured_generation_rates.iter().sum::<f64>() / measured_generation_rates.len() as f64)
    };

    output.push_str(&format!(
        "\nMeasured average generation throughput: {}\n",
        format_rate(average),
    ));

    output
}

fn format_hardware_observations(observations: &BenchmarkHardwareObservations) -> String {
    let mut output = format!(
        "\nHardware observations\nSamples: {}\n",
        observations.samples.len()
    );

    let cpu = observations
        .samples
        .iter()
        .rev()
        .find_map(|sample| sample.hardware.cpu_utilization_percent);

    match cpu {
        Some(value) => output.push_str(&format!(
            "CPU utilization (latest available): {value:.2}%\n"
        )),
        None => output.push_str("CPU utilization: N/A\n"),
    }

    if let Some(sample) = observations.samples.last() {
        output.push_str(&format!(
            "System RAM used (latest): {} bytes\n",
            sample.hardware.memory.used_bytes()
        ));

        for gpu in &sample.gpus.devices {
            output.push_str(&format!(
                "GPU {} ({:?})\n",
                gpu.gpu.identifier(),
                gpu.vendor
            ));

            // A GPU may be sampled less frequently than CPU/RAM.
            // Select each metric from the latest available observation
            // of this same PCI device, never from another GPU.
            let mut metrics = gpu.metrics.clone();

            for earlier in observations.samples.iter().rev().skip(1) {
                let Some(previous) = earlier
                    .gpus
                    .devices
                    .iter()
                    .find(|device| device.gpu.identifier() == gpu.gpu.identifier())
                else {
                    continue;
                };

                if metrics.utilization_percent.is_none() {
                    metrics.utilization_percent = previous.metrics.utilization_percent;
                }
                if metrics.memory_used_bytes.is_none() {
                    metrics.memory_used_bytes = previous.metrics.memory_used_bytes;
                }
                if metrics.memory_total_bytes.is_none() {
                    metrics.memory_total_bytes = previous.metrics.memory_total_bytes;
                }
                if metrics.temperature_celsius.is_none() {
                    metrics.temperature_celsius = previous.metrics.temperature_celsius;
                }
                if metrics.frequency_mhz.is_none() {
                    metrics.frequency_mhz = previous.metrics.frequency_mhz;
                }
                if metrics.power_watts.is_none() {
                    metrics.power_watts = previous.metrics.power_watts;
                }
            }
            let percentage = metrics
                .utilization_percent
                .map(|value| format!("{value:.2}%"))
                .unwrap_or_else(|| "N/A".into());
            let memory = metrics
                .memory_used_bytes
                .map(|value| format!("{value} bytes"))
                .unwrap_or_else(|| "N/A".into());
            let temperature = metrics
                .temperature_celsius
                .map(|value| format!("{value:.2} C"))
                .unwrap_or_else(|| "N/A".into());
            let frequency = metrics
                .frequency_mhz
                .map(|value| format!("{value} MHz"))
                .unwrap_or_else(|| "N/A".into());
            let power = metrics
                .power_watts
                .map(|value| format!("{value:.2} W"))
                .unwrap_or_else(|| "N/A".into());

            output.push_str(&format!(
                "  Utilization: {percentage}\n  VRAM used: {memory}\n  Temperature: {temperature}\n  Clock frequency: {frequency}\n  Power: {power}\n"
            ));
        }
    } else {
        output.push_str("System RAM used: N/A\n");
    }

    output
}

pub fn run(model_name: &str) -> ExitCode {
    let request = benchmark_request(model_name);

    if let Err(error) = request.validate() {
        eprintln!("Invalid benchmark request: {error:?}");
        return ExitCode::FAILURE;
    }

    let port = match benchmark_port(std::env::var("DAIA_BENCHMARK_OLLAMA_PORT").ok().as_deref()) {
        Ok(port) => port,
        Err(error) => {
            print_error(&error);
            return ExitCode::FAILURE;
        }
    };

    let mut executor = BenchmarkExecutor::new(OllamaBenchmarkRunner::local_port(port));

    let worker = match BenchmarkHardwareWorker::local() {
        Ok(worker) => worker,
        Err(error) => {
            print_error(&error);
            return ExitCode::FAILURE;
        }
    };

    match executor.execute_with_monitoring(&request, worker) {
        Ok((result, observations)) => {
            print!("{}", format_result(&result));
            print!("{}", format_hardware_observations(&observations));
            ExitCode::SUCCESS
        }
        Err(error) => {
            print_error(&error);
            ExitCode::FAILURE
        }
    }
}

fn benchmark_port(value: Option<&str>) -> Result<u16, BenchmarkError> {
    match value {
        None => Ok(11434),
        Some(value) => value
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or(BenchmarkError::InvalidConfiguration("ollama_port")),
    }
}

fn print_error(error: &BenchmarkError) {
    eprintln!("Model benchmark failed: {error:?}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::BenchmarkRun;

    #[test]
    fn hardware_formatter_selects_latest_available_metrics_per_gpu() {
        use engine::hardware_monitor::{HardwareSample, HardwareSampleWithGpus};
        use facts::{
            GpuFacts,
            gpu_telemetry::{GpuMetrics, GpuTelemetrySample, GpuTelemetrySnapshot},
            telemetry::MemoryUsage,
        };

        let gpu_a = GpuFacts::new("0000:01:00.0", "0x10de", "0x1fb2");
        let gpu_b = GpuFacts::new("0000:02:00.0", "0x10de", "0x2204");

        let make_sample = |a: GpuMetrics, b: GpuMetrics| HardwareSampleWithGpus {
            hardware: HardwareSample {
                cpu_utilization_percent: Some(40.0),
                memory: MemoryUsage {
                    total_bytes: 8192,
                    available_bytes: 4096,
                },
            },
            gpus: GpuTelemetrySnapshot {
                devices: vec![
                    GpuTelemetrySample::new(gpu_a.clone(), a),
                    GpuTelemetrySample::new(gpu_b.clone(), b),
                ],
            },
        };

        let observations = BenchmarkHardwareObservations {
            samples: vec![
                make_sample(
                    GpuMetrics {
                        utilization_percent: Some(25.0),
                        memory_used_bytes: Some(1024),
                        ..GpuMetrics::default()
                    },
                    GpuMetrics {
                        utilization_percent: Some(80.0),
                        memory_used_bytes: Some(8192),
                        temperature_celsius: Some(70.0),
                        ..GpuMetrics::default()
                    },
                ),
                make_sample(
                    GpuMetrics {
                        utilization_percent: Some(45.0),
                        ..GpuMetrics::default()
                    },
                    GpuMetrics::default(),
                ),
                make_sample(GpuMetrics::default(), GpuMetrics::default()),
            ],
        };

        let text = format_hardware_observations(&observations);

        let first = text
            .split("GPU 0000:01:00.0 (Nvidia)\n")
            .nth(1)
            .expect("first GPU section");
        let (first, second) = first
            .split_once("GPU 0000:02:00.0 (Nvidia)\n")
            .expect("second GPU section");

        assert!(first.contains("Utilization: 45.00%\n"));
        assert!(first.contains("VRAM used: 1024 bytes\n"));
        assert!(first.contains("Temperature: N/A\n"));
        assert!(!first.contains("80.00%"));
        assert!(!first.contains("8192 bytes"));

        assert!(second.contains("Utilization: 80.00%\n"));
        assert!(second.contains("VRAM used: 8192 bytes\n"));
        assert!(second.contains("Temperature: 70.00 C\n"));
        assert!(second.contains("Power: N/A\n"));
        assert!(!second.contains("45.00%"));
    }

    #[test]
    fn hardware_formatter_reports_missing_measurements() {
        let observations = BenchmarkHardwareObservations::default();
        let text = format_hardware_observations(&observations);

        assert!(text.contains("\nHardware observations\nSamples: 0\n"));
        assert!(text.contains("CPU utilization: N/A\n"));
        assert!(text.contains("System RAM used: N/A\n"));
        assert!(!text.contains("\\n"));
    }

    #[test]
    fn hardware_formatter_reports_cpu_and_ram_without_gpus() {
        use engine::hardware_monitor::{HardwareSample, HardwareSampleWithGpus};
        use facts::gpu_telemetry::GpuTelemetrySnapshot;
        use facts::telemetry::MemoryUsage;

        let observations = BenchmarkHardwareObservations {
            samples: vec![HardwareSampleWithGpus {
                hardware: HardwareSample {
                    cpu_utilization_percent: Some(37.5),
                    memory: MemoryUsage {
                        total_bytes: 8192,
                        available_bytes: 3072,
                    },
                },
                gpus: GpuTelemetrySnapshot::default(),
            }],
        };

        let text = format_hardware_observations(&observations);

        assert!(text.contains("Samples: 1\n"));
        assert!(text.contains("CPU utilization (latest available): 37.50%\n"));
        assert!(text.contains("System RAM used (latest): 5120 bytes\n"));
        assert!(!text.contains("GPU "));
        assert!(!text.contains("\\n"));
    }

    #[test]
    fn hardware_formatter_preserves_multiple_gpu_identities() {
        use engine::hardware_monitor::{HardwareSample, HardwareSampleWithGpus};
        use facts::{
            GpuFacts,
            gpu_telemetry::{GpuMetrics, GpuTelemetrySample, GpuTelemetrySnapshot},
            telemetry::MemoryUsage,
        };

        let intel = GpuTelemetrySample::new(
            GpuFacts::new("0000:00:02.0", "0x8086", "0x5912"),
            GpuMetrics {
                frequency_mhz: Some(350),
                ..GpuMetrics::default()
            },
        );

        let nvidia = GpuTelemetrySample::new(
            GpuFacts::new("0000:01:00.0", "0x10de", "0x1fb2"),
            GpuMetrics {
                utilization_percent: Some(72.5),
                memory_used_bytes: Some(2_147_483_648),
                temperature_celsius: Some(64.0),
                frequency_mhz: Some(1500),
                power_watts: Some(85.5),
                ..GpuMetrics::default()
            },
        );

        let observations = BenchmarkHardwareObservations {
            samples: vec![HardwareSampleWithGpus {
                hardware: HardwareSample {
                    cpu_utilization_percent: Some(25.0),
                    memory: MemoryUsage {
                        total_bytes: 16_384,
                        available_bytes: 4096,
                    },
                },
                gpus: GpuTelemetrySnapshot {
                    devices: vec![intel, nvidia],
                },
            }],
        };

        let text = format_hardware_observations(&observations);

        assert!(text.contains("GPU 0000:00:02.0 (Intel)\n"));
        assert!(text.contains("GPU 0000:01:00.0 (Nvidia)\n"));

        let intel_section = text
            .split("GPU 0000:00:02.0 (Intel)\n")
            .nth(1)
            .expect("Intel section")
            .split("GPU 0000:01:00.0 (Nvidia)\n")
            .next()
            .expect("Intel metrics");

        assert!(intel_section.contains("Utilization: N/A\n"));
        assert!(intel_section.contains("Clock frequency: 350 MHz\n"));

        let nvidia_section = text
            .split("GPU 0000:01:00.0 (Nvidia)\n")
            .nth(1)
            .expect("NVIDIA section");

        assert!(nvidia_section.contains("Utilization: 72.50%\n"));
        assert!(nvidia_section.contains("VRAM used: 2147483648 bytes\n"));
        assert!(nvidia_section.contains("Temperature: 64.00 C\n"));
        assert!(nvidia_section.contains("Clock frequency: 1500 MHz\n"));
        assert!(nvidia_section.contains("Power: 85.50 W\n"));
        assert!(!text.contains("\\n"));
    }

    #[test]
    fn hardware_formatter_keeps_unavailable_gpu_metrics_explicit() {
        use engine::hardware_monitor::{HardwareSample, HardwareSampleWithGpus};
        use facts::{
            GpuFacts,
            gpu_telemetry::{GpuMetrics, GpuTelemetrySample, GpuTelemetrySnapshot},
            telemetry::MemoryUsage,
        };

        let observations = BenchmarkHardwareObservations {
            samples: vec![HardwareSampleWithGpus {
                hardware: HardwareSample {
                    cpu_utilization_percent: None,
                    memory: MemoryUsage {
                        total_bytes: 8192,
                        available_bytes: 4096,
                    },
                },
                gpus: GpuTelemetrySnapshot {
                    devices: vec![GpuTelemetrySample::new(
                        GpuFacts::new("0000:03:00.0", "0x1002", "0x1234"),
                        GpuMetrics::default(),
                    )],
                },
            }],
        };

        let text = format_hardware_observations(&observations);

        assert!(text.contains("GPU 0000:03:00.0 (Amd)\n"));
        assert!(text.contains("CPU utilization: N/A\n"));
        assert!(text.contains("  Utilization: N/A\n"));
        assert!(text.contains("  VRAM used: N/A\n"));
        assert!(text.contains("  Temperature: N/A\n"));
        assert!(text.contains("  Clock frequency: N/A\n"));
        assert!(text.contains("  Power: N/A\n"));
        assert!(!text.contains("Utilization: 0.00%"));
        assert!(!text.contains("\\n"));
    }

    #[test]
    fn benchmark_port_defaults_and_validates_override() {
        assert_eq!(benchmark_port(None), Ok(11434));
        assert_eq!(benchmark_port(Some("23456")), Ok(23456));
        assert_eq!(
            benchmark_port(Some("0")),
            Err(BenchmarkError::InvalidConfiguration("ollama_port"))
        );
        assert_eq!(
            benchmark_port(Some("invalid")),
            Err(BenchmarkError::InvalidConfiguration("ollama_port"))
        );
        assert_eq!(
            benchmark_port(Some("65536")),
            Err(BenchmarkError::InvalidConfiguration("ollama_port"))
        );
    }

    #[test]
    fn calculates_throughput_and_handles_zero_duration() {
        assert_eq!(tokens_per_second(96, 4_000_000_000), Some(24.0));
        assert_eq!(tokens_per_second(50, 100_000_000), Some(500.0));
        assert_eq!(tokens_per_second(96, 0), None);
        assert_eq!(format_rate(None), "N/A");
        assert_eq!(format_rate(Some(24.0)), "24.00 tok/s");
    }

    #[test]
    fn default_request_is_valid_and_reproducible() {
        let request = benchmark_request("example:latest");

        assert!(request.validate().is_ok());
        assert_eq!(request.engine, InferenceEngineId::ollama());
        assert_eq!(request.configuration.context_length, 8192);
        assert_eq!(request.configuration.threads, 8);
        assert_eq!(request.configuration.max_output_tokens, 96);
        assert_eq!(request.configuration.temperature, 0.0);
        assert_eq!(request.configuration.seed, 42);
        assert_eq!(request.configuration.measured_repetitions, 2);
    }

    #[test]
    fn rejects_empty_model_name_without_contacting_ollama() {
        assert_eq!(run("  "), ExitCode::FAILURE);
    }

    #[test]
    fn formats_warmup_and_measured_raw_counters() {
        let request = benchmark_request("example:latest");
        let result = BenchmarkResult {
            request,
            runs: vec![
                BenchmarkRun {
                    kind: BenchmarkRunKind::Warmup,
                    prompt_eval_count: 50,
                    eval_count: 96,
                    prompt_eval_duration_ns: 100,
                    eval_duration_ns: 3_500_000_000,
                    total_duration_ns: 3_600_000_000,
                    load_duration_ns: 0,
                    done_reason: Some("length".into()),
                },
                BenchmarkRun {
                    kind: BenchmarkRunKind::Measured,
                    prompt_eval_count: 50,
                    eval_count: 96,
                    prompt_eval_duration_ns: 110,
                    eval_duration_ns: 3_400_000_000,
                    total_duration_ns: 3_500_000_000,
                    load_duration_ns: 0,
                    done_reason: Some("length".into()),
                },
            ],
        };

        let output = format_result(&result);

        assert!(output.contains("Model: example:latest"));
        assert!(output.contains("Run 1 (warmup)"));
        assert!(output.contains("Run 2 (measured)"));
        assert!(output.contains("Generation evaluation (ns): 3400000000"));
        assert!(output.contains("Done reason: length"));
        assert!(output.contains("Token-limit stop: yes"));
        assert!(output.contains("Generation throughput: 27.43 tok/s"));
        assert!(output.contains("Generation throughput: 28.24 tok/s"));
        assert!(output.contains("Measured average generation throughput: 28.24 tok/s"));
    }

    #[test]
    fn zero_duration_reports_unavailable_throughput() {
        let request = benchmark_request("example:latest");
        let result = BenchmarkResult {
            request,
            runs: vec![BenchmarkRun {
                kind: BenchmarkRunKind::Measured,
                prompt_eval_count: 50,
                eval_count: 96,
                prompt_eval_duration_ns: 0,
                eval_duration_ns: 0,
                total_duration_ns: 0,
                load_duration_ns: 0,
                done_reason: None,
            }],
        };

        let output = format_result(&result);
        assert!(output.contains("Prompt throughput: N/A"));
        assert!(output.contains("Generation throughput: N/A"));
        assert!(output.contains("Token-limit stop: no"));
        assert!(output.contains("Measured average generation throughput: N/A"));
    }
}
