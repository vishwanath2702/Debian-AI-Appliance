//! DAIA command-line interface.

use std::env;
use std::process::ExitCode;

use engine::Engine;
use model::Capability;

fn main() -> ExitCode {
    let mut args = env::args();

    // Skip executable name.
    let _ = args.next();

    let Some(capability_name) = args.next() else {
        eprintln!("Usage:");
        eprintln!("    daia <capability>");
        eprintln!();
        eprintln!("Example:");
        eprintln!("    daia desktop");
        return ExitCode::FAILURE;
    };

    let engine = Engine::new();

    match engine.plan(&Capability::new(capability_name)) {
        Ok(plan) => {
            println!("Capability : {}", plan.capability);
            println!("Provider   : {}", plan.provider);
            println!();

            println!("Plan:");

            for (index, step) in plan.steps.iter().enumerate() {
                println!("  {}. {}", index + 1, step);
            }

            ExitCode::SUCCESS
        }

        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}
