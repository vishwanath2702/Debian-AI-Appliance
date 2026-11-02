//! DAIA Model Export isolated worker.
//!
//! Fail-closed bootstrap only. No request handling, source access,
//! descriptor reception, or export publication is enabled.

use std::os::fd::AsFd;

fn main() {
    let stdin = std::io::stdin();

    if let Err(error) = engine::export_ipc::validate_export_ipc_listener(stdin.as_fd()) {
        eprintln!("DAIA Model Export worker: invalid activation listener: {error}");
        std::process::exit(1);
    }

    eprintln!("DAIA Model Export worker: listener validated; operations disabled");
    std::process::exit(1);
}
