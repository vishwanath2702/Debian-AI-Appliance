//! Hardware, operating-system, and environment fact discovery.

use std::{io, process::Command};

trait ServiceCommandRunner {
    fn output(&mut self, command: &mut Command) -> io::Result<Vec<u8>>;
}

#[derive(Clone, Copy, Debug, Default)]
struct ProcessServiceCommandRunner;

impl ServiceCommandRunner for ProcessServiceCommandRunner {
    fn output(&mut self, command: &mut Command) -> io::Result<Vec<u8>> {
        let output = command.output()?;

        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(io::Error::other(format!(
                "systemctl service query exited unsuccessfully: {}",
                output.status
            )))
        }
    }
}

/// Compute accelerator information discovered from the current system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorFacts {
    identifier: String,
}

impl AcceleratorFacts {
    /// Creates accelerator facts from the discovered device identifier.
    #[must_use]
    pub fn new(identifier: impl Into<String>) -> Self {
        Self {
            identifier: identifier.into(),
        }
    }

    /// Returns the discovered device identifier.
    #[must_use]
    pub fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// CPU information discovered from the current system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuFacts {
    architecture: &'static str,
    logical_processor_count: usize,
    features: Vec<String>,
}

impl CpuFacts {
    /// Creates CPU facts with the discovered architecture and logical processor count.
    #[must_use]
    pub fn new(
        architecture: &'static str,
        logical_processor_count: usize,
        features: Vec<String>,
    ) -> Self {
        Self {
            architecture,
            logical_processor_count,
            features,
        }
    }

    /// Returns the CPU architecture.
    #[must_use]
    pub const fn architecture(&self) -> &'static str {
        self.architecture
    }

    /// Returns the number of logical processors.
    #[must_use]
    pub const fn logical_processor_count(&self) -> usize {
        self.logical_processor_count
    }

    /// Returns the discovered CPU feature identifiers.
    #[must_use]
    pub fn features(&self) -> &[String] {
        &self.features
    }
}

/// GPU information discovered from the current system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuFacts {
    identifier: String,
    vendor_id: String,
    device_id: String,
}

impl GpuFacts {
    /// Creates GPU facts from the discovered device identity.
    #[must_use]
    pub fn new(
        identifier: impl Into<String>,
        vendor_id: impl Into<String>,
        device_id: impl Into<String>,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            vendor_id: vendor_id.into(),
            device_id: device_id.into(),
        }
    }

    /// Returns the discovered device identifier.
    #[must_use]
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// Returns the discovered vendor identifier.
    #[must_use]
    pub fn vendor_id(&self) -> &str {
        &self.vendor_id
    }

    /// Returns the discovered device identifier assigned by the vendor.
    #[must_use]
    pub fn device_id(&self) -> &str {
        &self.device_id
    }
}

/// Hardware information discovered from the current system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HardwareFacts {
    cpu: CpuFacts,
    memory: MemoryFacts,
    gpus: Vec<GpuFacts>,
}

impl HardwareFacts {
    /// Creates hardware facts from discovered CPU, memory, and GPU information.
    #[must_use]
    pub fn new(cpu: CpuFacts, memory: MemoryFacts, gpus: Vec<GpuFacts>) -> Self {
        Self { cpu, memory, gpus }
    }

    /// Returns the discovered CPU information.
    #[must_use]
    pub const fn cpu(&self) -> &CpuFacts {
        &self.cpu
    }

    /// Returns the discovered memory information.
    #[must_use]
    pub const fn memory(&self) -> MemoryFacts {
        self.memory
    }

    /// Returns the discovered GPU information.
    #[must_use]
    pub fn gpus(&self) -> &[GpuFacts] {
        &self.gpus
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

fn query_systemd_service<R>(runner: &mut R, service: &str) -> io::Result<ServiceFacts>
where
    R: ServiceCommandRunner,
{
    let output = runner.output(
        Command::new("systemctl")
            .arg("show")
            .arg(service)
            .arg("--property=LoadState")
            .arg("--property=UnitFileState")
            .arg("--property=ActiveState")
            .arg("--no-pager"),
    )?;

    let output = std::str::from_utf8(&output).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("systemctl service output is not UTF-8: {error}"),
        )
    })?;

    parse_systemd_service_show(output).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "systemctl service output is missing required properties",
        )
    })
}

fn parse_systemd_service_show(output: &str) -> Option<ServiceFacts> {
    let mut load_state = None;
    let mut unit_file_state = None;
    let mut active_state = None;

    for line in output.lines() {
        let Some((field, value)) = line.split_once('=') else {
            continue;
        };

        match field {
            "LoadState" => load_state = Some(value),
            "UnitFileState" => unit_file_state = Some(value),
            "ActiveState" => active_state = Some(value),
            _ => {}
        }
    }

    let load_state = load_state?;
    let unit_file_state = unit_file_state?;
    let active_state = active_state?;

    Some(ServiceFacts::new(
        load_state != "not-found",
        unit_file_state == "enabled",
        active_state,
    ))
}

