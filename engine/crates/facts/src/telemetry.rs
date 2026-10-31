//! Read-only Linux CPU and memory telemetry.

use std::{fs, io, path::Path};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuTimes {
    pub total: u64,
    pub idle: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryUsage {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

impl MemoryUsage {
    #[must_use]
    pub fn used_bytes(self) -> u64 {
        self.total_bytes.saturating_sub(self.available_bytes)
    }
}

#[must_use]
pub fn parse_cpu_times(stat: &str) -> Option<CpuTimes> {
    let line = stat
        .lines()
        .find(|line| line.split_whitespace().next() == Some("cpu"))?;

    let counters: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;

    if counters.len() < 4 {
        return None;
    }

    // guest and guest_nice are already included in user and nice.
    // Count at most the first eight /proc/stat CPU time fields.
    let total = counters
        .iter()
        .take(8)
        .try_fold(0_u64, |sum, value| sum.checked_add(*value))?;

    let idle = counters[3].checked_add(counters.get(4).copied().unwrap_or(0))?;

    Some(CpuTimes { total, idle })
}

#[must_use]
pub fn cpu_utilization_percent(previous: CpuTimes, current: CpuTimes) -> Option<f64> {
    let total_delta = current.total.checked_sub(previous.total)?;
    let idle_delta = current.idle.checked_sub(previous.idle)?;

    if total_delta == 0 || idle_delta > total_delta {
        return None;
    }

    Some(100.0 * (total_delta - idle_delta) as f64 / total_delta as f64)
}

#[must_use]
pub fn parse_memory_usage(meminfo: &str) -> Option<MemoryUsage> {
    fn field(meminfo: &str, name: &str) -> Option<u64> {
        let value = meminfo.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key == name).then_some(value)
        })?;

        let mut parts = value.split_whitespace();
        let kib = parts.next()?.parse::<u64>().ok()?;

        if parts.next()? != "kB" || parts.next().is_some() {
            return None;
        }

        kib.checked_mul(1024)
    }

    let total_bytes = field(meminfo, "MemTotal")?;
    let available_bytes = field(meminfo, "MemAvailable")?;

    if available_bytes > total_bytes {
        return None;
    }

    Some(MemoryUsage {
        total_bytes,
        available_bytes,
    })
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub fn read_cpu_times(path: &Path) -> io::Result<CpuTimes> {
    let contents = fs::read_to_string(path)?;
    parse_cpu_times(&contents).ok_or_else(|| invalid_data("invalid Linux CPU counters"))
}

pub fn read_memory_usage(path: &Path) -> io::Result<MemoryUsage> {
    let contents = fs::read_to_string(path)?;
    parse_memory_usage(&contents).ok_or_else(|| invalid_data("invalid Linux memory counters"))
}

/// Reads aggregate CPU counters from the current Linux system.
///
/// # Errors
/// Returns an error if procfs cannot be read or parsed.
pub fn sample_cpu_times() -> io::Result<CpuTimes> {
    read_cpu_times(Path::new("/proc/stat"))
}

/// Reads total and available RAM from the current Linux system.
///
/// # Errors
/// Returns an error if procfs cannot be read or parsed.
pub fn sample_memory_usage() -> io::Result<MemoryUsage> {
    read_memory_usage(Path::new("/proc/meminfo"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guest_counters_are_not_double_counted() {
        let stat = "cpu 100 20 30 400 50 6 7 8 90 10\n";
        assert_eq!(
            parse_cpu_times(stat),
            Some(CpuTimes {
                total: 621,
                idle: 450,
            })
        );
    }

    #[test]
    fn guest_counters_do_not_overflow_total() {
        let stat = format!("cpu 1 2 3 4 5 6 7 8 {} {}\n", u64::MAX, u64::MAX);
        assert_eq!(
            parse_cpu_times(&stat),
            Some(CpuTimes { total: 36, idle: 9 })
        );
    }

    #[test]
    fn parses_aggregate_cpu_counters() {
        let stat = "cpu  100 20 30 400 50 6 7 8 0 0\ncpu0 1 2 3 4\n";
        assert_eq!(
            parse_cpu_times(stat),
            Some(CpuTimes {
                total: 621,
                idle: 450,
            })
        );
    }

    #[test]
    fn rejects_missing_or_malformed_cpu_counters() {
        assert_eq!(parse_cpu_times("cpu0 1 2 3 4\n"), None);
        assert_eq!(parse_cpu_times("cpu 1 2 3\n"), None);
        assert_eq!(parse_cpu_times("cpu 1 2 bad 4\n"), None);
    }

    #[test]
    fn calculates_cpu_utilization_from_two_samples() {
        let previous = CpuTimes {
            total: 1000,
            idle: 700,
        };
        let current = CpuTimes {
            total: 1200,
            idle: 750,
        };

        assert_eq!(cpu_utilization_percent(previous, current), Some(75.0));
    }

    #[test]
    fn rejects_invalid_cpu_intervals() {
        let previous = CpuTimes {
            total: 100,
            idle: 60,
        };
        assert_eq!(cpu_utilization_percent(previous, previous), None);
        assert_eq!(
            cpu_utilization_percent(
                previous,
                CpuTimes {
                    total: 90,
                    idle: 50,
                }
            ),
            None
        );
    }

    #[test]
    fn parses_memory_usage() {
        let meminfo = "MemTotal: 8192 kB\nMemAvailable: 3072 kB\n";
        let usage = parse_memory_usage(meminfo).expect("memory usage");

        assert_eq!(usage.total_bytes, 8_388_608);
        assert_eq!(usage.available_bytes, 3_145_728);
        assert_eq!(usage.used_bytes(), 5_242_880);
    }

    #[test]
    fn rejects_missing_or_impossible_memory_usage() {
        assert_eq!(parse_memory_usage("MemTotal: 8192 kB\n"), None);
        assert_eq!(
            parse_memory_usage("MemTotal: 1024 kB\nMemAvailable: 2048 kB\n"),
            None
        );
    }
}
