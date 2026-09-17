//! Hardware, operating-system, and environment fact discovery.

/// CPU information discovered from the current system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuFacts {
    logical_processor_count: usize,
}

impl CpuFacts {
    /// Creates CPU facts with the discovered logical processor count.
    #[must_use]
    pub const fn new(logical_processor_count: usize) -> Self {
        Self {
            logical_processor_count,
        }
    }

    /// Returns the number of logical processors.
    #[must_use]
    pub const fn logical_processor_count(self) -> usize {
        self.logical_processor_count
    }
}

/// Hardware information discovered from the current system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HardwareFacts {
    cpu: CpuFacts,
    memory: MemoryFacts,
}

impl HardwareFacts {
    /// Creates hardware facts from discovered CPU and memory information.
    #[must_use]
    pub const fn new(cpu: CpuFacts, memory: MemoryFacts) -> Self {
        Self { cpu, memory }
    }

    /// Returns the discovered CPU information.
    #[must_use]
    pub const fn cpu(self) -> CpuFacts {
        self.cpu
    }

    /// Returns the discovered memory information.
    #[must_use]
    pub const fn memory(self) -> MemoryFacts {
        self.memory
    }
}

/// Service information discovered from the current system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceFacts {
    present: bool,
    enabled: bool,
    active_state: String,
}

impl ServiceFacts {
    /// Creates service facts from discovered presence, enablement, and active state.
    #[must_use]
    pub fn new(present: bool, enabled: bool, active_state: impl Into<String>) -> Self {
        Self {
            present,
            enabled,
            active_state: active_state.into(),
        }
    }

    /// Returns whether the service is present on the system.
    #[must_use]
    pub const fn is_present(&self) -> bool {
        self.present
    }

    /// Returns whether the service is enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the observed service active state.
    #[must_use]
    pub fn active_state(&self) -> &str {
        &self.active_state
    }
}

/// Memory information discovered from the current system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryFacts {
    total_bytes: u64,
}

impl MemoryFacts {
    /// Creates memory facts with the discovered total memory size.
    #[must_use]
    pub const fn new(total_bytes: u64) -> Self {
        Self { total_bytes }
    }

    /// Returns total system memory in bytes.
    #[must_use]
    pub const fn total_bytes(self) -> u64 {
        self.total_bytes
    }
}

fn parse_linux_cpuinfo(cpuinfo: &str) -> Option<CpuFacts> {
    let logical_processor_count = cpuinfo
        .lines()
        .filter(|line| {
            line.split_once(':')
                .is_some_and(|(field, _)| field.trim() == "processor")
        })
        .count();

    (logical_processor_count > 0).then(|| CpuFacts::new(logical_processor_count))
}

fn parse_linux_meminfo(meminfo: &str) -> Option<MemoryFacts> {
    let total_kib = meminfo.lines().find_map(|line| {
        let value = line.strip_prefix("MemTotal:")?;
        let value = value.trim().strip_suffix("kB")?.trim();
        value.parse::<u64>().ok()
    })?;

    Some(MemoryFacts::new(total_kib * 1024))
}

fn read_linux_cpuinfo(path: &std::path::Path) -> std::io::Result<CpuFacts> {
    let cpuinfo = std::fs::read_to_string(path)?;

    parse_linux_cpuinfo(&cpuinfo).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "processor entries missing from Linux cpuinfo",
        )
    })
}

fn read_linux_meminfo(path: &std::path::Path) -> std::io::Result<MemoryFacts> {
    let meminfo = std::fs::read_to_string(path)?;

    parse_linux_meminfo(&meminfo).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "MemTotal missing from Linux meminfo",
        )
    })
}

/// Discovers CPU facts for the current Linux system.
///
/// # Errors
///
/// Returns an error when `/proc/cpuinfo` cannot be read or does not contain
/// any processor entries.
pub fn discover_cpu() -> std::io::Result<CpuFacts> {
    read_linux_cpuinfo(std::path::Path::new("/proc/cpuinfo"))
}

