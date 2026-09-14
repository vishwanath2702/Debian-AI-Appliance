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

#[cfg(test)]
mod tests {
    use super::{MemoryFacts, parse_linux_meminfo};

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
}