fn parse_linux_cpuinfo(cpuinfo: &str) -> Option<CpuFacts> {
    let logical_processor_count = cpuinfo
        .lines()
        .filter(|line| {
            line.split_once(':')
                .is_some_and(|(field, _)| field.trim() == "processor")
        })
        .count();

    let features = cpuinfo
        .lines()
        .find_map(|line| {
            let (field, value) = line.split_once(':')?;
            matches!(field.trim(), "flags" | "Features")
                .then(|| value.split_whitespace().map(str::to_owned).collect())
        })
        .unwrap_or_default();

    (logical_processor_count > 0)
        .then(|| CpuFacts::new(std::env::consts::ARCH, logical_processor_count, features))
}

fn parse_linux_meminfo(meminfo: &str) -> Option<MemoryFacts> {
    let total_kib = meminfo.lines().find_map(|line| {
        let value = line.strip_prefix("MemTotal:")?;
        let value = value.trim().strip_suffix("kB")?.trim();
        value.parse::<u64>().ok()
    })?;

    Some(MemoryFacts::new(total_kib * 1024))
}

fn read_linux_gpus(path: &std::path::Path) -> std::io::Result<Vec<GpuFacts>> {
    let mut gpus = Vec::new();

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let device_path = entry.path();

        let class = match std::fs::read_to_string(device_path.join("class")) {
            Ok(class) => class,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };

        if !class.trim().starts_with("0x03") {
            continue;
        }

        let vendor_id = std::fs::read_to_string(device_path.join("vendor"))?;
        let device_id = std::fs::read_to_string(device_path.join("device"))?;

        gpus.push(GpuFacts::new(
            entry.file_name().to_string_lossy(),
            vendor_id.trim(),
            device_id.trim(),
        ));
    }

    gpus.sort_by(|left, right| left.identifier().cmp(right.identifier()));

    Ok(gpus)
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

/// Discovers GPU facts for the current Linux system.
///
/// # Errors
///
/// Returns an error when the PCI sysfs device directory cannot be read or
/// when a discovered GPU device cannot be inspected.
pub fn discover_gpus() -> std::io::Result<Vec<GpuFacts>> {
    read_linux_gpus(std::path::Path::new("/sys/bus/pci/devices"))
}

