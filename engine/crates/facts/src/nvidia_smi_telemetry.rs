//! Read-only parsing of NVIDIA SMI GPU telemetry.
//!
//! This module does not execute commands. It parses caller-supplied output.

use crate::nvidia_gpu_telemetry::NvidiaGpuMetrics;

/// One NVIDIA GPU's PCI identity and available measurements.
#[derive(Clone, Debug, PartialEq)]
pub struct NvidiaSmiObservation {
    pub pci_identifier: String,
    pub metrics: NvidiaGpuMetrics,
}

/// Normalizes a PCI bus identifier for exact device matching.
///
/// NVIDIA tools may use an eight-digit PCI domain, while Linux sysfs
/// commonly uses four digits.
#[must_use]
pub fn normalize_pci_identifier(value: &str) -> Option<String> {
    let parts: Vec<_> = value.trim().split(':').collect();

    if parts.len() != 3 {
        return None;
    }

    let domain = parts[0];
    let bus = parts[1];
    let device_function: Vec<_> = parts[2].split('.').collect();

    if device_function.len() != 2 {
        return None;
    }

    let device = device_function[0];
    let function = device_function[1];

    if !(4..=8).contains(&domain.len())
        || bus.len() != 2
        || device.len() != 2
        || function.len() != 1
    {
        return None;
    }

    if ![domain, bus, device, function]
        .iter()
        .all(|part| part.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return None;
    }

    let domain_number = u32::from_str_radix(domain, 16).ok()?;
    let bus_number = u8::from_str_radix(bus, 16).ok()?;
    let device_number = u8::from_str_radix(device, 16).ok()?;
    let function_number = u8::from_str_radix(function, 16).ok()?;

    if device_number > 31 || function_number > 7 {
        return None;
    }

    Some(format!(
        "{domain_number:08x}:{bus_number:02x}:{device_number:02x}.{function_number:x}"
    ))
}

fn optional_number<T: std::str::FromStr>(value: &str) -> Option<T> {
    let value = value.trim();

    if value.is_empty()
        || value.eq_ignore_ascii_case("N/A")
        || value.eq_ignore_ascii_case("[Not Supported]")
        || value.eq_ignore_ascii_case("[Not Available]")
    {
        return None;
    }

    value.parse().ok()
}

fn mib_to_bytes(value: &str) -> Option<u64> {
    optional_number::<u64>(value)?.checked_mul(1024 * 1024)
}

/// Parses one CSV row in the exact requested field order:
/// PCI bus ID, GPU utilization, memory used, memory total,
/// temperature, graphics clock, and power draw.
///
/// The input must use `--format=csv,noheader,nounits`.
#[must_use]
pub fn parse_nvidia_smi_row(row: &str) -> Option<NvidiaSmiObservation> {
    let fields: Vec<_> = row.split(',').map(str::trim).collect();

    if fields.len() != 7 {
        return None;
    }

    let pci_identifier = normalize_pci_identifier(fields[0])?;

    let metrics = NvidiaGpuMetrics {
        utilization_percent: optional_number(fields[1]),
        memory_used_bytes: mib_to_bytes(fields[2]),
        memory_total_bytes: mib_to_bytes(fields[3]),
        temperature_celsius: optional_number(fields[4]),
        frequency_mhz: optional_number(fields[5]),
        power_watts: optional_number(fields[6]),
    };

    Some(NvidiaSmiObservation {
        pci_identifier,
        metrics,
    })
}

/// Parses NVIDIA SMI CSV output, ignoring empty lines.
///
/// Malformed rows are rejected rather than assigned to another GPU.
#[must_use]
pub fn parse_nvidia_smi_output(output: &str) -> Vec<NvidiaSmiObservation> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(parse_nvidia_smi_row)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_linux_and_nvidia_pci_formats() {
        assert_eq!(
            normalize_pci_identifier("0000:01:00.0"),
            Some("00000000:01:00.0".to_owned())
        );
        assert_eq!(
            normalize_pci_identifier("00000000:01:00.0"),
            Some("00000000:01:00.0".to_owned())
        );
        assert_eq!(
            normalize_pci_identifier("00000000:AB:1F.7"),
            Some("00000000:ab:1f.7".to_owned())
        );
    }

    #[test]
    fn rejects_invalid_pci_identifiers() {
        for value in [
            "",
            "01:00.0",
            "0000:01:00",
            "0000:01:20.0",
            "0000:01:00.8",
            "0000:GG:00.0",
            "000000000:01:00.0",
        ] {
            assert_eq!(normalize_pci_identifier(value), None, "{value}");
        }
    }

    #[test]
    fn parses_complete_gpu_measurements() {
        let row = "00000000:01:00.0, 42, 1024, 4096, 58, 1200, 35.5";
        let observation = parse_nvidia_smi_row(row).unwrap();

        assert_eq!(observation.pci_identifier, "00000000:01:00.0");
        assert_eq!(observation.metrics.utilization_percent, Some(42.0));
        assert_eq!(
            observation.metrics.memory_used_bytes,
            Some(1024 * 1024 * 1024)
        );
        assert_eq!(
            observation.metrics.memory_total_bytes,
            Some(4096 * 1024 * 1024)
        );
        assert_eq!(observation.metrics.temperature_celsius, Some(58.0));
        assert_eq!(observation.metrics.frequency_mhz, Some(1200));
        assert_eq!(observation.metrics.power_watts, Some(35.5));
    }

    #[test]
    fn unavailable_measurements_remain_none() {
        let row = "00000000:01:00.0, N/A, [Not Supported], 4096, N/A, 350, [Not Available]";
        let observation = parse_nvidia_smi_row(row).unwrap();

        assert_eq!(observation.metrics.utilization_percent, None);
        assert_eq!(observation.metrics.memory_used_bytes, None);
        assert_eq!(
            observation.metrics.memory_total_bytes,
            Some(4096 * 1024 * 1024)
        );
        assert_eq!(observation.metrics.temperature_celsius, None);
        assert_eq!(observation.metrics.frequency_mhz, Some(350));
        assert_eq!(observation.metrics.power_watts, None);
    }

    #[test]
    fn malformed_rows_are_rejected() {
        assert!(parse_nvidia_smi_row("bad,row").is_none());
        assert!(parse_nvidia_smi_row("invalid,1,2,3,4,5,6").is_none());
    }

    #[test]
    fn parses_multiple_gpu_rows_without_reordering() {
        let output = concat!(
            "00000000:01:00.0, 20, 100, 4000, 50, 900, 20\n",
            "00000000:02:00.0, 70, 200, 8000, 65, 1500, 60\n"
        );

        let observations = parse_nvidia_smi_output(output);

        assert_eq!(observations.len(), 2);
        assert_eq!(observations[0].pci_identifier, "00000000:01:00.0");
        assert_eq!(observations[1].pci_identifier, "00000000:02:00.0");
        assert_eq!(observations[0].metrics.utilization_percent, Some(20.0));
        assert_eq!(observations[1].metrics.utilization_percent, Some(70.0));
    }

    #[test]
    fn memory_overflow_is_unavailable() {
        let row = format!("00000000:01:00.0, 1, {}, 4096, 50, 900, 20", u64::MAX);

        let observation = parse_nvidia_smi_row(&row).unwrap();

        assert_eq!(observation.metrics.memory_used_bytes, None);
    }
}
