//! Read-only planning for exporting Ollama-managed models.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAX_OLLAMA_EXPORT_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    config: Option<Descriptor>,
    #[serde(default)]
    layers: Vec<Descriptor>,
}

#[derive(Debug, Deserialize)]
struct Descriptor {
    digest: String,
    size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportBlob {
    pub digest: String,
    pub path: PathBuf,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OllamaExportPlan {
    pub manifest_path: PathBuf,
    pub blobs: Vec<ExportBlob>,
    pub total_bytes: u64,
}

/// An opened blob whose file identity is retained across planning and copying.
///
/// The descriptor must originate from a separately authorized source.
#[derive(Debug)]
struct OpenedExportBlob {
    digest: String,
    file: fs::File,
    size_bytes: u64,
}

/// An export plan that retains opened source files instead of trusting paths.
///
/// This type does not itself establish caller authorization or process isolation.
#[derive(Debug)]
struct OpenedOllamaExportPlan {
    manifest: fs::File,
    blobs: Vec<OpenedExportBlob>,
    manifest_relative_path: PathBuf,
    manifest_size_bytes: u64,
    total_bytes: u64,
}

impl OpenedOllamaExportPlan {
    fn new(
        manifest: fs::File,
        blobs: Vec<OpenedExportBlob>,
        manifest_size: u64,
        manifest_relative_path: &Path,
    ) -> io::Result<Self> {
        use std::path::Component;

        if manifest_relative_path.components().count() != 4
            || manifest_relative_path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(invalid("invalid Ollama export manifest identity"));
        }

        let mut total_bytes = manifest_size;

        for blob in &blobs {
            total_bytes = total_bytes
                .checked_add(blob.size_bytes)
                .ok_or_else(|| invalid("export size overflow"))?;
        }

        Ok(Self {
            manifest,
            blobs,
            manifest_relative_path: manifest_relative_path.to_path_buf(),
            manifest_size_bytes: manifest_size,
            total_bytes,
        })
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn blob_filename(digest: &str) -> io::Result<String> {
    let hash = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| invalid("unsupported Ollama blob digest"))?;

    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid("invalid SHA-256 digest"));
    }

    Ok(format!("sha256-{hash}"))
}

/// Parses a bounded Ollama manifest without accessing filesystem paths.
fn parse_ollama_export_manifest(bytes: &[u8]) -> io::Result<Manifest> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_OLLAMA_EXPORT_MANIFEST_BYTES {
        return Err(invalid("Ollama manifest exceeds maximum size"));
    }

    serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("invalid Ollama manifest: {error}")))
}

/// Reads a bounded Ollama manifest from an already-open stream.
///
/// Opening and authorization remain the caller's responsibility.
fn read_ollama_export_manifest<R: Read>(reader: &mut R) -> io::Result<(Manifest, u64)> {
    let mut bytes = Vec::new();

    reader
        .take(MAX_OLLAMA_EXPORT_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)?;

    let manifest = parse_ollama_export_manifest(&bytes)?;

    let size = u64::try_from(bytes.len()).map_err(|_| invalid("manifest size overflow"))?;

    Ok((manifest, size))
}

/// Copies one Ollama blob with bounded memory and verifies its SHA-256.
///
/// The caller owns destination creation, cleanup, and publication.
pub fn copy_blob_with_sha256_verification<R, W, F>(
    reader: &mut R,
    writer: &mut W,
    expected_digest: &str,
    expected_size: u64,
    mut progress: F,
) -> io::Result<u64>
where
    R: Read,
    W: Write,
    F: FnMut(u64),
{
    let expected_hash = expected_digest
        .strip_prefix("sha256:")
        .ok_or_else(|| invalid("unsupported Ollama blob digest"))?;

    if expected_hash.len() != 64 || !expected_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid("invalid SHA-256 digest"));
    }

    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut copied = 0_u64;

    loop {
        let count = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };

        copied = copied
            .checked_add(count as u64)
            .ok_or_else(|| invalid("blob size overflow"))?;

        if copied > expected_size {
            return Err(invalid("Ollama blob exceeds expected size"));
        }

        writer.write_all(&buffer[..count])?;
        hasher.update(&buffer[..count]);
        progress(copied);
    }

    if copied != expected_size {
        return Err(invalid("Ollama blob size mismatch"));
    }

    let actual_hash = format!("{:x}", hasher.finalize());

    if !actual_hash.eq_ignore_ascii_case(expected_hash) {
        return Err(invalid("Ollama blob SHA-256 mismatch"));
    }

    writer.flush()?;
    Ok(copied)
}

/// Result of attempting to publish an export directory.
///
/// A successful rename followed by failed synchronization is not
/// equivalent to a failed rename: the destination may already exist.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportPublicationOutcome {
    NotPublished,
    PublishedAndSynced,
    PublishedDurabilityUncertain,
}

/// Publication failure with an explicit filesystem outcome.
///
/// The source error is preserved so callers can distinguish collisions,
/// permission failures, and synchronization failures.
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub struct ExportPublicationError {
    pub outcome: ExportPublicationOutcome,
    pub source: io::Error,
}

#[cfg(target_os = "linux")]
impl std::fmt::Display for ExportPublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "export publication {:?}: {}",
            self.outcome, self.source,
        )
    }
}

#[cfg(target_os = "linux")]
impl std::error::Error for ExportPublicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Classifies the outcome of rename and parent-directory synchronization.
#[cfg(target_os = "linux")]
fn classify_export_publication_outcome(renamed: bool, synced: bool) -> ExportPublicationOutcome {
    if !renamed {
        ExportPublicationOutcome::NotPublished
    } else if synced {
        ExportPublicationOutcome::PublishedAndSynced
    } else {
        ExportPublicationOutcome::PublishedDurabilityUncertain
    }
}

/// A private staging directory anchored to its original parent handle.
///
/// Drop removes an empty unpublished staging directory. Recursive cleanup
/// of populated staging directories will be added before copying models.
#[cfg(target_os = "linux")]
pub struct HandleAnchoredExportStaging {
    parent: std::os::fd::OwnedFd,
    directory: std::os::fd::OwnedFd,
    name: String,
    published: bool,
}

#[cfg(target_os = "linux")]
impl HandleAnchoredExportStaging {
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Publishes the staging directory without replacing an existing entry.
    ///
    /// The caller must first verify and synchronize all staged contents.
    pub fn publish(
        self,
        destination_name: &std::ffi::OsStr,
    ) -> Result<ExportPublicationOutcome, ExportPublicationError> {
        self.publish_with_sync(destination_name, |parent| {
            rustix::fs::fsync(parent).map_err(Into::into)
        })
    }

    fn publish_with_sync<F>(
        mut self,
        destination_name: &std::ffi::OsStr,
        synchronize_parent: F,
    ) -> Result<ExportPublicationOutcome, ExportPublicationError>
    where
        F: FnOnce(&std::os::fd::OwnedFd) -> io::Result<()>,
    {
        use rustix::fs::RenameFlags;

        let not_published = |source: io::Error| ExportPublicationError {
            outcome: ExportPublicationOutcome::NotPublished,
            source,
        };
        use std::path::{Component, Path};

        let mut components = Path::new(destination_name).components();

        if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
            return Err(not_published(io::Error::new(
                io::ErrorKind::InvalidInput,
                "export destination must be a single normal name",
            )));
        }

        // Verify that the staging name still refers to the
        // directory retained when staging was created.
        let original =
            rustix::fs::fstat(&self.directory).map_err(|error| not_published(error.into()))?;

        let named_directory = rustix::fs::openat(
            &self.parent,
            self.name.as_str(),
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|error| not_published(error.into()))?;

        let named =
            rustix::fs::fstat(&named_directory).map_err(|error| not_published(error.into()))?;

        if original.st_dev != named.st_dev || original.st_ino != named.st_ino {
            return Err(not_published(io::Error::new(
                io::ErrorKind::InvalidData,
                "export staging directory identity changed",
            )));
        }

        rustix::fs::renameat_with(
            &self.parent,
            self.name.as_str(),
            &self.parent,
            destination_name,
            RenameFlags::NOREPLACE,
        )
        .map_err(|error| not_published(error.into()))?;

        self.published = true;

        synchronize_parent(&self.parent).map_err(|error| ExportPublicationError {
            outcome: ExportPublicationOutcome::PublishedDurabilityUncertain,
            source: error,
        })?;

        Ok(ExportPublicationOutcome::PublishedAndSynced)
    }
}

/// Removes only entries beneath an already opened directory.
///
/// All traversal is descriptor-relative. Symlinks are unlinked, never followed.
#[cfg(target_os = "linux")]
fn remove_export_directory_contents(
    directory: &std::os::fd::OwnedFd,
    root_device: u64,
) -> io::Result<()> {
    use rustix::fs::{AtFlags, Mode, OFlags, RawDir};
    use std::mem::MaybeUninit;

    let mut buffer = [MaybeUninit::<u8>::uninit(); 8192];
    let mut entries = RawDir::new(directory, &mut buffer);

    while let Some(result) = entries.next() {
        let entry = result?;
        let name = entry.file_name();

        if name.to_bytes() == b"." || name.to_bytes() == b".." {
            continue;
        }

        match rustix::fs::openat(
            directory,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(child) => {
                let metadata = rustix::fs::fstat(&child)?;

                if metadata.st_dev != root_device {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "refusing cross-filesystem export cleanup",
                    ));
                }

                remove_export_directory_contents(&child, root_device)?;

                rustix::fs::unlinkat(directory, name, AtFlags::REMOVEDIR)?;
            }
            Err(rustix::io::Errno::NOTDIR) | Err(rustix::io::Errno::LOOP) => {
                rustix::fs::unlinkat(directory, name, AtFlags::empty())?;
            }
            Err(error) => return Err(error.into()),
        }
    }

    Ok(())
}