/// Discovers hardware facts for the current system.
///
/// # Errors
///
/// Returns an error when CPU or memory facts cannot be discovered.
pub fn discover_hardware() -> std::io::Result<HardwareFacts> {
    let cpu = discover_cpu()?;
    let memory = discover_memory()?;
    let gpus = discover_gpus()?;

    Ok(HardwareFacts::new(cpu, memory, gpus))
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

/// Discovers service facts for the current system.
///
/// # Errors
///
/// Returns an error when `systemctl` cannot be executed or its output cannot
/// be interpreted as service facts.
pub fn discover_service(service: &str) -> io::Result<ServiceFacts> {
    let mut runner = ProcessServiceCommandRunner;

    query_systemd_service(&mut runner, service)
}

#[cfg(test)]
mod tests {
    use super::{
        AcceleratorFacts, CpuFacts, GpuFacts, HardwareFacts, MemoryFacts, ServiceCommandRunner,
        ServiceFacts, discover_cpu, discover_hardware, discover_memory, parse_linux_cpuinfo,
        parse_linux_meminfo, parse_systemd_service_show, query_systemd_service, read_linux_cpuinfo,
        read_linux_gpus, read_linux_meminfo,
    };
    use std::{ffi::OsStr, fs, io, process::Command};

    #[test]
    fn accelerator_facts_exposes_device_identity() {
        let facts = AcceleratorFacts::new("accel0");

        assert_eq!(facts.identifier(), "accel0");
    }

    #[test]
    fn cpu_facts_exposes_logical_processor_count() {
        let facts = CpuFacts::new("x86_64", 4, vec!["sse4_2".to_owned(), "avx2".to_owned()]);

        assert_eq!(facts.architecture(), "x86_64");
        assert_eq!(facts.logical_processor_count(), 4);
        assert_eq!(facts.features(), ["sse4_2", "avx2"]);
    }

    #[test]
    fn parses_linux_cpuinfo_logical_processor_count() {
        let cpuinfo = "processor\t: 0\nmodel name\t: Test CPU\nflags\t\t: sse4_2 avx avx2\n\nprocessor\t: 1\nmodel name\t: Test CPU\n";

        let facts = parse_linux_cpuinfo(cpuinfo).expect("CPU facts");

        assert_eq!(facts.architecture(), std::env::consts::ARCH);
        assert_eq!(facts.logical_processor_count(), 2);
        assert_eq!(facts.features(), ["sse4_2", "avx", "avx2"]);
    }

    #[test]
    fn parses_linux_cpuinfo_features_field() {
        let cpuinfo = "processor\t: 0\nFeatures\t: fp asimd aes sha1 sha2 crc32\n";

        let facts = parse_linux_cpuinfo(cpuinfo).expect("CPU facts");

        assert_eq!(
            facts.features(),
            ["fp", "asimd", "aes", "sha1", "sha2", "crc32"]
        );
    }

    #[test]
    fn reads_linux_cpuinfo_from_path() {
        let path = std::env::temp_dir().join("daia-facts-cpuinfo-test");
        fs::write(
            &path,
            "processor\t: 0\nflags\t\t: avx2\n\nprocessor\t: 1\n\nprocessor\t: 2\n",
        )
        .expect("write cpuinfo");

        let facts = read_linux_cpuinfo(&path).expect("read CPU facts");

        fs::remove_file(&path).expect("remove cpuinfo");

        assert_eq!(facts.logical_processor_count(), 3);
    }

    #[test]
    fn reads_linux_gpus_from_pci_sysfs() {
        let root = std::env::temp_dir().join("daia-facts-pci-gpu-test");
        let gpu = root.join("0000:01:00.0");
        let other = root.join("0000:02:00.0");

        fs::create_dir_all(&gpu).expect("create GPU sysfs directory");
        fs::create_dir_all(&other).expect("create non-GPU sysfs directory");

        fs::write(gpu.join("class"), "0x030000\n").expect("write GPU class");
        fs::write(gpu.join("vendor"), "0x10de\n").expect("write GPU vendor");
        fs::write(gpu.join("device"), "0x2684\n").expect("write GPU device");

        fs::write(other.join("class"), "0x020000\n").expect("write other class");
        fs::write(other.join("vendor"), "0x8086\n").expect("write other vendor");
        fs::write(other.join("device"), "0x1234\n").expect("write other device");

        let facts = read_linux_gpus(&root).expect("read GPU facts");

        fs::remove_dir_all(&root).expect("remove fake PCI sysfs");

        assert_eq!(facts, [GpuFacts::new("0000:01:00.0", "0x10de", "0x2684")]);
    }

    #[test]
    fn gpu_facts_exposes_device_identity() {
        let facts = GpuFacts::new("0000:01:00.0", "0x10de", "0x2684");

        assert_eq!(facts.identifier(), "0000:01:00.0");
        assert_eq!(facts.vendor_id(), "0x10de");
        assert_eq!(facts.device_id(), "0x2684");
    }

    #[test]
    fn hardware_facts_exposes_cpu_and_memory() {
        let facts = HardwareFacts::new(
            CpuFacts::new("x86_64", 4, vec!["avx2".to_owned()]),
            MemoryFacts::new(17_179_869_184),
            vec![GpuFacts::new("0000:01:00.0", "0x10de", "0x2684")],
        );

        assert_eq!(facts.cpu().architecture(), "x86_64");
        assert_eq!(facts.cpu().logical_processor_count(), 4);
        assert_eq!(facts.memory().total_bytes(), 17_179_869_184);
        assert_eq!(
            facts.gpus(),
            [GpuFacts::new("0000:01:00.0", "0x10de", "0x2684")]
        );
    }

    struct RecordingServiceCommandRunner {
        output: Vec<u8>,
        command: Option<Vec<String>>,
    }

    impl ServiceCommandRunner for RecordingServiceCommandRunner {
        fn output(&mut self, command: &mut Command) -> io::Result<Vec<u8>> {
            self.command = Some(
                command
                    .get_args()
                    .map(OsStr::to_string_lossy)
                    .map(|argument| argument.into_owned())
                    .collect(),
            );

            Ok(self.output.clone())
        }
    }

    #[test]
    fn queries_systemd_service_through_command_runner() {
        let mut runner = RecordingServiceCommandRunner {
            output: b"LoadState=loaded\nActiveState=active\nUnitFileState=enabled\n".to_vec(),
            command: None,
        };

        let facts =
            query_systemd_service(&mut runner, "ollama.service").expect("query service facts");

        assert_eq!(
            runner.command,
            Some(vec![
                "show".to_owned(),
                "ollama.service".to_owned(),
                "--property=LoadState".to_owned(),
                "--property=UnitFileState".to_owned(),
                "--property=ActiveState".to_owned(),
                "--no-pager".to_owned(),
            ])
        );
        assert!(facts.is_present());
        assert!(facts.is_enabled());
        assert_eq!(facts.active_state(), "active");
    }

    #[test]
    fn parses_systemd_service_show_state() {
        let output = "LoadState=loaded\nActiveState=active\nUnitFileState=enabled\n";

        let facts = parse_systemd_service_show(output).expect("service facts");

        assert!(facts.is_present());
        assert!(facts.is_enabled());
        assert_eq!(facts.active_state(), "active");
    }

    #[test]
    fn parses_missing_systemd_service() {
        let output = "LoadState=not-found\nActiveState=inactive\nUnitFileState=\n";

        let facts = parse_systemd_service_show(output).expect("service facts");

        assert!(!facts.is_present());
        assert!(!facts.is_enabled());
        assert_eq!(facts.active_state(), "inactive");
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

        assert_eq!(facts.cpu().architecture(), std::env::consts::ARCH);
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
