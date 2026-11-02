//! Internal Unix IPC primitives for the DAIA Model Exporter.
//!
//! These primitives do not provide authorization or same-UID isolation.
//! A separately authorized broker and isolated worker are required.

#[cfg(target_os = "linux")]
pub fn create_export_ipc_pair() -> std::io::Result<(std::os::fd::OwnedFd, std::os::fd::OwnedFd)> {
    use rustix::net::{AddressFamily, SocketFlags, SocketType};

    rustix::net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .map_err(std::io::Error::from)
}

#[cfg(target_os = "linux")]
pub fn export_ipc_peer_credentials(
    socket: &std::os::fd::OwnedFd,
) -> std::io::Result<rustix::net::UCred> {
    rustix::net::sockopt::socket_peercred(socket).map_err(std::io::Error::from)
}

/// Protocol version for the internal exporter IPC envelope.
#[cfg(target_os = "linux")]
const EXPORT_IPC_VERSION: u8 = 1;

/// The only currently recognized operation is a non-privileged probe.
/// Export requests must not be accepted until authorization is implemented.
#[cfg(target_os = "linux")]
const EXPORT_IPC_PROBE: u8 = 0;

/// Validate an internal IPC envelope without performing an operation.
///
/// Version 1 probe packets consist of exactly two bytes and carry no
/// descriptors. This is not an authorization decision.
#[cfg(target_os = "linux")]
fn validate_export_ipc_message(message: &[u8], descriptor_count: usize) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};

    if message.len() != 2 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "invalid exporter IPC message length",
        ));
    }

    if message[0] != EXPORT_IPC_VERSION {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "unsupported exporter IPC version",
        ));
    }

    if message[1] != EXPORT_IPC_PROBE {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "unsupported exporter IPC operation",
        ));
    }

    if descriptor_count != 0 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "unexpected exporter IPC descriptors",
        ));
    }

    Ok(())
}

/// Receive exactly one non-privileged probe packet.
///
/// The empty ancillary buffer deliberately rejects descriptor-bearing
/// messages through MSG_CTRUNC. No exporter operation is performed.
#[cfg(target_os = "linux")]
fn receive_export_ipc_probe(socket: &std::os::fd::OwnedFd) -> std::io::Result<()> {
    use rustix::net::{RecvAncillaryBuffer, RecvFlags, ReturnFlags};
    use std::io::{Error, ErrorKind, IoSliceMut};

    let mut payload = [0_u8; 3];
    let mut iov = [IoSliceMut::new(&mut payload)];
    let mut control = RecvAncillaryBuffer::default();

    let received = rustix::net::recvmsg(socket, &mut iov, &mut control, RecvFlags::CMSG_CLOEXEC)
        .map_err(std::io::Error::from)?;

    if received
        .flags
        .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
    {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "truncated exporter IPC message or ancillary data",
        ));
    }

    if received.bytes != 2 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "invalid exporter IPC packet size",
        ));
    }

    validate_export_ipc_message(&payload[..received.bytes], 0)
}

/// Verify kernel-reported peer identity against an independently
/// established expected identity.
///
/// The expected values must originate from trusted process supervision,
/// not from the IPC message or an untrusted client.
#[cfg(target_os = "linux")]
fn verify_export_ipc_peer(
    socket: &std::os::fd::OwnedFd,
    expected: rustix::net::UCred,
) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};

    let actual = export_ipc_peer_credentials(socket)?;

    if actual != expected {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            "exporter IPC peer identity mismatch",
        ));
    }

    Ok(())
}

/// Validate a listening socket supplied by a trusted launcher.
///
/// This checks socket properties, not the launcher's identity or authorization.
#[cfg(target_os = "linux")]
pub fn validate_export_ipc_listener(socket: std::os::fd::BorrowedFd<'_>) -> std::io::Result<()> {
    use rustix::net::sockopt;
    use rustix::net::{AddressFamily, SocketType};
    use std::io::{Error, ErrorKind};

    if sockopt::socket_domain(socket).map_err(Error::from)? != AddressFamily::UNIX {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "export IPC socket is not AF_UNIX",
        ));
    }

    if sockopt::socket_type(socket).map_err(Error::from)? != SocketType::SEQPACKET {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "export IPC socket is not SOCK_SEQPACKET",
        ));
    }

    if !sockopt::socket_acceptconn(socket).map_err(Error::from)? {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "export IPC socket is not listening",
        ));
    }

    Ok(())
}