/// Discovers hardware facts for the current system.
///
/// # Errors
///
/// Returns an error when CPU or memory facts cannot be discovered.
pub fn discover_hardware() -> std::io::Result<HardwareFacts> {
    let cpu = discover_cpu()?;
    let memory = discover_memory()?;

    Ok(HardwareFacts::new(cpu, memory))
}

/// Discovers memory facts for the current Linux system.
///
/// # Errors
///
/// Returns an error when `/proc/meminfo` cannot be read or does not contain
/// a valid `MemTotal` entry.
pub fn discover_memory() -> std::io::Result<MemoryFacts> {
    read_linux_meminfo(std::path::Path::new("/proc/meminfo"))
}

#[cfg(test)]
mod tests {
    use super::{
        CpuFacts, HardwareFacts, MemoryFacts, ServiceFacts, discover_cpu, discover_hardware,
        discover_memory, parse_linux_cpuinfo, parse_linux_meminfo, read_linux_cpuinfo,
        read_linux_meminfo,
    };
    use std::fs;

    #[test]
    fn cpu_facts_exposes_logical_processor_count() {
        let facts = CpuFacts::new(4);

        assert_eq!(facts.logical_processor_count(), 4);
    }

    #[test]
    fn parses_linux_cpuinfo_logical_processor_count() {
        let cpuinfo =
            "processor\t: 0\nmodel name\t: Test CPU\n\nprocessor\t: 1\nmodel name\t: Test CPU\n";

        let facts = parse_linux_cpuinfo(cpuinfo).expect("CPU facts");

        assert_eq!(facts.logical_processor_count(), 2);
    }

    #[test]
    fn reads_linux_cpuinfo_from_path() {
        let path = std::env::temp_dir().join("daia-facts-cpuinfo-test");
        fs::write(
            &path,
            "processor\t: 0\n\nprocessor\t: 1\n\nprocessor\t: 2\n",
        )
        .expect("write cpuinfo");

        let facts = read_linux_cpuinfo(&path).expect("read CPU facts");

        fs::remove_file(&path).expect("remove cpuinfo");

        assert_eq!(facts.logical_processor_count(), 3);
    }

    #[test]
    fn hardware_facts_exposes_cpu_and_memory() {
        let facts = HardwareFacts::new(CpuFacts::new(4), MemoryFacts::new(17_179_869_184));

        assert_eq!(facts.cpu().logical_processor_count(), 4);
        assert_eq!(facts.memory().total_bytes(), 17_179_869_184);
    }

    #[test]
    fn service_facts_exposes_observed_state() {
        let facts = ServiceFacts::new(true, true, "active");

        assert!(facts.is_present());
        assert!(facts.is_enabled());
        assert_eq!(facts.active_state(), "active");
    }

    #[test]
    fn memory_facts_exposes_total_bytes() {
        let facts = MemoryFacts::new(17_179_869_184);

        assert_eq!(facts.total_bytes(), 17_179_869_184);
    }

    #[test]
    fn parses_linux_meminfo_total_memory() {
        let meminfo = "MemTotal:       16384256 kB\nMemFree:         1024000 kB\n";

        let facts = parse_linux_meminfo(meminfo).expect("memory facts");

        assert_eq!(facts.total_bytes(), 16_777_478_144);
    }

    #[test]
    fn reads_linux_meminfo_from_path() {
        let path = std::env::temp_dir().join("daia-facts-meminfo-test");
        fs::write(&path, "MemTotal:       8192 kB\n").expect("write meminfo");

        let facts = read_linux_meminfo(&path).expect("read memory facts");

        fs::remove_file(&path).expect("remove meminfo");

        assert_eq!(facts.total_bytes(), 8_388_608);
    }

    #[test]
    fn discovers_current_system_hardware() {
        let facts = discover_hardware().expect("discover hardware facts");

        assert!(facts.cpu().logical_processor_count() > 0);
        assert!(facts.memory().total_bytes() > 0);
    }

    #[test]
    fn discovers_current_system_cpu() {
        let facts = discover_cpu().expect("discover CPU facts");

        assert!(facts.logical_processor_count() > 0);
    }

    #[test]
    fn discovers_current_system_memory() {
        let facts = discover_memory().expect("discover memory facts");

        assert!(facts.total_bytes() > 0);
    }
}
