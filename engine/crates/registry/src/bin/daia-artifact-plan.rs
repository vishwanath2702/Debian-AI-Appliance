use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: daia-artifact-plan MANIFEST");
        process::exit(2);
    }

    let input = match fs::read_to_string(&args[1]) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("cannot read manifest: {error}");
            process::exit(1);
        }
    };

    let manifest = match registry::validate_artifact_manifest(&input) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("invalid artifact manifest: {error}");
            process::exit(1);
        }
    };

    for artifact in manifest.artifacts {
        println!(
            "{}\t{}\t{}\t{}",
            artifact.source,
            artifact.destination,
            artifact.sha256,
            artifact.mode
        );
    }
}
