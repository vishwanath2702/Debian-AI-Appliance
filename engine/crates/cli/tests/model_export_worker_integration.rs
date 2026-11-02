#![cfg(target_os = "linux")]

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn worker_rejects_real_socket_connection_without_exporting() {
    use rustix::net::{AddressFamily, SocketAddrUnix, SocketFlags, SocketType};

    let directory = tempfile::tempdir().unwrap();
    let socket_path = directory.path().join("worker.sock");

    let listener = rustix::net::socket_with(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();

    let address = SocketAddrUnix::new(&socket_path).unwrap();
    rustix::net::bind(&listener, &address).unwrap();
    rustix::net::listen(&listener, 1).unwrap();

    // Stdio takes ownership of the duplicated descriptor safely.
    let child_stdin = Stdio::from(rustix::io::dup(&listener).unwrap());

    let mut child = Command::new(env!("CARGO_BIN_EXE_daia-model-export-worker"))
        .stdin(child_stdin)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let client = rustix::net::socket_with(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();

    rustix::net::connect(&client, &address).unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);

    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(!status.success(), "worker unexpectedly authorized export");

            let output = child.wait_with_output().unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);

            assert!(
                stderr.contains("export IPC peer is not a privileged broker")
                    || stderr.contains("DAIA Model Export operations remain disabled")
                    || stderr.contains("missing systemd STATE_DIRECTORY"),
                "worker failed for an unexpected reason: {stderr}"
            );
            break;
        }

        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("worker did not terminate after connection");
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    drop(client);
}
