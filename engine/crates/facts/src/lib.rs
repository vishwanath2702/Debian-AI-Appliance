//! Hardware, operating-system, and environment fact discovery.

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

fn parse_linux_meminfo(meminfo: &str) -> Option<MemoryFacts> {
    let total_kib = meminfo.lines().find_map(|line| {
        let value = line.strip_prefix("MemTotal:")?;
        let value = value.trim().strip_suffix("kB")?.trim();
        value.parse::<u64>().ok()
    })?;

    Some(MemoryFacts::new(total_kib * 1024))
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
    use super::{MemoryFacts, discover_memory, parse_linux_meminfo, read_linux_meminfo};
    use std::fs;

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
    fn discovers_current_system_memory() {
        let facts = discover_memory().expect("discover memory facts");

        assert!(facts.total_bytes() > 0);
    }
}