#[cfg(target_os = "linux")]
impl Drop for HandleAnchoredExportStaging {
    fn drop(&mut self) {
        if self.published {
            return;
        }

        use rustix::fs::{AtFlags, Mode, OFlags};

        let original = match rustix::fs::fstat(&self.directory) {
            Ok(metadata) => metadata,
            Err(_) => return,
        };

        // Do not clean anything if the original staging directory
        // has been moved or replaced under its original name.
        let named_directory = match rustix::fs::openat(
            &self.parent,
            self.name.as_str(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(directory) => directory,
            Err(_) => return,
        };

        let named = match rustix::fs::fstat(&named_directory) {
            Ok(metadata) => metadata,
            Err(_) => return,
        };

        if original.st_dev != named.st_dev || original.st_ino != named.st_ino {
            return;
        }

        // Operate on the original handle, never on a reopened
        // directory that could point elsewhere.
        if remove_export_directory_contents(&self.directory, original.st_dev).is_err() {
            return;
        }

        let _ = rustix::fs::unlinkat(&self.parent, self.name.as_str(), AtFlags::REMOVEDIR);
    }
}

/// Allows the conventional root-owned sticky temporary directory
/// as an ancestor for non-root export operations.
///
/// This policy does not independently validate child ownership,
/// directory-entry stability, or the complete destination path.
#[cfg(target_os = "linux")]
fn is_safe_export_sticky_ancestor(
    effective_uid: u32,
    ancestor_uid: u32,
    ancestor_mode: u32,
) -> bool {
    effective_uid != 0 && ancestor_uid == 0 && ancestor_mode & 0o7777 == 0o1777
}

/// Determines whether an ancestor directory is trusted for
/// unprivileged model export path traversal.
///
/// Sticky directories are deliberately excluded until their
/// replacement semantics are handled separately.
#[cfg(target_os = "linux")]
fn is_safe_export_ancestor(effective_uid: u32, ancestor_uid: u32, ancestor_mode: u32) -> bool {
    effective_uid != 0
        && (ancestor_uid == 0 || ancestor_uid == effective_uid)
        && ancestor_mode & 0o022 == 0
}

/// Writable exports must run as an unprivileged user and target
/// a destination parent owned by that same effective user.
#[cfg(target_os = "linux")]
fn is_safe_export_destination_owner(effective_uid: u32, parent_uid: u32) -> bool {
    effective_uid != 0 && effective_uid == parent_uid
}

/// Restricts writable exports to filesystems with native Unix
/// directory permissions and expected rename semantics.
///
/// Filesystem identifiers come from Linux `fstatfs`.
#[cfg(target_os = "linux")]
fn is_supported_export_destination_filesystem(filesystem_type: u64) -> bool {
    matches!(
        filesystem_type,
        0xEF53       // ext2/ext3/ext4
            | 0x58465342 // XFS
            | 0x9123683E // Btrfs
            | 0x01021994 // tmpfs
    )
}

/// Validates each directory component using descriptor-relative traversal.
///
/// The root-owned sticky-directory exception supports conventional /tmp
/// paths, but each subsequent component must independently pass validation.
#[cfg(target_os = "linux")]
fn validate_export_destination_ancestors(
    root_fd: &std::os::fd::OwnedFd,
    relative_parent: &Path,
    effective_uid: u32,
) -> io::Result<std::os::fd::OwnedFd> {
    use rustix::fs::{Mode, OFlags};
    use std::path::Component;

    let mut current = rustix::fs::openat(
        root_fd,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    let root_metadata = rustix::fs::fstat(&current)?;
    if !is_safe_export_ancestor(effective_uid, root_metadata.st_uid, root_metadata.st_mode) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "export destination root directory is not trusted",
        ));
    }

    for component in relative_parent.components() {
        let Component::Normal(name) = component else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid export destination ancestor component",
            ));
        };

        let next = rustix::fs::openat(
            &current,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?;

        let metadata = rustix::fs::fstat(&next)?;

        if !is_safe_export_ancestor(effective_uid, metadata.st_uid, metadata.st_mode)
            && !is_safe_export_sticky_ancestor(effective_uid, metadata.st_uid, metadata.st_mode)
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "export destination contains an untrusted ancestor",
            ));
        }

        current = next;
    }

    Ok(current)
}

/// Creates a private directory using a retained destination-parent handle.
///
/// The parent path must be absolute and contain no symlink components.
#[cfg(target_os = "linux")]
pub fn create_handle_anchored_export_staging(
    destination_parent: &Path,
) -> io::Result<HandleAnchoredExportStaging> {
    use rustix::fs::{Mode, OFlags};
    use std::path::Component;

    if !destination_parent.is_absolute()
        || destination_parent
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "export parent must be an absolute path without traversal",
        ));
    }

    let root_fd = rustix::fs::open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    let relative_parent = destination_parent
        .strip_prefix("/")
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid export parent"))?;

    let effective_uid = rustix::process::geteuid().as_raw();

    let parent = validate_export_destination_ancestors(&root_fd, relative_parent, effective_uid)?;

    // A writable parent allows other users to replace the staging
    // directory name between identity verification and publication.
    // Inspect the retained handle; never change user permissions.
    let parent_metadata = rustix::fs::fstat(&parent)?;

    // Inspect the filesystem through the retained directory handle.
    // Do not trust a pathname-based filesystem classification.
    let filesystem = rustix::fs::fstatfs(&parent)?;

    if !is_supported_export_destination_filesystem(filesystem.f_type as u64) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "export destination filesystem is not supported for writable export",
        ));
    }

    let filesystem_status = rustix::fs::fstatvfs(&parent)?;

    if filesystem_status
        .f_flag
        .contains(rustix::fs::StatVfsMountFlags::RDONLY)
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "export destination filesystem is mounted read-only",
        ));
    }

    if !is_safe_export_destination_owner(effective_uid, parent_metadata.st_uid) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "export destination must be owned by the non-root exporting user",
        ));
    }

    if parent_metadata.st_mode & 0o022 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "export destination parent must not be group- or world-writable",
        ));
    }

    for _ in 0..16 {
        let mut random = [0_u8; 16];

        let filled = rustix::rand::getrandom(&mut random, rustix::rand::GetRandomFlags::empty())?;

        if filled != random.len() {
            return Err(io::Error::other("incomplete export staging randomness"));
        }

        let suffix: String = random.iter().map(|byte| format!("{byte:02x}")).collect();

        let name = format!(".daia-export-{suffix}");

        match rustix::fs::mkdirat(&parent, name.as_str(), Mode::from_raw_mode(0o700)) {
            Ok(()) => {
                let directory = match rustix::fs::openat(
                    &parent,
                    name.as_str(),
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                ) {
                    Ok(directory) => directory,
                    Err(error) => {
                        let _ = rustix::fs::unlinkat(
                            &parent,
                            name.as_str(),
                            rustix::fs::AtFlags::REMOVEDIR,
                        );
                        return Err(error.into());
                    }
                };

                // Verify the opened staging directory before accepting it.
                // This does not make mkdirat/openat atomic.
                let staging_metadata = rustix::fs::fstat(&directory)?;

                if staging_metadata.st_uid != effective_uid
                    || staging_metadata.st_mode & 0o7777 != 0o700
                    || staging_metadata.st_dev != parent_metadata.st_dev
                {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "export staging directory has unexpected metadata",
                    ));
                }

                return Ok(HandleAnchoredExportStaging {
                    parent,
                    directory,
                    name,
                    published: false,
                });
            }
            Err(rustix::io::Errno::EXIST) => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "unable to allocate unique export staging directory",
    ))
}

/// Creates an automatically cleaned-up private export staging directory.
///
/// The destination parent must already exist and must not be a symlink.
/// Publication of a completed export is handled separately.
pub fn create_private_export_staging(destination_parent: &Path) -> io::Result<tempfile::TempDir> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::symlink_metadata(destination_parent)?;

    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "export destination parent must be a real directory",
        ));
    }

    let staging = tempfile::Builder::new()
        .prefix(".daia-export-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in(destination_parent)?;

    Ok(staging)
}

/// Atomically publishes a completed staging directory without overwriting.
///
/// The caller must have fully copied, verified, and synchronized its contents.
/// Staging and destination must share the same parent directory.
#[cfg(target_os = "linux")]
pub fn publish_completed_ollama_export(
    staging: tempfile::TempDir,
    destination: &Path,
) -> io::Result<()> {
    use rustix::fs::{Mode, OFlags, RenameFlags, ResolveFlags};

    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing export destination parent",
        )
    })?;

    if staging.path().parent() != Some(parent) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "export staging and destination must share a parent",
        ));
    }

    let name = destination.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing export destination name",
        )
    })?;

    if name == "." || name == ".." {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid export destination name",
        ));
    }

    let staging_name = staging.path().file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "missing export staging name")
    })?;

    if !parent.is_absolute()
        || parent
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "export destination parent must be absolute without traversal",
        ));
    }

    let root_fd = rustix::fs::open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    let relative_parent = parent
        .strip_prefix("/")
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid export parent"))?;

    let parent_fd = rustix::fs::openat2(
        &root_fd,
        relative_parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
    )?;

    rustix::fs::renameat_with(
        &parent_fd,
        staging_name,
        &parent_fd,
        name,
        RenameFlags::NOREPLACE,
    )?;

    rustix::fs::fsync(&parent_fd)?;

    // The directory now belongs to the published destination.
    // Prevent TempDir from deleting it on drop.
    let _published_path = staging.keep();

    Ok(())
}

/// Opens a manifest beneath an already-opened manifest root.
///
/// The returned descriptor retains the validated file identity.
#[cfg(target_os = "linux")]
fn open_ollama_export_manifest(
    manifest_root_fd: &std::os::fd::OwnedFd,
    relative: &Path,
) -> io::Result<fs::File> {
    use rustix::fs::{Mode, OFlags, ResolveFlags};

    let manifest_fd = rustix::fs::openat2(
        manifest_root_fd,
        relative,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
    )?;

    let metadata = rustix::fs::fstat(&manifest_fd)?;
    if rustix::fs::FileType::from_raw_mode(metadata.st_mode) != rustix::fs::FileType::RegularFile {
        return Err(invalid("unsafe Ollama manifest file"));
    }

    Ok(fs::File::from(manifest_fd))
}

