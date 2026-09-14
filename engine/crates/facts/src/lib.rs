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

#[cfg(test)]
mod tests {
    use super::MemoryFacts;

    #[test]
    fn memory_facts_exposes_total_bytes() {
        let facts = MemoryFacts::new(17_179_869_184);

        assert_eq!(facts.total_bytes(), 17_179_869_184);
    }
}