/// Validate the expected systemd socket-activation metadata.
///
/// These environment values are untrusted metadata, not authentication.
/// A trusted service launch and separate authorization remain necessary.
#[cfg(target_os = "linux")]
pub fn validate_export_ipc_activation_metadata(
    listen_pid: Option<&str>,
    listen_fds: Option<&str>,
    listen_fdnames: Option<&str>,
    expected_pid: u32,
) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};

    let invalid = || {
        Error::new(
            ErrorKind::PermissionDenied,
            "invalid exporter socket activation metadata",
        )
    };

    let pid = listen_pid
        .ok_or_else(invalid)?
        .parse::<u32>()
        .map_err(|_| invalid())?;

    if pid == 0 || pid != expected_pid {
        return Err(invalid());
    }

    if listen_fds != Some("1") {
        return Err(invalid());
    }

    if listen_fdnames != Some("daia-model-export.socket") {
        return Err(invalid());
    }

    Ok(())
}

/// Validate and duplicate an already-owned activation listener.
///
/// The trusted caller must establish descriptor ownership and provenance.
/// Activation environment metadata is not an authentication mechanism.
#[cfg(target_os = "linux")]
pub fn validate_owned_export_ipc_activation_listener(
    listener: &std::os::fd::OwnedFd,
    listen_pid: Option<&str>,
    listen_fds: Option<&str>,
    listen_fdnames: Option<&str>,
    expected_pid: u32,
) -> std::io::Result<std::os::fd::OwnedFd> {
    use std::os::fd::AsFd;

    validate_export_ipc_activation_metadata(listen_pid, listen_fds, listen_fdnames, expected_pid)?;

    validate_export_ipc_listener(listener.as_fd())?;

    let duplicate = listener.as_fd().try_clone_to_owned()?;

    validate_export_ipc_listener(duplicate.as_fd())?;

    Ok(duplicate)
}

/// Validate a trusted service-owned socket directory.
///
/// This validates the opened directory itself, not the trustworthiness
/// of the path used to reach it. Callers must retain a trusted directory
/// descriptor and prevent pathname substitution before socket binding.
#[cfg(target_os = "linux")]
pub fn validate_export_ipc_directory(
    directory: &std::fs::File,
    expected_uid: u32,
) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let metadata = directory.metadata()?;

    if !metadata.is_dir()
        || metadata.uid() != expected_uid
        || metadata.permissions().mode() & 0o7777 != 0o700
    {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            "exporter IPC directory is not private and service-owned",
        ));
    }

    Ok(())
}

/// Open and validate the private, root-owned exporter IPC directory.
///
/// Traverses from the filesystem root using retained directory descriptors.
/// This establishes directory ancestry, not peer authentication or export
/// authorization. The caller must retain the returned descriptor.
#[cfg(target_os = "linux")]
pub fn open_trusted_export_ipc_directory(
    path: &std::path::Path,
) -> std::io::Result<std::os::fd::OwnedFd> {
    use rustix::fs::{Mode, OFlags};
    use std::io::{Error, ErrorKind};
    use std::path::Component;

    if !path.is_absolute() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "export IPC directory path must be absolute",
        ));
    }

    let mut directory = rustix::fs::openat(
        rustix::fs::CWD,
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    let check_ancestor = |fd: &std::os::fd::OwnedFd| -> std::io::Result<()> {
        let metadata = rustix::fs::fstat(fd)?;

        if metadata.st_uid != 0 || metadata.st_gid != 0 || metadata.st_mode & 0o022 != 0 {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "untrusted exporter IPC directory ancestor",
            ));
        }

        Ok(())
    };

    check_ancestor(&directory)?;

    let mut components = path.components();

    if components.next() != Some(Component::RootDir) {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "invalid exporter IPC directory root",
        ));
    }

    let mut found_component = false;

    for component in components {
        let Component::Normal(name) = component else {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "invalid exporter IPC directory component",
            ));
        };

        found_component = true;

        let next = rustix::fs::openat(
            &directory,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?;

        check_ancestor(&next)?;
        directory = next;
    }

    if !found_component {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "export IPC directory cannot be filesystem root",
        ));
    }

    let metadata = rustix::fs::fstat(&directory)?;

    if metadata.st_mode & 0o7777 != 0o700 {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            "export IPC directory must have mode 0700",
        ));
    }

    Ok(directory)
}