/// Opens a digest-named blob beneath an already-opened blob directory.
///
/// The returned file descriptor retains the validated source identity.
#[cfg(target_os = "linux")]
fn open_ollama_export_blob(
    blob_root_fd: &std::os::fd::OwnedFd,
    digest: &str,
    expected_size: u64,
) -> io::Result<fs::File> {
    use rustix::fs::{Mode, OFlags, ResolveFlags};

    let filename = blob_filename(digest)?;

    let blob_fd = rustix::fs::openat2(
        blob_root_fd,
        filename.as_str(),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
    )?;

    let metadata = rustix::fs::fstat(&blob_fd)?;

    if rustix::fs::FileType::from_raw_mode(metadata.st_mode) != rustix::fs::FileType::RegularFile {
        return Err(invalid("unsafe Ollama blob file"));
    }

    if metadata.st_size < 0 || metadata.st_size as u64 != expected_size {
        return Err(invalid("Ollama blob size mismatch"));
    }

    Ok(fs::File::from(blob_fd))
}

/// Parses an already-opened manifest and rewinds it for subsequent copying.
///
/// Source authorization and safe descriptor opening are caller responsibilities.
#[cfg(target_os = "linux")]
fn prepare_opened_export_manifest(
    mut manifest_file: fs::File,
) -> io::Result<(fs::File, Manifest, u64)> {
    use std::io::{Seek, SeekFrom};

    let (manifest, manifest_size) = read_ollama_export_manifest(&mut manifest_file)?;

    manifest_file.seek(SeekFrom::Start(0))?;

    Ok((manifest_file, manifest, manifest_size))
}

/// Collects unique, validated blob descriptors without reopening duplicates.
///
/// The blob directory descriptor must originate from an authorized source.
#[cfg(target_os = "linux")]
fn collect_opened_export_blobs(
    blob_root_fd: &std::os::fd::OwnedFd,
    manifest: &Manifest,
) -> io::Result<Vec<OpenedExportBlob>> {
    let mut blobs: Vec<OpenedExportBlob> = Vec::new();

    for descriptor in manifest.config.iter().chain(&manifest.layers) {
        if let Some(existing) = blobs.iter().find(|blob| blob.digest == descriptor.digest) {
            if existing.size_bytes != descriptor.size {
                return Err(invalid("conflicting Ollama blob descriptors"));
            }
            continue;
        }

        let file = open_ollama_export_blob(blob_root_fd, &descriptor.digest, descriptor.size)?;

        blobs.push(OpenedExportBlob {
            digest: descriptor.digest.clone(),
            file,
            size_bytes: descriptor.size,
        });
    }

    Ok(blobs)
}

/// Builds a descriptor-owning plan from previously validated source handles.
///
/// The caller must independently authorize the source and retain any required
/// mount lifetime. This function does not establish process isolation.
#[cfg(target_os = "linux")]
fn assemble_opened_ollama_export_plan(
    manifest_file: fs::File,
    blob_root_fd: &std::os::fd::OwnedFd,
    manifest_relative_path: &Path,
) -> io::Result<OpenedOllamaExportPlan> {
    let (manifest_file, manifest, manifest_size) = prepare_opened_export_manifest(manifest_file)?;

    let blobs = collect_opened_export_blobs(blob_root_fd, &manifest)?;

    OpenedOllamaExportPlan::new(manifest_file, blobs, manifest_size, manifest_relative_path)
}

/// Copies a retained blob descriptor with the existing SHA-256 verification.
///
/// The caller must provide a separately authorized destination writer.
/// This does not establish process isolation or destination publication.
#[cfg(target_os = "linux")]
fn copy_opened_export_blob<W, F>(
    blob: &mut OpenedExportBlob,
    writer: &mut W,
    progress: F,
) -> io::Result<u64>
where
    W: Write,
    F: FnMut(u64),
{
    copy_blob_with_sha256_verification(
        &mut blob.file,
        writer,
        &blob.digest,
        blob.size_bytes,
        progress,
    )
}

/// Creates and opens a new directory beneath a retained parent descriptor.
///
/// The parent must already be independently authorized and isolated from
/// untrusted processes. This helper does not establish same-UID isolation.
#[cfg(target_os = "linux")]
fn create_opened_export_directory(
    parent: &std::os::fd::OwnedFd,
    name: &std::ffi::OsStr,
) -> io::Result<std::os::fd::OwnedFd> {
    use rustix::fs::{Mode, OFlags};
    use std::path::{Component, Path};

    let mut components = Path::new(name).components();

    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "export directory name must be a single normal component",
        ));
    }

    let parent_metadata = rustix::fs::fstat(parent)?;

    rustix::fs::mkdirat(parent, name, Mode::from_raw_mode(0o700))?;

    let directory = rustix::fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    let metadata = rustix::fs::fstat(&directory)?;

    if rustix::fs::FileType::from_raw_mode(metadata.st_mode) != rustix::fs::FileType::Directory
        || metadata.st_uid != rustix::process::geteuid().as_raw()
        || metadata.st_mode & 0o7777 != 0o700
        || metadata.st_dev != parent_metadata.st_dev
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe export directory metadata",
        ));
    }

    Ok(directory)
}

/// Creates the registry/namespace/model hierarchy under an opened
/// manifests directory, returning the model directory descriptor.
///
/// The caller must supply an independently authorized and isolated parent.
/// This helper does not establish protection from malicious same-UID processes.
#[cfg(target_os = "linux")]
fn create_opened_export_manifest_parent(
    manifests_directory: &std::os::fd::OwnedFd,
    relative_manifest_path: &Path,
) -> io::Result<std::os::fd::OwnedFd> {
    use std::path::Component;

    let components: Vec<_> = relative_manifest_path.components().collect();

    if components.len() != 4
        || components
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid("invalid Ollama export manifest identity"));
    }

    let mut current = manifests_directory.try_clone()?;

    for component in &components[..3] {
        let child = create_opened_export_directory(&current, component.as_os_str())?;

        rustix::fs::fsync(&child)?;
        rustix::fs::fsync(&current)?;

        current = child;
    }

    Ok(current)
}

/// Exclusively creates a regular export file beneath a retained directory.
///
/// The caller must authorize the destination directory independently.
/// This helper does not establish isolation from same-UID processes.
#[cfg(target_os = "linux")]
fn create_opened_export_file(
    directory: &std::os::fd::OwnedFd,
    name: &std::ffi::OsStr,
) -> io::Result<fs::File> {
    use rustix::fs::{Mode, OFlags};
    use std::path::{Component, Path};

    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "export filename must be a single normal component",
        ));
    }

    let descriptor = rustix::fs::openat(
        directory,
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )?;

    let metadata = rustix::fs::fstat(&descriptor)?;
    if rustix::fs::FileType::from_raw_mode(metadata.st_mode) != rustix::fs::FileType::RegularFile {
        return Err(invalid("unsafe export destination file"));
    }

    Ok(fs::File::from(descriptor))
}

/// Copies a retained blob into a newly created destination file.
///
/// The destination directory must be independently authorized. The caller
/// remains responsible for cleanup, directory synchronization, publication,
/// and isolation from same-UID processes.
#[cfg(target_os = "linux")]
fn write_opened_export_blob<F>(
    blob: &mut OpenedExportBlob,
    destination_directory: &std::os::fd::OwnedFd,
    destination_name: &std::ffi::OsStr,
    progress: F,
) -> io::Result<u64>
where
    F: FnMut(u64),
{
    let mut destination = create_opened_export_file(destination_directory, destination_name)?;

    let copied = copy_opened_export_blob(blob, &mut destination, progress)?;
    destination.sync_all()?;
    Ok(copied)
}

/// Copies a retained manifest into an exclusively created destination file.
///
/// This preserves the original manifest bytes without reserialization.
/// The caller must authorize and isolate the destination directory, verify
/// the complete export, and synchronize directories before publication.
#[cfg(target_os = "linux")]
fn write_opened_export_manifest(
    manifest: &mut fs::File,
    expected_size: u64,
    destination_directory: &std::os::fd::OwnedFd,
    destination_name: &std::ffi::OsStr,
) -> io::Result<u64> {
    use std::io::{Seek, SeekFrom};

    manifest.seek(SeekFrom::Start(0))?;

    let mut destination = create_opened_export_file(destination_directory, destination_name)?;

    let mut limited = manifest.take(expected_size.saturating_add(1));
    let copied = io::copy(&mut limited, &mut destination)?;

    if copied != expected_size {
        return Err(invalid("Ollama export manifest size mismatch"));
    }

    destination.sync_all()?;
    Ok(copied)
}

/// Writes an opened Ollama export plan into an existing staging directory.
///
/// The caller must supply an independently authorized, isolated staging
/// directory. This function does not publish the export or establish
/// protection against malicious same-UID processes.
#[cfg(target_os = "linux")]
fn write_opened_ollama_export_plan(
    plan: &mut OpenedOllamaExportPlan,
    staging_directory: &std::os::fd::OwnedFd,
) -> io::Result<u64> {
    use std::ffi::OsStr;

    let manifests = create_opened_export_directory(staging_directory, OsStr::new("manifests"))?;

    let blobs = create_opened_export_directory(staging_directory, OsStr::new("blobs"))?;

    let manifest_parent =
        create_opened_export_manifest_parent(&manifests, &plan.manifest_relative_path)?;

    let manifest_name = plan
        .manifest_relative_path
        .file_name()
        .ok_or_else(|| invalid("missing Ollama export manifest tag"))?;

    let mut written = write_opened_export_manifest(
        &mut plan.manifest,
        plan.manifest_size_bytes,
        &manifest_parent,
        manifest_name,
    )?;

    for blob in &mut plan.blobs {
        let name = blob_filename(&blob.digest)?;

        let copied = write_opened_export_blob(blob, &blobs, OsStr::new(&name), |_| {})?;

        written = written
            .checked_add(copied)
            .ok_or_else(|| invalid("export byte count overflow"))?;
    }

    if written != plan.total_bytes {
        return Err(invalid("export byte count mismatch"));
    }

    rustix::fs::fsync(&manifest_parent)?;
    rustix::fs::fsync(&manifests)?;
    rustix::fs::fsync(&blobs)?;
    rustix::fs::fsync(staging_directory)?;

    Ok(written)
}

