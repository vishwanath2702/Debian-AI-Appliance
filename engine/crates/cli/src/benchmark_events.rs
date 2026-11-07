//! Versioned JSON-lines events for local benchmark consumers.

use engine::hardware_monitor::HardwareSampleWithGpus;
use model::{BenchmarkResult, BenchmarkRunKind};
use serde_json::{Value, json};

const SCHEMA_VERSION: u32 = 1;

pub fn started(model_name: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "event": "started",
        "model": model_name,
    })
}

pub fn hardware_sample(sample: &HardwareSampleWithGpus) -> Value {
    let gpus: Vec<Value> = sample
        .gpus
        .devices
        .iter()
        .map(|device| {
            json!({
                "identifier": device.gpu.identifier(),
                "vendor_id": device.gpu.vendor_id(),
                "device_id": device.gpu.device_id(),
                "utilization_percent": device.metrics.utilization_percent,
                "memory_used_bytes": device.metrics.memory_used_bytes,
                "memory_total_bytes": device.metrics.memory_total_bytes,
                "temperature_celsius": device.metrics.temperature_celsius,
                "frequency_mhz": device.metrics.frequency_mhz,
                "power_watts": device.metrics.power_watts,
            })
        })
        .collect();

    json!({
        "schema_version": SCHEMA_VERSION,
        "event": "hardware_sample",
        "cpu_utilization_percent": sample.hardware.cpu_utilization_percent,
        "ram_used_bytes": sample.hardware.memory.used_bytes(),
        "ram_total_bytes": sample.hardware.memory.total_bytes,
        "gpus": gpus,
    })
}

pub fn completed(result: &BenchmarkResult, sample_count: usize) -> Value {
    let runs: Vec<Value> = result
        .runs
        .iter()
        .map(|run| {
            json!({
                "kind": match run.kind {
                    BenchmarkRunKind::Warmup => "warmup",
                    BenchmarkRunKind::Measured => "measured",
                },
                "prompt_eval_count": run.prompt_eval_count,
                "eval_count": run.eval_count,
                "prompt_eval_duration_ns": run.prompt_eval_duration_ns,
                "eval_duration_ns": run.eval_duration_ns,
                "total_duration_ns": run.total_duration_ns,
                "load_duration_ns": run.load_duration_ns,
                "done_reason": run.done_reason,
            })
        })
        .collect();

    json!({
        "schema_version": SCHEMA_VERSION,
        "event": "completed",
        "model": result.request.model_name,
        "hardware_sample_count": sample_count,
        "runs": runs,
    })
}

pub fn failed(message: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "event": "failed",
        "message": message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::hardware_monitor::HardwareSample;
    use facts::{
        GpuFacts,
        gpu_telemetry::{GpuMetrics, GpuTelemetrySample, GpuTelemetrySnapshot},
        telemetry::MemoryUsage,
    };

    #[test]
    fn started_event_has_version_and_exact_model_name() {
        let event = started("namespace/model:tag");
        assert_eq!(event["schema_version"], 1);
        assert_eq!(event["event"], "started");
        assert_eq!(event["model"], "namespace/model:tag");
    }

    #[test]
    fn hardware_event_preserves_missing_values_and_multiple_gpus() {
        let sample = HardwareSampleWithGpus {
            hardware: HardwareSample {
                cpu_utilization_percent: None,
                memory: MemoryUsage {
                    total_bytes: 8192,
                    available_bytes: 4096,
                },
            },
            gpus: GpuTelemetrySnapshot {
                devices: vec![
                    GpuTelemetrySample::new(
                        GpuFacts::new("0000:01:00.0", "0x10de", "0x1fb2"),
                        GpuMetrics {
                            utilization_percent: Some(35.0),
                            memory_used_bytes: Some(1024),
                            ..GpuMetrics::default()
                        },
                    ),
                    GpuTelemetrySample::new(
                        GpuFacts::new("0000:02:00.0", "0x8086", "0x5912"),
                        GpuMetrics::default(),
                    ),
                ],
            },
        };

        let event = hardware_sample(&sample);
        assert_eq!(event["cpu_utilization_percent"], Value::Null);
        assert_eq!(event["ram_used_bytes"], 4096);
        assert_eq!(event["gpus"][0]["identifier"], "0000:01:00.0");
        assert_eq!(event["gpus"][0]["utilization_percent"], 35.0);
        assert_eq!(event["gpus"][1]["identifier"], "0000:02:00.0");
        assert_eq!(event["gpus"][1]["utilization_percent"], Value::Null);
        assert_eq!(event["gpus"][1]["memory_used_bytes"], Value::Null);
    }

    #[test]
    fn failure_event_preserves_message_as_json_string() {
        let event = failed("benchmark failed: \"offline\"");
        assert_eq!(event["event"], "failed");
        assert_eq!(event["message"], "benchmark failed: \"offline\"");
        assert!(serde_json::to_string(&event).is_ok());
    }
}