/// Create a Unix SEQPACKET listener at a caller-controlled path.
///
/// The caller must establish an independently protected parent directory
/// before invoking this function. Socket creation alone is not authorization
/// and does not provide isolation from malicious same-UID processes.
#[cfg(target_os = "linux")]
pub fn create_export_ipc_listener(
    path: &std::path::Path,
    backlog: i32,
) -> std::io::Result<std::os::fd::OwnedFd> {
    use rustix::net::{AddressFamily, SocketFlags, SocketType};
    use std::io::{Error, ErrorKind};
    use std::os::fd::AsFd;

    if backlog <= 0 {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "exporter IPC backlog must be positive",
        ));
    }

    let socket = rustix::net::socket_with(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .map_err(std::io::Error::from)?;

    let address = rustix::net::SocketAddrUnix::new(path).map_err(std::io::Error::from)?;

    rustix::net::bind(&socket, &address).map_err(std::io::Error::from)?;

    rustix::net::listen(&socket, backlog).map_err(std::io::Error::from)?;

    validate_export_ipc_listener(socket.as_fd())?;

    Ok(socket)
}

/// Connect to an internal exporter IPC listener.
///
/// This establishes transport only. The caller must independently verify
/// endpoint provenance and authorization before sending any request.
#[cfg(target_os = "linux")]
pub fn connect_export_ipc(path: &std::path::Path) -> std::io::Result<std::os::fd::OwnedFd> {
    use rustix::net::{AddressFamily, SocketFlags, SocketType};

    let socket = rustix::net::socket_with(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .map_err(std::io::Error::from)?;

    let address = rustix::net::SocketAddrUnix::new(path).map_err(std::io::Error::from)?;

    rustix::net::connect(&socket, &address).map_err(std::io::Error::from)?;

    Ok(socket)
}

#[cfg(target_os = "linux")]
fn validate_export_descriptor_handoff(
    payload: &[u8],
    descriptor_count: usize,
    truncated: bool,
) -> std::io::Result<()> {
    if truncated || payload != [1, 1] || descriptor_count != 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid internal export descriptor handoff",
        ));
    }

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn receive_export_descriptor_handoff(
    socket: std::os::fd::BorrowedFd<'_>,
) -> std::io::Result<std::os::fd::OwnedFd> {
    use rustix::io::IoSliceMut;
    use rustix::net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, recvmsg};
    use std::mem::MaybeUninit;

    let mut payload = [0u8; 3];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
    let mut control = RecvAncillaryBuffer::new(&mut space);

    let message = recvmsg(
        socket,
        &mut [IoSliceMut::new(&mut payload)],
        &mut control,
        RecvFlags::CMSG_CLOEXEC,
    )?;

    let truncated = message
        .flags
        .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC);

    let bytes = message.bytes;
    let mut descriptors = Vec::new();
    let mut unexpected_ancillary = false;

    for ancillary in control.drain() {
        match ancillary {
            RecvAncillaryMessage::ScmRights(received) => {
                descriptors.extend(received);
            }
            _ => unexpected_ancillary = true,
        }
    }

    if unexpected_ancillary {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "unexpected export handoff ancillary message",
        ));
    }

    validate_export_descriptor_handoff(
        &payload[..bytes.min(payload.len())],
        descriptors.len(),
        truncated,
    )?;

    let descriptor = descriptors.pop().expect("validated descriptor count");
    let metadata = rustix::fs::fstat(&descriptor)?;

    if rustix::fs::FileType::from_raw_mode(metadata.st_mode) != rustix::fs::FileType::RegularFile {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "export handoff descriptor is not a regular file",
        ));
    }

    Ok(descriptor)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn descriptor_receiver_rejects_directory_descriptor() {
        use rustix::fd::AsFd;
        use rustix::io::IoSlice;
        use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};
        use std::mem::MaybeUninit;

        let directory = tempfile::tempdir().unwrap();
        let directory_file = std::fs::File::open(directory.path()).unwrap();
        let (sender, receiver) = create_export_ipc_pair().unwrap();

        let descriptors = [directory_file.as_fd()];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut control = SendAncillaryBuffer::new(&mut space);

        assert!(control.push(SendAncillaryMessage::ScmRights(&descriptors)));

        assert_eq!(
            sendmsg(
                &sender,
                &[IoSlice::new(&[1, 1])],
                &mut control,
                SendFlags::empty(),
            )
            .unwrap(),
            2
        );

        let error = receive_export_descriptor_handoff(receiver.as_fd()).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn descriptor_receiver_accepts_one_and_rejects_invalid_packets() {
        use rustix::fd::AsFd;
        use rustix::io::IoSlice;
        use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};
        use std::io::{Read, Seek, SeekFrom, Write};
        use std::mem::MaybeUninit;

        fn send_packet(
            sender: &std::os::fd::OwnedFd,
            payload: &[u8],
            files: &[std::os::fd::BorrowedFd<'_>],
        ) {
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
            let mut control = SendAncillaryBuffer::new(&mut space);

            if !files.is_empty() {
                assert!(control.push(SendAncillaryMessage::ScmRights(files)));
            }

            assert_eq!(
                sendmsg(
                    sender,
                    &[IoSlice::new(payload)],
                    &mut control,
                    SendFlags::empty(),
                )
                .unwrap(),
                payload.len()
            );
        }

        let mut file = tempfile::tempfile().unwrap();
        file.write_all(b"verified-handoff").unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let (sender, receiver) = create_export_ipc_pair().unwrap();
        send_packet(&sender, &[1, 1], &[file.as_fd()]);

        let owned = receive_export_descriptor_handoff(receiver.as_fd()).unwrap();

        let mut received = std::fs::File::from(owned);
        let mut contents = String::new();
        received.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "verified-handoff");

        let (sender, receiver) = create_export_ipc_pair().unwrap();
        send_packet(&sender, &[1, 1], &[]);
        assert!(receive_export_descriptor_handoff(receiver.as_fd()).is_err());

        let (sender, receiver) = create_export_ipc_pair().unwrap();
        send_packet(&sender, &[1, 1], &[file.as_fd(), file.as_fd()]);
        assert!(receive_export_descriptor_handoff(receiver.as_fd()).is_err());

        let (sender, receiver) = create_export_ipc_pair().unwrap();
        send_packet(&sender, &[1, 0], &[file.as_fd()]);
        assert!(receive_export_descriptor_handoff(receiver.as_fd()).is_err());

        let (sender, receiver) = create_export_ipc_pair().unwrap();
        send_packet(&sender, &[1, 1, 0, 0], &[file.as_fd()]);
        assert!(receive_export_descriptor_handoff(receiver.as_fd()).is_err());
    }

    #[test]
    fn descriptor_handoff_validation_is_fail_closed() {
        assert!(validate_export_descriptor_handoff(&[1, 1], 1, false).is_ok());

        for (payload, count, truncated) in [
            (&[1, 1][..], 0, false),
            (&[1, 1][..], 2, false),
            (&[1, 1][..], 1, true),
            (&[1, 0][..], 1, false),
            (&[2, 1][..], 1, false),
            (&[1][..], 1, false),
            (&[1, 1, 0][..], 1, false),
        ] {
            assert!(
                validate_export_descriptor_handoff(payload, count, truncated).is_err(),
                "unexpectedly accepted malformed handoff"
            );
        }
    }

    #[test]
    fn descriptor_handoff_transfers_one_owned_file() {
        use rustix::fd::AsFd;
        use rustix::io::{IoSlice, IoSliceMut};
        use rustix::net::{
            RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SendAncillaryBuffer,
            SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
        };
        use std::io::{Read, Seek, SeekFrom, Write};
        use std::mem::MaybeUninit;

        let (sender, receiver) = create_export_ipc_pair().unwrap();

        let mut file = tempfile::tempfile().unwrap();
        file.write_all(b"authorized-descriptor-test").unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let descriptors = [file.as_fd()];
        let mut send_space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut send_control = SendAncillaryBuffer::new(&mut send_space);

        assert!(send_control.push(SendAncillaryMessage::ScmRights(&descriptors)));

        assert_eq!(
            sendmsg(
                &sender,
                &[IoSlice::new(b"F")],
                &mut send_control,
                SendFlags::empty(),
            )
            .unwrap(),
            1
        );

        let mut payload = [0u8; 1];
        let mut receive_space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut receive_control = RecvAncillaryBuffer::new(&mut receive_space);

        let received = recvmsg(
            &receiver,
            &mut [IoSliceMut::new(&mut payload)],
            &mut receive_control,
            RecvFlags::CMSG_CLOEXEC,
        )
        .unwrap();

        assert_eq!(received.bytes, 1);
        assert_eq!(&payload, b"F");

        let mut received_files = Vec::new();

        for message in receive_control.drain() {
            match message {
                RecvAncillaryMessage::ScmRights(files) => {
                    received_files.extend(files);
                }
                _ => panic!("unexpected ancillary message"),
            }
        }

        assert_eq!(received_files.len(), 1);

        let owned = received_files.pop().unwrap();
        let mut received_file = std::fs::File::from(owned);
        let mut contents = String::new();

        received_file.read_to_string(&mut contents).unwrap();

        assert_eq!(contents, "authorized-descriptor-test");
    }

    #[test]
    fn export_ipc_owned_listener_accepts_valid_socket_and_duplicates_fd() {
        use rustix::net::{AddressFamily, SocketFlags, SocketType};
        use std::os::fd::{AsFd, AsRawFd};

        let directory = tempfile::tempdir().expect("temporary directory");
        let socket_path = directory.path().join("owned-export.sock");

        let socket = rustix::net::socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .expect("seqpacket socket");

        let address = rustix::net::SocketAddrUnix::new(&socket_path).expect("Unix socket address");

        rustix::net::bind(&socket, &address).expect("bind socket");
        rustix::net::listen(&socket, 4).expect("listen");

        let duplicate = validate_owned_export_ipc_activation_listener(
            &socket,
            Some("1234"),
            Some("1"),
            Some("daia-model-export.socket"),
            1234,
        )
        .expect("valid owned listener");

        assert_ne!(socket.as_raw_fd(), duplicate.as_raw_fd());

        let flags = rustix::io::fcntl_getfd(duplicate.as_fd()).expect("descriptor flags");

        assert!(flags.contains(rustix::io::FdFlags::CLOEXEC));

        drop(socket);

        validate_export_ipc_listener(duplicate.as_fd()).expect("duplicate remains valid");
    }

    #[test]
    fn export_ipc_owned_listener_rejects_invalid_metadata() {
        use rustix::net::{AddressFamily, SocketFlags, SocketType};

        let socket = rustix::net::socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .expect("seqpacket socket");

        assert!(
            validate_owned_export_ipc_activation_listener(
                &socket,
                Some("1234"),
                Some("2"),
                Some("daia-model-export.socket"),
                1234,
            )
            .is_err()
        );
    }

    #[test]
    fn export_ipc_owned_listener_rejects_non_listening_socket() {
        use rustix::net::{AddressFamily, SocketFlags, SocketType};

        let socket = rustix::net::socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .expect("seqpacket socket");

        assert!(
            validate_owned_export_ipc_activation_listener(
                &socket,
                Some("1234"),
                Some("1"),
                Some("daia-model-export.socket"),
                1234,
            )
            .is_err()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires isolated root-owned filesystem fixture"]
    fn trusted_export_ipc_directory_accepts_isolated_root_fixture() {
        use std::os::fd::AsFd;

        let path = std::path::Path::new("/run/daia-model-export");

        let descriptor = super::open_trusted_export_ipc_directory(path)
            .expect("trusted root-owned IPC directory");

        let metadata = rustix::fs::fstat(descriptor.as_fd()).expect("directory metadata");

        assert_eq!(metadata.st_uid, 0);
        assert_eq!(metadata.st_gid, 0);
        assert_eq!(metadata.st_mode & 0o7777, 0o700);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn trusted_export_ipc_directory_rejects_relative_path() {
        assert!(
            super::open_trusted_export_ipc_directory(std::path::Path::new("relative/export"))
                .is_err()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn trusted_export_ipc_directory_rejects_filesystem_root() {
        assert!(super::open_trusted_export_ipc_directory(std::path::Path::new("/")).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn trusted_export_ipc_directory_rejects_unprivileged_owner() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();

        assert!(super::open_trusted_export_ipc_directory(directory.path()).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn trusted_export_ipc_directory_rejects_symlinked_ancestor() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let actual = directory.path().join("actual");
        let linked = directory.path().join("linked");

        std::fs::create_dir(&actual).unwrap();
        symlink(&actual, &linked).unwrap();

        assert!(super::open_trusted_export_ipc_directory(&linked).is_err());
    }

    #[test]
    fn export_ipc_directory_accepts_private_owned_directory() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let directory = tempfile::tempdir().expect("temporary directory");

        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
            .expect("set private directory permissions");

        let opened = std::fs::File::open(directory.path()).expect("open directory");

        let uid = opened.metadata().expect("directory metadata").uid();

        validate_export_ipc_directory(&opened, uid).expect("private owned directory");
    }

    #[test]
    fn export_ipc_directory_rejects_wrong_uid() {
        use std::os::unix::fs::MetadataExt;

        let directory = tempfile::tempdir().expect("temporary directory");
        let opened = std::fs::File::open(directory.path()).expect("open directory");

        let uid = opened.metadata().expect("directory metadata").uid();

        assert!(validate_export_ipc_directory(&opened, uid.wrapping_add(1)).is_err());
    }

    #[test]
    fn export_ipc_directory_rejects_insecure_permissions() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let directory = tempfile::tempdir().expect("temporary directory");

        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o750))
            .expect("set directory permissions");

        let opened = std::fs::File::open(directory.path()).expect("open directory");

        let uid = opened.metadata().expect("directory metadata").uid();

        assert!(validate_export_ipc_directory(&opened, uid).is_err());
    }

    #[test]
    fn export_ipc_directory_rejects_regular_file() {
        use std::os::unix::fs::MetadataExt;

        let file = tempfile::tempfile().expect("temporary file");
        let uid = file.metadata().expect("file metadata").uid();

        assert!(validate_export_ipc_directory(&file, uid).is_err());
    }

    #[test]
    fn export_ipc_listener_helper_creates_valid_socket() {
        use std::os::fd::AsFd;

        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("export.sock");

        let listener = create_export_ipc_listener(&path, 4).expect("create listener");

        validate_export_ipc_listener(listener.as_fd()).expect("valid listener");

        assert!(path.exists());
    }

    #[test]
    fn export_ipc_connector_reaches_seqpacket_listener() {
        use std::os::fd::AsFd;

        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("export.sock");

        let listener = create_export_ipc_listener(&path, 4).expect("listener");
        let client = connect_export_ipc(&path).expect("connect");
        let server =
            rustix::net::accept_with(&listener, rustix::net::SocketFlags::CLOEXEC).expect("accept");

        for socket in [&client, &server] {
            assert_eq!(
                rustix::net::sockopt::socket_type(socket).unwrap(),
                rustix::net::SocketType::SEQPACKET,
            );

            let flags = rustix::io::fcntl_getfd(socket.as_fd()).unwrap();
            assert!(flags.contains(rustix::io::FdFlags::CLOEXEC));
        }
    }

    #[test]
    fn export_ipc_connector_rejects_missing_socket() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("missing.sock");

        assert!(connect_export_ipc(&path).is_err());
    }

    #[test]
    fn export_ipc_connector_rejects_regular_file_endpoint() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("not-a-socket");

        std::fs::write(&path, b"not a socket").unwrap();

        assert!(connect_export_ipc(&path).is_err());
    }

    #[test]
    fn export_ipc_listener_helper_rejects_invalid_backlog() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("export.sock");

        for backlog in [0, -1] {
            assert!(create_export_ipc_listener(&path, backlog).is_err());
        }

        assert!(!path.exists());
    }

    #[test]
    fn export_ipc_listener_helper_rejects_existing_path() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("export.sock");

        let original = create_export_ipc_listener(&path, 4).expect("first listener");

        assert!(create_export_ipc_listener(&path, 4).is_err());

        drop(original);
    }

    #[test]
    fn export_ipc_activation_accepts_exact_metadata() {
        validate_export_ipc_activation_metadata(
            Some("1234"),
            Some("1"),
            Some("daia-model-export.socket"),
            1234,
        )
        .expect("valid activation metadata");
    }

    #[test]
    fn export_ipc_activation_rejects_missing_metadata() {
        for (pid, fds, names) in [
            (None, Some("1"), Some("daia-model-export.socket")),
            (Some("1234"), None, Some("daia-model-export.socket")),
            (Some("1234"), Some("1"), None),
        ] {
            assert!(validate_export_ipc_activation_metadata(pid, fds, names, 1234).is_err());
        }
    }

    #[test]
    fn export_ipc_activation_rejects_wrong_pid() {
        for pid in ["0", "1235", "-1", "abc", "1234x", ""] {
            assert!(
                validate_export_ipc_activation_metadata(
                    Some(pid),
                    Some("1"),
                    Some("daia-model-export.socket"),
                    1234,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn export_ipc_activation_rejects_unexpected_descriptor_count() {
        for count in ["0", "2", "01", "-1", "abc", ""] {
            assert!(
                validate_export_ipc_activation_metadata(
                    Some("1234"),
                    Some(count),
                    Some("daia-model-export.socket"),
                    1234,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn export_ipc_activation_rejects_unexpected_descriptor_names() {
        for name in [
            "",
            "connection",
            "other.socket",
            "daia-model-export.socket:other.socket",
        ] {
            assert!(
                validate_export_ipc_activation_metadata(Some("1234"), Some("1"), Some(name), 1234,)
                    .is_err()
            );
        }
    }

    #[test]
    fn export_ipc_listener_accepts_bound_listening_seqpacket_socket() {
        use rustix::net::{AddressFamily, SocketFlags, SocketType};
        use std::os::fd::AsFd;

        let directory = tempfile::tempdir().expect("temporary directory");
        let socket_path = directory.path().join("export.sock");

        let socket = rustix::net::socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .expect("seqpacket socket");

        let address = rustix::net::SocketAddrUnix::new(&socket_path).expect("Unix socket address");

        rustix::net::bind(&socket, &address).expect("bind socket");
        rustix::net::listen(&socket, 4).expect("listen on socket");

        validate_export_ipc_listener(socket.as_fd()).expect("valid listening socket");
    }

    #[test]
    fn export_ipc_listener_rejects_regular_file() {
        use std::os::fd::AsFd;

        let file = tempfile::tempfile().expect("temporary file");
        assert!(validate_export_ipc_listener(file.as_fd()).is_err());
    }

    #[test]
    fn export_ipc_listener_rejects_stream_socket() {
        use rustix::net::{AddressFamily, SocketFlags, SocketType};
        use std::os::fd::AsFd;

        let socket = rustix::net::socket_with(
            AddressFamily::UNIX,
            SocketType::STREAM,
            SocketFlags::CLOEXEC,
            None,
        )
        .expect("stream socket");

        assert!(validate_export_ipc_listener(socket.as_fd()).is_err());
    }

    #[test]
    fn export_ipc_listener_rejects_non_listening_seqpacket_socket() {
        use std::os::fd::AsFd;

        let (socket, _peer) = create_export_ipc_pair().expect("socket pair");
        assert!(validate_export_ipc_listener(socket.as_fd()).is_err());
    }

    #[test]
    fn export_ipc_peer_policy_accepts_matching_kernel_identity() {
        let (first, _second) = create_export_ipc_pair().unwrap();
        let expected = export_ipc_peer_credentials(&first).unwrap();

        verify_export_ipc_peer(&first, expected).unwrap();
    }

    #[test]
    fn export_ipc_peer_policy_rejects_wrong_uid() {
        let (first, _second) = create_export_ipc_pair().unwrap();
        let mut expected = export_ipc_peer_credentials(&first).unwrap();

        let wrong_uid = expected.uid.as_raw().wrapping_add(1);
        expected.uid = rustix::process::Uid::from_raw(wrong_uid);

        assert_eq!(
            verify_export_ipc_peer(&first, expected).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn export_ipc_peer_policy_rejects_wrong_gid() {
        let (first, _second) = create_export_ipc_pair().unwrap();
        let mut expected = export_ipc_peer_credentials(&first).unwrap();

        let wrong_gid = expected.gid.as_raw().wrapping_add(1);
        expected.gid = rustix::process::Gid::from_raw(wrong_gid);

        assert_eq!(
            verify_export_ipc_peer(&first, expected).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn export_ipc_peer_policy_rejects_wrong_pid() {
        let (first, _second) = create_export_ipc_pair().unwrap();
        let mut expected = export_ipc_peer_credentials(&first).unwrap();

        let wrong_pid = expected.pid.as_raw_pid().wrapping_add(1);
        expected.pid = rustix::process::Pid::from_raw(wrong_pid).unwrap();

        assert_eq!(
            verify_export_ipc_peer(&first, expected).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn export_ipc_receiver_accepts_valid_probe() {
        use rustix::net::{SendAncillaryBuffer, SendFlags};
        use std::io::IoSlice;

        let (sender, receiver) = create_export_ipc_pair().unwrap();
        let mut control = SendAncillaryBuffer::default();

        let sent = rustix::net::sendmsg(
            &sender,
            &[IoSlice::new(&[1, 0])],
            &mut control,
            SendFlags::empty(),
        )
        .unwrap();

        assert_eq!(sent, 2);
        receive_export_ipc_probe(&receiver).unwrap();
    }

    #[test]
    fn export_ipc_receiver_rejects_oversized_packet() {
        use rustix::net::{SendAncillaryBuffer, SendFlags};
        use std::io::IoSlice;

        let (sender, receiver) = create_export_ipc_pair().unwrap();
        let mut control = SendAncillaryBuffer::default();

        rustix::net::sendmsg(
            &sender,
            &[IoSlice::new(&[1, 0, 0, 0])],
            &mut control,
            SendFlags::empty(),
        )
        .unwrap();

        assert!(receive_export_ipc_probe(&receiver).is_err());
    }

    #[test]
    fn export_ipc_receiver_rejects_descriptor_bearing_packet() {
        use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags};
        use std::io::IoSlice;
        use std::os::fd::AsFd;

        let (sender, receiver) = create_export_ipc_pair().unwrap();
        let descriptor = std::fs::File::open("/dev/null").unwrap();
        let descriptors = [descriptor.as_fd()];

        let mut space = [std::mem::MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];

        let mut control = SendAncillaryBuffer::new(&mut space);

        assert!(control.push(SendAncillaryMessage::ScmRights(&descriptors)));

        rustix::net::sendmsg(
            &sender,
            &[IoSlice::new(&[1, 0])],
            &mut control,
            SendFlags::empty(),
        )
        .unwrap();

        assert!(receive_export_ipc_probe(&receiver).is_err());
    }

    #[test]
    fn export_ipc_probe_contract_accepts_only_exact_envelope() {
        assert!(validate_export_ipc_message(&[1, 0], 0).is_ok());
    }

    #[test]
    fn export_ipc_probe_contract_rejects_malformed_messages() {
        for message in [
            &[][..],
            &[1][..],
            &[1, 0, 0][..],
            &[0, 0][..],
            &[2, 0][..],
            &[1, 1][..],
            &[1, 255][..],
        ] {
            assert!(validate_export_ipc_message(message, 0).is_err());
        }
    }

    #[test]
    fn export_ipc_probe_contract_rejects_unexpected_descriptors() {
        assert!(validate_export_ipc_message(&[1, 0], 1).is_err());
        assert!(validate_export_ipc_message(&[1, 0], 2).is_err());
    }

    use std::os::fd::AsRawFd;

    #[test]
    fn export_ipc_pair_creates_distinct_connected_descriptors() {
        let (first, second) = create_export_ipc_pair().unwrap();
        assert_ne!(first.as_raw_fd(), second.as_raw_fd());
    }

    #[test]
    fn export_ipc_pair_reports_current_process_credentials() {
        let (first, second) = create_export_ipc_pair().unwrap();

        let first_peer = export_ipc_peer_credentials(&first).unwrap();
        let second_peer = export_ipc_peer_credentials(&second).unwrap();

        let uid = rustix::process::getuid();
        let gid = rustix::process::getgid();

        assert_eq!(first_peer.uid, uid);
        assert_eq!(second_peer.uid, uid);
        assert_eq!(first_peer.gid, gid);
        assert_eq!(second_peer.gid, gid);
    }
}