/// Exports one Ollama model from a separately authorized model store.
///
/// The caller must establish source authorization and process isolation.
/// The destination must pass the existing non-root ownership and filesystem
/// checks. This operation does not grant access to protected Ollama storage.
#[cfg(target_os = "linux")]
pub fn export_authorized_ollama_model(
    model_store: &Path,
    manifest_relative_path: &Path,
    destination_parent: &Path,
    destination_name: &std::ffi::OsStr,
) -> Result<ExportPublicationOutcome, ExportPublicationError> {
    use rustix::fs::{Mode, OFlags, ResolveFlags};

    let not_published = |source: io::Error| ExportPublicationError {
        outcome: ExportPublicationOutcome::NotPublished,
        source,
    };

    if manifest_relative_path.components().count() != 4
        || manifest_relative_path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(not_published(invalid(
            "invalid Ollama export manifest identity",
        )));
    }

    let store = rustix::fs::openat2(
        rustix::fs::CWD,
        model_store,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS,
    )
    .map_err(|error| not_published(error.into()))?;

    let manifests = rustix::fs::openat(
        &store,
        "manifests",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| not_published(error.into()))?;

    let blobs = rustix::fs::openat(
        &store,
        "blobs",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| not_published(error.into()))?;

    let manifest =
        open_ollama_export_manifest(&manifests, manifest_relative_path).map_err(not_published)?;

    let mut plan = assemble_opened_ollama_export_plan(manifest, &blobs, manifest_relative_path)
        .map_err(not_published)?;

    let staging =
        create_handle_anchored_export_staging(destination_parent).map_err(not_published)?;

    write_opened_ollama_export_plan(&mut plan, &staging.directory).map_err(not_published)?;

    staging.publish(destination_name)
}

/// Validates a manifest and resolves the blobs required for offline export.
///
/// This function never modifies the source model store.
pub fn plan_ollama_export(
    model_store: &Path,
    manifest_path: &Path,
) -> io::Result<OllamaExportPlan> {
    let manifest_root = model_store.join("manifests");
    let blob_root = model_store.join("blobs");

    let root_metadata = fs::symlink_metadata(model_store)?;
    if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
        return Err(invalid("invalid model store directory"));
    }

    for directory in [&manifest_root, &blob_root] {
        let metadata = fs::symlink_metadata(directory)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(invalid("unsafe Ollama model store directory"));
        }
    }

    let relative = manifest_path
        .strip_prefix(&manifest_root)
        .map_err(|_| invalid("manifest outside Ollama manifest directory"))?;

    if relative
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
        || relative.components().count() < 3
    {
        return Err(invalid("invalid Ollama manifest path"));
    }

    let mut current = manifest_root.clone();
    let components: Vec<_> = relative.components().collect();

    for component in &components[..components.len() - 1] {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(invalid("unsafe manifest directory"));
        }
    }

    use rustix::fs::{Mode, OFlags};

    let manifest_root_fd = rustix::fs::openat(
        rustix::fs::CWD,
        &manifest_root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    let mut manifest_file = open_ollama_export_manifest(&manifest_root_fd, relative)?;
    let (manifest, mut total_bytes) = read_ollama_export_manifest(&mut manifest_file)?;

    let blob_root_fd = rustix::fs::openat(
        rustix::fs::CWD,
        &blob_root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    let mut blobs = Vec::new();

    for descriptor in manifest.config.iter().chain(&manifest.layers) {
        let filename = blob_filename(&descriptor.digest)?;
        let path = blob_root.join(&filename);

        let _blob_file =
            open_ollama_export_blob(&blob_root_fd, &descriptor.digest, descriptor.size)?;

        if let Some(existing) = blobs
            .iter()
            .find(|blob: &&ExportBlob| blob.digest == descriptor.digest)
        {
            if existing.size_bytes != descriptor.size {
                return Err(invalid("conflicting Ollama blob descriptors"));
            }
            continue;
        }

        total_bytes = total_bytes
            .checked_add(descriptor.size)
            .ok_or_else(|| invalid("export size overflow"))?;

        blobs.push(ExportBlob {
            digest: descriptor.digest.clone(),
            path,
            size_bytes: descriptor.size,
        });
    }

    Ok(OllamaExportPlan {
        manifest_path: manifest_path.to_path_buf(),
        blobs,
        total_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ollama_export_manifest_from_bytes() {
        let bytes = br#"{"layers":[{"digest":"sha256:abc","size":7}]}"#;
        let parsed = parse_ollama_export_manifest(bytes).unwrap();
        assert_eq!(parsed.layers.len(), 1);
        assert_eq!(parsed.layers[0].size, 7);
    }

    #[test]
    fn reads_ollama_export_manifest_from_stream() {
        let bytes = br#"{"layers":[{"digest":"sha256:abc","size":7}]}"#;
        let mut reader = std::io::Cursor::new(bytes);

        let (manifest, size) = read_ollama_export_manifest(&mut reader).unwrap();

        assert_eq!(manifest.layers.len(), 1);
        assert_eq!(size, bytes.len() as u64);
    }

    #[test]
    fn rejects_oversized_ollama_export_manifest_stream() {
        let bytes = vec![b' '; MAX_OLLAMA_EXPORT_MANIFEST_BYTES as usize + 1];
        let mut reader = std::io::Cursor::new(bytes);

        assert!(read_ollama_export_manifest(&mut reader).is_err());
    }

    #[test]
    fn rejects_invalid_ollama_export_manifest_bytes() {
        assert!(parse_ollama_export_manifest(b"{invalid").is_err());
    }

    #[test]
    fn rejects_oversized_ollama_export_manifest_bytes() {
        let oversized = vec![b' '; MAX_OLLAMA_EXPORT_MANIFEST_BYTES as usize + 1];
        assert!(parse_ollama_export_manifest(&oversized).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn handle_anchored_staging_creates_and_cleans_empty_directory() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();

        let name = {
            let staging = create_handle_anchored_export_staging(directory.path()).unwrap();

            let path = directory.path().join(staging.name());

            assert!(path.is_dir());
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o700
            );

            staging.name().to_owned()
        };

        assert!(!directory.path().join(name).exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn export_publication_outcome_distinguishes_durability_failure() {
        assert_eq!(
            classify_export_publication_outcome(false, false),
            ExportPublicationOutcome::NotPublished,
        );

        assert_eq!(
            classify_export_publication_outcome(true, true),
            ExportPublicationOutcome::PublishedAndSynced,
        );

        assert_eq!(
            classify_export_publication_outcome(true, false),
            ExportPublicationOutcome::PublishedDurabilityUncertain,
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn export_publication_error_preserves_outcome_and_cause() {
        let error = ExportPublicationError {
            outcome: ExportPublicationOutcome::PublishedDurabilityUncertain,
            source: io::Error::other("simulated parent fsync failure"),
        };

        assert_eq!(
            error.outcome,
            ExportPublicationOutcome::PublishedDurabilityUncertain,
        );

        assert_eq!(error.source.kind(), io::ErrorKind::Other,);

        let not_published = ExportPublicationError {
            outcome: ExportPublicationOutcome::NotPublished,
            source: io::Error::new(io::ErrorKind::AlreadyExists, "destination already exists"),
        };

        assert_eq!(
            not_published.outcome,
            ExportPublicationOutcome::NotPublished,
        );

        assert_eq!(not_published.source.kind(), io::ErrorKind::AlreadyExists,);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn authorized_ollama_export_preserves_manifest_and_blob_without_overwrite() {
        use sha2::{Digest, Sha256};
        use std::ffi::OsStr;
        use std::os::unix::fs::PermissionsExt;

        if rustix::process::geteuid().as_raw() == 0 {
            return;
        }

        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let store = root.path().join("models");
        let manifest_directory = store.join("manifests/registry.ollama.ai/library/example");
        let blob_directory = store.join("blobs");
        fs::create_dir_all(&manifest_directory).unwrap();
        fs::create_dir_all(&blob_directory).unwrap();

        let blob = b"verified offline Ollama model";
        let hash = format!("{:x}", Sha256::digest(blob));
        let digest = format!("sha256:{hash}");
        fs::write(blob_directory.join(format!("sha256-{hash}")), blob).unwrap();

        let manifest = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 2,
            "config": {"digest": digest, "size": blob.len()},
            "layers": []
        }))
        .unwrap();

        fs::write(manifest_directory.join("latest"), &manifest).unwrap();

        let destination = root.path().join("exports");
        fs::create_dir(&destination).unwrap();
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o700)).unwrap();

        let relative = Path::new("registry.ollama.ai/library/example/latest");
        let name = OsStr::new("example-export");

        let outcome = export_authorized_ollama_model(&store, relative, &destination, name).unwrap();

        assert_eq!(outcome, ExportPublicationOutcome::PublishedAndSynced);
        assert_eq!(
            fs::read(destination.join("example-export/manifests").join(relative)).unwrap(),
            manifest
        );
        assert_eq!(
            fs::read(
                destination
                    .join("example-export/blobs")
                    .join(format!("sha256-{hash}"))
            )
            .unwrap(),
            blob
        );

        assert!(
            export_authorized_ollama_model(&store, relative, &destination, name).is_err(),
            "existing exports must not be overwritten"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn handle_anchored_publication_succeeds() {
        use std::ffi::OsStr;
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let staging = create_handle_anchored_export_staging(directory.path()).unwrap();

        let original_name = staging.name().to_owned();

        let outcome = staging.publish(OsStr::new("exported-model")).unwrap();
        assert_eq!(outcome, ExportPublicationOutcome::PublishedAndSynced,);

        assert!(directory.path().join("exported-model").is_dir());
        assert!(!directory.path().join(original_name).exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn handle_anchored_publication_reports_post_rename_sync_failure() {
        use std::ffi::OsStr;
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();

        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();

        let staging = create_handle_anchored_export_staging(directory.path()).unwrap();

        let staging_name = staging.name().to_owned();

        let error = staging
            .publish_with_sync(OsStr::new("exported-model"), |_| {
                Err(io::Error::other("injected parent fsync failure"))
            })
            .unwrap_err();

        assert_eq!(
            error.outcome,
            ExportPublicationOutcome::PublishedDurabilityUncertain,
        );

        assert_eq!(error.source.to_string(), "injected parent fsync failure",);

        assert!(directory.path().join("exported-model").is_dir());
        assert!(!directory.path().join(staging_name).exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn handle_anchored_publication_rejects_replaced_staging() {
        use std::ffi::OsStr;
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();

        let staging = create_handle_anchored_export_staging(directory.path()).unwrap();

        let original_path = directory.path().join(staging.name());
        let relocated = directory.path().join("relocated-original");

        fs::rename(&original_path, &relocated).unwrap();

        fs::create_dir(&original_path).unwrap();
        fs::write(original_path.join("protected-model"), b"must not publish").unwrap();

        let destination = directory.path().join("exported-model");

        assert!(
            staging.publish(OsStr::new("exported-model")).is_err(),
            "publication must reject replacement staging directories"
        );

        assert!(
            !destination.exists(),
            "replacement staging must never be published"
        );

        assert_eq!(
            fs::read(original_path.join("protected-model")).unwrap(),
            b"must not publish"
        );

        assert!(relocated.is_dir());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn handle_anchored_publication_preserves_existing_destination() {
        use std::ffi::OsStr;
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let destination = directory.path().join("exported-model");

        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("marker"), b"preserve").unwrap();

        let staging = create_handle_anchored_export_staging(directory.path()).unwrap();

        let staging_name = staging.name().to_owned();

        let error = staging.publish(OsStr::new("exported-model")).unwrap_err();

        assert_eq!(error.outcome, ExportPublicationOutcome::NotPublished,);
        assert_eq!(error.source.kind(), io::ErrorKind::AlreadyExists);

        assert_eq!(fs::read(destination.join("marker")).unwrap(), b"preserve");
        assert!(!directory.path().join(staging_name).exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn handle_anchored_publication_rejects_path_traversal() {
        use std::ffi::OsStr;
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let staging = create_handle_anchored_export_staging(directory.path()).unwrap();

        assert!(staging.publish(OsStr::new("../escape")).is_err());
        assert!(!directory.path().parent().unwrap().join("escape").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn handle_anchored_staging_cleans_populated_directory() {
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let outside = tempfile::tempdir().unwrap();

        let protected = outside.path().join("protected-model");
        fs::write(&protected, b"must survive").unwrap();

        let staging_name = {
            let staging = create_handle_anchored_export_staging(directory.path()).unwrap();

            let staging_path = directory.path().join(staging.name());

            fs::write(staging_path.join("partial-blob"), b"partial").unwrap();
            symlink(&protected, staging_path.join("external-link")).unwrap();

            staging.name().to_owned()
        };

        assert!(
            !directory.path().join(staging_name).exists(),
            "unpublished populated staging must be removed"
        );

        assert_eq!(
            fs::read(&protected).unwrap(),
            b"must survive",
            "cleanup must not follow symlinks"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn handle_anchored_cleanup_preserves_replacement_directory() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();

        let staging = create_handle_anchored_export_staging(directory.path()).unwrap();

        let staging_path = directory.path().join(staging.name());
        let relocated = directory.path().join("relocated-staging");

        fs::rename(&staging_path, &relocated).unwrap();

        fs::create_dir(&staging_path).unwrap();

        let protected = staging_path.join("protected-model");
        fs::write(&protected, b"must survive").unwrap();

        drop(staging);

        assert_eq!(
            fs::read(&protected).unwrap(),
            b"must survive",
            "cleanup must not delete contents of a replacement directory"
        );

        assert!(
            relocated.exists(),
            "relocated original staging directory must remain untouched"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn export_destination_sticky_ancestor_policy_is_restricted() {
        assert!(is_safe_export_sticky_ancestor(1000, 0, 0o1777));
        assert!(is_safe_export_sticky_ancestor(1001, 0, 0o1777));

        assert!(!is_safe_export_sticky_ancestor(0, 0, 0o1777));
        assert!(!is_safe_export_sticky_ancestor(1000, 1001, 0o1777));
        assert!(!is_safe_export_sticky_ancestor(1000, 1000, 0o1777));
        assert!(!is_safe_export_sticky_ancestor(1000, 0, 0o777));
        assert!(!is_safe_export_sticky_ancestor(1000, 0, 0o1775));
        assert!(!is_safe_export_sticky_ancestor(1000, 0, 0o1755));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn export_destination_ancestor_policy_rejects_unsafe_permissions() {
        assert!(is_safe_export_ancestor(1000, 0, 0o755));
        assert!(is_safe_export_ancestor(1000, 1000, 0o700));
        assert!(is_safe_export_ancestor(1000, 1000, 0o755));

        assert!(!is_safe_export_ancestor(1000, 1001, 0o755));
        assert!(!is_safe_export_ancestor(1000, 1000, 0o775));
        assert!(!is_safe_export_ancestor(1000, 1000, 0o777));
        assert!(!is_safe_export_ancestor(1000, 0, 0o1777));
        assert!(!is_safe_export_ancestor(0, 0, 0o755));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn export_destination_ownership_policy_requires_current_non_root_user() {
        assert!(is_safe_export_destination_owner(1000, 1000));
        assert!(is_safe_export_destination_owner(1001, 1001));

        assert!(!is_safe_export_destination_owner(0, 0));
        assert!(!is_safe_export_destination_owner(1000, 0));
        assert!(!is_safe_export_destination_owner(1000, 1001));
        assert!(!is_safe_export_destination_owner(1001, 1000));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn export_destination_filesystem_policy_rejects_unsupported_types() {
        assert!(is_supported_export_destination_filesystem(0xEF53));
        assert!(is_supported_export_destination_filesystem(0x58465342));
        assert!(is_supported_export_destination_filesystem(0x9123683E));
        assert!(is_supported_export_destination_filesystem(0x01021994));

        assert!(!is_supported_export_destination_filesystem(0x4d44));
        assert!(!is_supported_export_destination_filesystem(0x2011BAB0));
        assert!(!is_supported_export_destination_filesystem(0x65735546));
        assert!(!is_supported_export_destination_filesystem(0));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn handle_anchored_staging_rejects_group_writable_ancestor() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let shared = directory.path().join("shared");
        let destination = shared.join("private-export");

        fs::create_dir(&shared).unwrap();
        fs::create_dir(&destination).unwrap();

        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o770)).unwrap();
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o700)).unwrap();

        let result = create_handle_anchored_export_staging(&destination);

        assert!(
            result.is_err(),
            "group-writable ancestor must be rejected even when destination is private"
        );

        assert_eq!(
            fs::metadata(&shared).unwrap().permissions().mode() & 0o777,
            0o770,
            "validation must not modify ancestor permissions"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn handle_anchored_staging_rejects_group_writable_parent() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("shared-export");

        fs::create_dir(&destination).unwrap();

        fs::set_permissions(&destination, fs::Permissions::from_mode(0o770)).unwrap();

        let result = create_handle_anchored_export_staging(&destination);

        assert!(
            result.is_err(),
            "group-writable destination parent must be rejected"
        );

        assert_eq!(
            fs::metadata(&destination).unwrap().permissions().mode() & 0o777,
            0o770,
            "export validation must not modify destination permissions"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn handle_anchored_staging_rejects_symlinked_parent() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let real = directory.path().join("real");
        let linked = directory.path().join("linked");

        fs::create_dir(&real).unwrap();
        symlink(&real, &linked).unwrap();

        assert!(create_handle_anchored_export_staging(&linked).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn publishes_completed_export_without_overwrite() {
        let directory = tempfile::tempdir().unwrap();
        let staging = create_private_export_staging(directory.path()).unwrap();
        fs::write(staging.path().join("manifest"), b"test").unwrap();

        let destination = directory.path().join("exported-model");
        publish_completed_ollama_export(staging, &destination).unwrap();

        assert_eq!(fs::read(destination.join("manifest")).unwrap(), b"test");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn publication_rejects_symlinked_ancestor() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let real_parent = directory.path().join("real");
        let linked_parent = directory.path().join("linked");

        fs::create_dir(&real_parent).unwrap();
        symlink(&real_parent, &linked_parent).unwrap();

        let staging = create_private_export_staging(&real_parent).unwrap();

        // Construct a path containing a symlinked ancestor while retaining
        // the same lexical parent as the staging directory.
        let staging_path = staging.path().to_path_buf();
        let staging_name = staging_path.file_name().unwrap().to_owned();

        let linked_staging = linked_parent.join(&staging_name);
        assert!(linked_staging.exists());

        // The secure parent opening must reject this path.
        use rustix::fs::{Mode, OFlags, ResolveFlags};

        let root_fd = rustix::fs::open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let relative = linked_parent.strip_prefix("/").unwrap();

        assert!(
            rustix::fs::openat2(
                &root_fd,
                relative,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
            )
            .is_err()
        );

        assert!(staging_path.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn publication_helper_rejects_symlinked_ancestor() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let real_parent = directory.path().join("real");
        let linked_parent = directory.path().join("linked");

        fs::create_dir(&real_parent).unwrap();
        symlink(&real_parent, &linked_parent).unwrap();

        // Create staging through the symlinked path. The existing staging
        // helper checks the final component, so this currently succeeds.
        let staging = create_private_export_staging(&linked_parent.join(".")).unwrap();

        let destination = linked_parent.join(".").join("exported-model");

        fs::write(staging.path().join("marker"), b"preserve").unwrap();

        let result = publish_completed_ollama_export(staging, &destination);

        assert!(result.is_err());
        assert!(!real_parent.join("exported-model").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn publication_rejects_existing_destination() {
        let directory = tempfile::tempdir().unwrap();
        let staging = create_private_export_staging(directory.path()).unwrap();
        let staging_path = staging.path().to_path_buf();
        let destination = directory.path().join("exported-model");

        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("existing"), b"preserve").unwrap();

        assert!(publish_completed_ollama_export(staging, &destination).is_err());

        assert_eq!(fs::read(destination.join("existing")).unwrap(), b"preserve");
        assert!(!staging_path.exists());
    }

    #[test]
    fn private_export_staging_is_cleaned_up() {
        use std::os::unix::fs::PermissionsExt;

        let destination = tempfile::tempdir().unwrap();

        let staging_path = {
            let staging = create_private_export_staging(destination.path()).unwrap();

            let path = staging.path().to_path_buf();

            assert!(path.is_dir());
            assert!(path.starts_with(destination.path()));

            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;

            assert_eq!(mode, 0o700);

            path
        };

        assert!(!staging_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn private_export_staging_rejects_symlink_parent() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target");
        let link = directory.path().join("link");

        fs::create_dir(&target).unwrap();
        symlink(&target, &link).unwrap();

        assert!(create_private_export_staging(&link).is_err());
    }

    #[test]
    fn verified_blob_copy_succeeds() {
        use std::io::Cursor;

        let bytes = b"test";
        let digest = format!("sha256:{:x}", Sha256::digest(bytes));

        let mut reader = Cursor::new(bytes);
        let mut output = Vec::new();
        let mut updates = Vec::new();

        let copied = copy_blob_with_sha256_verification(
            &mut reader,
            &mut output,
            &digest,
            bytes.len() as u64,
            |count| updates.push(count),
        )
        .unwrap();

        assert_eq!(copied, 4);
        assert_eq!(output, bytes);
        assert_eq!(updates, vec![4]);
    }

    #[test]
    fn verified_blob_copy_rejects_wrong_digest() {
        use std::io::Cursor;

        let mut reader = Cursor::new(b"test");
        let mut output = Vec::new();

        let error = copy_blob_with_sha256_verification(
            &mut reader,
            &mut output,
            &format!("sha256:{}", "a".repeat(64)),
            4,
            |_| {},
        )
        .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn verified_blob_copy_rejects_wrong_size() {
        use std::io::Cursor;

        let bytes = b"test";
        let digest = format!("sha256:{:x}", Sha256::digest(bytes));

        let mut reader = Cursor::new(bytes);
        let mut output = Vec::new();

        assert!(
            copy_blob_with_sha256_verification(&mut reader, &mut output, &digest, 5, |_| {},)
                .is_err()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_export_directory_creation_is_exclusive_and_private() {
        use rustix::fs::{Mode, OFlags};
        use std::ffi::OsStr;

        let directory = tempfile::tempdir().unwrap();
        let parent = rustix::fs::openat(
            rustix::fs::CWD,
            directory.path(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let name = OsStr::new("models");
        let child = create_opened_export_directory(&parent, name).unwrap();

        let metadata = rustix::fs::fstat(&child).unwrap();
        assert_eq!(metadata.st_mode & 0o7777, 0o700);
        assert_eq!(metadata.st_uid, rustix::process::geteuid().as_raw());
        assert_eq!(metadata.st_dev, rustix::fs::fstat(&parent).unwrap().st_dev);
        assert!(directory.path().join(name).is_dir());

        let error = create_opened_export_directory(&parent, name).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_export_directory_creation_rejects_traversal_and_symlinks() {
        use rustix::fs::{Mode, OFlags};
        use std::ffi::OsStr;
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let parent = rustix::fs::openat(
            rustix::fs::CWD,
            directory.path(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        for name in ["../escape", "nested/path", ".", "..", "/absolute"] {
            let error = create_opened_export_directory(&parent, OsStr::new(name)).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        }

        let target = directory.path().join("target");
        fs::create_dir(&target).unwrap();
        symlink(&target, directory.path().join("linked")).unwrap();

        assert!(create_opened_export_directory(&parent, OsStr::new("linked")).is_err());

        assert!(target.is_dir());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_export_file_creation_is_exclusive_and_private() {
        use rustix::fs::{Mode, OFlags};
        use std::ffi::OsStr;
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let directory_fd = rustix::fs::openat(
            rustix::fs::CWD,
            directory.path(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let name = OsStr::new("model-blob");
        let mut file = create_opened_export_file(&directory_fd, name).unwrap();
        file.write_all(b"verified export").unwrap();
        file.sync_all().unwrap();

        let metadata = fs::metadata(directory.path().join(name)).unwrap();
        assert!(metadata.is_file());
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        assert_eq!(
            fs::read(directory.path().join(name)).unwrap(),
            b"verified export"
        );

        let error = create_opened_export_file(&directory_fd, name).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read(directory.path().join(name)).unwrap(),
            b"verified export"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_export_file_creation_rejects_traversal_and_symlinks() {
        use rustix::fs::{Mode, OFlags};
        use std::ffi::OsStr;
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let directory_fd = rustix::fs::openat(
            rustix::fs::CWD,
            directory.path(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        for name in ["../escape", "subdir/file", ".", "..", "/tmp/escape"] {
            let error = create_opened_export_file(&directory_fd, OsStr::new(name)).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{name}");
        }

        let target = directory.path().join("target");
        fs::write(&target, b"unchanged").unwrap();
        symlink(&target, directory.path().join("link")).unwrap();

        assert!(create_opened_export_file(&directory_fd, OsStr::new("link")).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"unchanged");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_export_manifest_write_preserves_bytes_and_rejects_collision() {
        use rustix::fs::{Mode, OFlags};
        use std::ffi::OsStr;

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source-manifest");
        let bytes = b"{ \"schemaVersion\" : 2, \"layers\" : [] }\\n";
        fs::write(&source, bytes).unwrap();

        let destination = directory.path().join("destination");
        fs::create_dir(&destination).unwrap();

        let destination_fd = rustix::fs::openat(
            rustix::fs::CWD,
            &destination,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let mut manifest = fs::File::open(&source).unwrap();
        let name = OsStr::new("latest");

        let copied =
            write_opened_export_manifest(&mut manifest, bytes.len() as u64, &destination_fd, name)
                .unwrap();

        assert_eq!(copied, bytes.len() as u64);
        assert_eq!(fs::read(destination.join(name)).unwrap(), bytes);

        let error =
            write_opened_export_manifest(&mut manifest, bytes.len() as u64, &destination_fd, name)
                .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(destination.join(name)).unwrap(), bytes);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_export_manifest_write_rejects_size_mismatch() {
        use rustix::fs::{Mode, OFlags};
        use std::ffi::OsStr;

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source-manifest");
        let bytes = b"manifest-data";
        fs::write(&source, bytes).unwrap();

        let destination = directory.path().join("destination");
        fs::create_dir(&destination).unwrap();

        let destination_fd = rustix::fs::openat(
            rustix::fs::CWD,
            &destination,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        for (name, expected_size) in [
            ("too-small", bytes.len() as u64 - 1),
            ("too-large", bytes.len() as u64 + 1),
        ] {
            let mut manifest = fs::File::open(&source).unwrap();

            let error = write_opened_export_manifest(
                &mut manifest,
                expected_size,
                &destination_fd,
                OsStr::new(name),
            )
            .unwrap_err();

            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_export_blob_write_succeeds_and_rejects_collision() {
        use rustix::fs::{Mode, OFlags};
        use std::ffi::OsStr;

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source");
        let bytes = b"verified model data";
        fs::write(&source, bytes).unwrap();

        let destination = directory.path().join("destination");
        fs::create_dir(&destination).unwrap();

        let destination_fd = rustix::fs::openat(
            rustix::fs::CWD,
            &destination,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let mut blob = OpenedExportBlob {
            digest: format!("sha256:{:x}", Sha256::digest(bytes)),
            file: fs::File::open(&source).unwrap(),
            size_bytes: bytes.len() as u64,
        };

        let copied =
            write_opened_export_blob(&mut blob, &destination_fd, OsStr::new("blob"), |_| {})
                .unwrap();

        assert_eq!(copied, bytes.len() as u64);
        assert_eq!(fs::read(destination.join("blob")).unwrap(), bytes);

        let error =
            write_opened_export_blob(&mut blob, &destination_fd, OsStr::new("blob"), |_| {})
                .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(destination.join("blob")).unwrap(), bytes);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_export_blob_write_rejects_digest_mismatch() {
        use rustix::fs::{Mode, OFlags};
        use std::ffi::OsStr;

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source");
        let bytes = b"tampered model data";
        fs::write(&source, bytes).unwrap();

        let destination = directory.path().join("destination");
        fs::create_dir(&destination).unwrap();

        let destination_fd = rustix::fs::openat(
            rustix::fs::CWD,
            &destination,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let mut blob = OpenedExportBlob {
            digest: format!("sha256:{:x}", Sha256::digest(b"original model data")),
            file: fs::File::open(&source).unwrap(),
            size_bytes: bytes.len() as u64,
        };

        let error =
            write_opened_export_blob(&mut blob, &destination_fd, OsStr::new("blob"), |_| {})
                .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("SHA-256 mismatch"));

        // A failed write must not be treated as a publishable export.
        // Cleanup of this partial destination belongs to the staging owner.
        assert!(destination.join("blob").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_blob_copy_verifies_original_after_pathname_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("blob");

        let bytes = b"original verified blob";
        let digest = format!("sha256:{:x}", Sha256::digest(bytes));
        fs::write(&path, bytes).unwrap();

        let mut blob = OpenedExportBlob {
            digest,
            file: fs::File::open(&path).unwrap(),
            size_bytes: bytes.len() as u64,
        };

        fs::rename(&path, directory.path().join("old-blob")).unwrap();
        fs::write(&path, b"replacement blob").unwrap();

        let mut output = Vec::new();
        let copied = copy_opened_export_blob(&mut blob, &mut output, |_| {}).unwrap();

        assert_eq!(copied, bytes.len() as u64);
        assert_eq!(output, bytes);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_blob_copy_rejects_in_place_source_modification() {
        use std::io::{Seek, SeekFrom};

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("blob");

        let bytes = b"original verified blob";
        let digest = format!("sha256:{:x}", Sha256::digest(bytes));
        fs::write(&path, bytes).unwrap();

        let mut blob = OpenedExportBlob {
            digest,
            file: fs::File::open(&path).unwrap(),
            size_bytes: bytes.len() as u64,
        };

        let mut attacker = fs::OpenOptions::new().write(true).open(&path).unwrap();
        attacker.seek(SeekFrom::Start(0)).unwrap();
        attacker.write_all(&vec![b'X'; bytes.len()]).unwrap();
        attacker.sync_all().unwrap();

        let mut output = Vec::new();
        let error = copy_opened_export_blob(&mut blob, &mut output, |_| {}).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("SHA-256 mismatch"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn assembled_opened_plan_retains_sources_after_pathname_replacement() {
        use rustix::fs::{Mode, OFlags};

        let directory = tempfile::tempdir().unwrap();
        let manifest_path = directory.path().join("manifest");
        let blob_root = directory.path().join("blobs");
        fs::create_dir(&blob_root).unwrap();

        let original_blob = b"original model bytes";
        let digest = format!("sha256:{:x}", Sha256::digest(original_blob));
        let blob_path = blob_root.join(blob_filename(&digest).unwrap());
        fs::write(&blob_path, original_blob).unwrap();

        let original_manifest = format!(
            r#"{{"layers":[{{"digest":"{}","size":{}}}]}}"#,
            digest,
            original_blob.len(),
        );
        fs::write(&manifest_path, original_manifest.as_bytes()).unwrap();

        let blob_root_fd = rustix::fs::openat(
            rustix::fs::CWD,
            &blob_root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let manifest_file = fs::File::open(&manifest_path).unwrap();
        let mut plan = assemble_opened_ollama_export_plan(
            manifest_file,
            &blob_root_fd,
            Path::new("registry.ollama.ai/library/example/latest"),
        )
        .unwrap();

        assert_eq!(plan.blobs.len(), 1);
        assert_eq!(
            plan.total_bytes,
            original_manifest.len() as u64 + original_blob.len() as u64
        );

        fs::rename(&manifest_path, directory.path().join("old-manifest")).unwrap();
        fs::write(&manifest_path, b"replacement manifest").unwrap();

        fs::rename(&blob_path, blob_root.join("old-blob")).unwrap();
        fs::write(&blob_path, b"replacement blob").unwrap();

        let mut retained_manifest = Vec::new();
        plan.manifest.read_to_end(&mut retained_manifest).unwrap();
        assert_eq!(retained_manifest, original_manifest.as_bytes());

        let mut retained_blob = Vec::new();
        plan.blobs[0].file.read_to_end(&mut retained_blob).unwrap();
        assert_eq!(retained_blob, original_blob);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_blob_collection_deduplicates_and_rejects_conflicts() {
        use rustix::fs::{Mode, OFlags};

        let directory = tempfile::tempdir().unwrap();
        let blob_root = directory.path().join("blobs");
        fs::create_dir(&blob_root).unwrap();

        let bytes = b"verified blob";
        let digest = format!("sha256:{:x}", Sha256::digest(bytes));
        let filename = blob_filename(&digest).unwrap();
        fs::write(blob_root.join(filename), bytes).unwrap();

        let root_fd = rustix::fs::openat(
            rustix::fs::CWD,
            &blob_root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let manifest = Manifest {
            config: Some(Descriptor {
                digest: digest.clone(),
                size: bytes.len() as u64,
            }),
            layers: vec![Descriptor {
                digest: digest.clone(),
                size: bytes.len() as u64,
            }],
        };

        let mut blobs = collect_opened_export_blobs(&root_fd, &manifest).unwrap();
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0].digest, digest);

        let mut copied = Vec::new();
        blobs[0].file.read_to_end(&mut copied).unwrap();
        assert_eq!(copied, bytes);

        let conflicting = Manifest {
            config: Some(Descriptor {
                digest: digest.clone(),
                size: bytes.len() as u64,
            }),
            layers: vec![Descriptor {
                digest,
                size: bytes.len() as u64 + 1,
            }],
        };

        let error = collect_opened_export_blobs(&root_fd, &conflicting).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("conflicting"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prepared_opened_manifest_is_rewound_for_copying() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("manifest");

        let bytes = br#"{"layers":[{"digest":"sha256:abc","size":7}]}"#;
        fs::write(&path, bytes).unwrap();

        let file = fs::File::open(&path).unwrap();
        let (mut retained, parsed, size) = prepare_opened_export_manifest(file).unwrap();

        assert_eq!(parsed.layers.len(), 1);
        assert_eq!(parsed.layers[0].size, 7);
        assert_eq!(size, bytes.len() as u64);

        fs::remove_file(&path).unwrap();

        let mut copied = Vec::new();
        retained.read_to_end(&mut copied).unwrap();

        assert_eq!(copied, bytes);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn failed_opened_export_assembly_cleans_unpublished_staging() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();

        let manifest_path = directory.path().join("source-manifest");
        let blob_path = directory.path().join("source-blob");

        let manifest_bytes = br#"{"layers":[]}"#;
        let original_blob = b"original model bytes";
        let corrupted_blob = b"corrupted model bytes";

        fs::write(&manifest_path, manifest_bytes).unwrap();
        fs::write(&blob_path, corrupted_blob).unwrap();

        let blob = OpenedExportBlob {
            digest: format!("sha256:{:x}", Sha256::digest(original_blob)),
            file: fs::File::open(&blob_path).unwrap(),
            size_bytes: corrupted_blob.len() as u64,
        };

        let mut plan = OpenedOllamaExportPlan::new(
            fs::File::open(&manifest_path).unwrap(),
            vec![blob],
            manifest_bytes.len() as u64,
            Path::new("registry.ollama.ai/library/example/latest"),
        )
        .unwrap();

        let staging_name = {
            let staging = create_handle_anchored_export_staging(directory.path()).unwrap();

            let staging_path = directory.path().join(staging.name());

            let error = write_opened_ollama_export_plan(&mut plan, &staging.directory).unwrap_err();

            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert!(error.to_string().contains("SHA-256 mismatch"));

            assert!(
                staging_path
                    .join("manifests/registry.ollama.ai/library/example/latest")
                    .exists()
            );
            assert!(staging_path.join("blobs").is_dir());

            staging.name().to_owned()
        };

        assert!(
            !directory.path().join(staging_name).exists(),
            "failed unpublished export staging must be removed"
        );

        assert_eq!(fs::read(&manifest_path).unwrap(), manifest_bytes);
        assert_eq!(fs::read(&blob_path).unwrap(), corrupted_blob);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_export_assembly_rejects_corrupted_blob() {
        use rustix::fs::{Mode, OFlags};

        let directory = tempfile::tempdir().unwrap();
        let manifest_path = directory.path().join("source-manifest");
        let blob_path = directory.path().join("source-blob");

        let manifest_bytes = br#"{"layers":[]}"#;
        let original_blob = b"original model bytes";
        let corrupted_blob = b"corrupted model bytes";

        fs::write(&manifest_path, manifest_bytes).unwrap();
        fs::write(&blob_path, corrupted_blob).unwrap();

        let digest = format!("sha256:{:x}", Sha256::digest(original_blob));
        let blob_name = blob_filename(&digest).unwrap();

        let blob = OpenedExportBlob {
            digest,
            file: fs::File::open(&blob_path).unwrap(),
            size_bytes: corrupted_blob.len() as u64,
        };

        let mut plan = OpenedOllamaExportPlan::new(
            fs::File::open(&manifest_path).unwrap(),
            vec![blob],
            manifest_bytes.len() as u64,
            Path::new("registry.ollama.ai/library/example/latest"),
        )
        .unwrap();

        let staging_path = directory.path().join("staging");
        fs::create_dir(&staging_path).unwrap();

        let staging_fd = rustix::fs::openat(
            rustix::fs::CWD,
            &staging_path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let error = write_opened_ollama_export_plan(&mut plan, &staging_fd).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("SHA-256 mismatch"));

        assert_eq!(
            fs::read(staging_path.join("manifests/registry.ollama.ai/library/example/latest"))
                .unwrap(),
            manifest_bytes
        );

        assert!(
            staging_path.join("blobs").join(blob_name).exists(),
            "partial blob may remain until staging owner performs cleanup"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_export_assembly_preserves_manifest_and_verified_blob() {
        use rustix::fs::{Mode, OFlags};

        let directory = tempfile::tempdir().unwrap();
        let manifest_path = directory.path().join("source-manifest");
        let blob_path = directory.path().join("source-blob");

        let manifest_bytes = br#"{ "layers" : [] }"#;
        let blob_bytes = b"verified offline model bytes";

        fs::write(&manifest_path, manifest_bytes).unwrap();
        fs::write(&blob_path, blob_bytes).unwrap();

        let digest = format!("sha256:{:x}", Sha256::digest(blob_bytes));
        let blob_name = blob_filename(&digest).unwrap();

        let blob = OpenedExportBlob {
            digest,
            file: fs::File::open(&blob_path).unwrap(),
            size_bytes: blob_bytes.len() as u64,
        };

        let mut plan = OpenedOllamaExportPlan::new(
            fs::File::open(&manifest_path).unwrap(),
            vec![blob],
            manifest_bytes.len() as u64,
            Path::new("registry.ollama.ai/library/example/latest"),
        )
        .unwrap();

        let staging_path = directory.path().join("staging");
        fs::create_dir(&staging_path).unwrap();

        let staging_fd = rustix::fs::openat(
            rustix::fs::CWD,
            &staging_path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let written = write_opened_ollama_export_plan(&mut plan, &staging_fd).unwrap();

        assert_eq!(
            written,
            manifest_bytes.len() as u64 + blob_bytes.len() as u64
        );
        assert_eq!(written, plan.total_bytes);

        assert_eq!(
            fs::read(staging_path.join("manifests/registry.ollama.ai/library/example/latest"))
                .unwrap(),
            manifest_bytes
        );

        assert_eq!(
            fs::read(staging_path.join("blobs").join(blob_name)).unwrap(),
            blob_bytes
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_export_assembly_publishes_verified_model_without_overwrite() {
        use std::ffi::OsStr;
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();

        let manifest_path = directory.path().join("source-manifest");
        let blob_path = directory.path().join("source-blob");

        let blob_bytes = b"verified offline model bytes";
        let digest = format!("sha256:{:x}", Sha256::digest(blob_bytes));
        let blob_name = blob_filename(&digest).unwrap();

        let manifest_bytes = format!(
            r#"{{"layers":[{{"digest":"{}","size":{}}}]}}"#,
            digest,
            blob_bytes.len(),
        );

        fs::write(&manifest_path, manifest_bytes.as_bytes()).unwrap();
        fs::write(&blob_path, blob_bytes).unwrap();

        let make_plan = || {
            OpenedOllamaExportPlan::new(
                fs::File::open(&manifest_path).unwrap(),
                vec![OpenedExportBlob {
                    digest: digest.clone(),
                    file: fs::File::open(&blob_path).unwrap(),
                    size_bytes: blob_bytes.len() as u64,
                }],
                manifest_bytes.len() as u64,
                Path::new("registry.ollama.ai/library/example/latest"),
            )
            .unwrap()
        };

        let mut plan = make_plan();
        let staging = create_handle_anchored_export_staging(directory.path()).unwrap();

        let written = write_opened_ollama_export_plan(&mut plan, &staging.directory).unwrap();

        assert_eq!(written, plan.total_bytes);

        let outcome = staging.publish(OsStr::new("exported-model")).unwrap();
        assert_eq!(outcome, ExportPublicationOutcome::PublishedAndSynced);

        let destination = directory.path().join("exported-model");

        assert_eq!(
            fs::read(destination.join("manifests/registry.ollama.ai/library/example/latest"))
                .unwrap(),
            manifest_bytes.as_bytes(),
        );

        assert_eq!(
            fs::read(destination.join("blobs").join(blob_name)).unwrap(),
            blob_bytes,
        );

        let mut second_plan = make_plan();
        let second_staging = create_handle_anchored_export_staging(directory.path()).unwrap();
        let second_name = second_staging.name().to_owned();

        write_opened_ollama_export_plan(&mut second_plan, &second_staging.directory).unwrap();

        let error = second_staging
            .publish(OsStr::new("exported-model"))
            .unwrap_err();

        assert_eq!(error.outcome, ExportPublicationOutcome::NotPublished);
        assert!(!directory.path().join(second_name).exists());

        assert_eq!(
            fs::read(destination.join("manifests/registry.ollama.ai/library/example/latest"))
                .unwrap(),
            manifest_bytes.as_bytes(),
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_export_manifest_parent_preserves_model_hierarchy() {
        use rustix::fs::{Mode, OFlags};

        let directory = tempfile::tempdir().unwrap();
        let manifests = directory.path().join("manifests");
        fs::create_dir(&manifests).unwrap();

        let root = rustix::fs::openat(
            rustix::fs::CWD,
            &manifests,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let relative = Path::new("registry.ollama.ai/library/example/latest");
        let model_directory = create_opened_export_manifest_parent(&root, relative).unwrap();

        let expected = manifests.join("registry.ollama.ai/library/example");
        assert!(expected.is_dir());

        let metadata = fs::metadata(&expected).unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(metadata.permissions().mode() & 0o777, 0o700);

        let name = relative.file_name().unwrap();
        let mut manifest = create_opened_export_file(&model_directory, name).unwrap();
        manifest.write_all(b"preserved manifest").unwrap();

        assert_eq!(
            fs::read(expected.join("latest")).unwrap(),
            b"preserved manifest"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opened_export_manifest_parent_rejects_invalid_identity() {
        use rustix::fs::{Mode, OFlags};

        let directory = tempfile::tempdir().unwrap();
        let root = rustix::fs::openat(
            rustix::fs::CWD,
            directory.path(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        for identity in [
            "../library/example/latest",
            "/registry.ollama.ai/library/example/latest",
            "registry.ollama.ai/library/example",
            "registry.ollama.ai/library/example/latest/extra",
        ] {
            let error =
                create_opened_export_manifest_parent(&root, Path::new(identity)).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }
    }

    #[test]
    fn opened_export_plan_retains_files_and_counts_bytes() {
        let directory = tempfile::tempdir().unwrap();

        let manifest_path = directory.path().join("manifest");
        let blob_path = directory.path().join("blob");

        fs::write(&manifest_path, b"manifest").unwrap();
        fs::write(&blob_path, b"blob").unwrap();

        let manifest = fs::File::open(&manifest_path).unwrap();
        let blob = OpenedExportBlob {
            digest: format!("sha256:{:x}", Sha256::digest(b"blob")),
            file: fs::File::open(&blob_path).unwrap(),
            size_bytes: 4,
        };

        let mut plan = OpenedOllamaExportPlan::new(
            manifest,
            vec![blob],
            8,
            Path::new("registry.ollama.ai/library/example/latest"),
        )
        .unwrap();

        fs::remove_file(&manifest_path).unwrap();
        fs::remove_file(&blob_path).unwrap();

        assert_eq!(plan.total_bytes, 12);
        assert_eq!(plan.blobs.len(), 1);

        let mut manifest_bytes = Vec::new();
        plan.manifest.read_to_end(&mut manifest_bytes).unwrap();
        assert_eq!(manifest_bytes, b"manifest");

        let mut blob_bytes = Vec::new();
        plan.blobs[0].file.read_to_end(&mut blob_bytes).unwrap();
        assert_eq!(blob_bytes, b"blob");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn retained_blob_descriptor_survives_pathname_replacement() {
        use rustix::fs::{Mode, OFlags, ResolveFlags};

        let directory = tempfile::tempdir().unwrap();
        let blob_root = directory.path().join("blobs");
        fs::create_dir(&blob_root).unwrap();

        let original_bytes = b"original verified blob";
        let replacement_bytes = b"malicious replacement";
        let digest = format!("sha256:{:x}", Sha256::digest(original_bytes));
        let filename = blob_filename(&digest).unwrap();
        let blob_path = blob_root.join(&filename);

        fs::write(&blob_path, original_bytes).unwrap();

        let root_fd = rustix::fs::openat(
            rustix::fs::CWD,
            &blob_root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let blob_fd = rustix::fs::openat2(
            &root_fd,
            filename.as_str(),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
        )
        .unwrap();

        let mut opened = OpenedExportBlob {
            digest: digest.clone(),
            file: fs::File::from(blob_fd),
            size_bytes: original_bytes.len() as u64,
        };

        let displaced = blob_root.join("displaced");
        fs::rename(&blob_path, &displaced).unwrap();
        fs::write(&blob_path, replacement_bytes).unwrap();

        assert_eq!(fs::read(&blob_path).unwrap(), replacement_bytes);

        let mut output = Vec::new();
        let copied = copy_blob_with_sha256_verification(
            &mut opened.file,
            &mut output,
            &opened.digest,
            opened.size_bytes,
            |_| {},
        )
        .unwrap();

        assert_eq!(copied, original_bytes.len() as u64);
        assert_eq!(output, original_bytes);
    }

    #[test]
    fn plans_manifest_with_config_and_layers() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path();
        let manifests = store.join("manifests/registry.ollama.ai/library/example");
        let blobs = store.join("blobs");

        fs::create_dir_all(&manifests).unwrap();
        fs::create_dir_all(&blobs).unwrap();

        let digest = format!("sha256:{}", "a".repeat(64));
        fs::write(blobs.join(format!("sha256-{}", "a".repeat(64))), b"test").unwrap();

        let manifest = serde_json::json!({
            "schemaVersion": 2,
            "config": {"digest": digest, "size": 4},
            "layers": [{"digest": digest, "size": 4}]
        });

        let path = manifests.join("latest");
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();

        let plan = plan_ollama_export(store, &path).unwrap();

        assert_eq!(plan.blobs.len(), 1);
        assert_eq!(plan.blobs[0].size_bytes, 4);
        assert_eq!(plan.total_bytes, fs::metadata(&path).unwrap().len() + 4);
    }

    #[test]
    fn rejects_missing_blob() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path();
        let manifests = store.join("manifests/registry.ollama.ai/library/example");

        fs::create_dir_all(&manifests).unwrap();
        fs::create_dir_all(store.join("blobs")).unwrap();

        let manifest = serde_json::json!({
            "layers": [{
                "digest": format!("sha256:{}", "b".repeat(64)),
                "size": 4
            }]
        });

        let path = manifests.join("latest");
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();

        assert!(plan_ollama_export(store, &path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_blob() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let store = directory.path();
        let manifests = store.join("manifests/registry.ollama.ai/library/example");
        let blobs = store.join("blobs");

        fs::create_dir_all(&manifests).unwrap();
        fs::create_dir_all(&blobs).unwrap();

        let target = store.join("target");
        fs::write(&target, b"test").unwrap();

        let hash = "c".repeat(64);
        symlink(&target, blobs.join(format!("sha256-{hash}"))).unwrap();

        let path = manifests.join("latest");
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "layers": [{"digest": format!("sha256:{hash}"), "size": 4}]
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(plan_ollama_export(store, &path).is_err());
    }
}
