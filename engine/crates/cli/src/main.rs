use std::env;
use std::process::ExitCode;

use engine::Engine;
use model::Capability;
use registry::Registry;

#[allow(clippy::missing_const_for_fn)]
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args();

    let _program = args.next();

    match args.next().as_deref() {
        Some("plan") => {
            let capability = args
                .next()
                .ok_or_else(|| String::from("missing capability"))?;

            if args.next().is_some() {
                return Err(String::from("too many arguments"));
            }

            let registry = Registry::built_in();
            let engine = Engine::new(registry);

            let plan = engine
                .plan(Capability::new(capability))
                .map_err(|error| error.to_string())?;

            println!("Execution plan");
            println!();
            println!("Capability:");
            println!("  {}", plan.capability);
            println!();
            println!("Provider:");
            println!("  {}", plan.provider);
            println!();
            println!("Steps:");

            for (index, step) in plan.steps.iter().enumerate() {
                println!("  {}. {}", index + 1, step);
            }

            println!();
            println!("{} step(s) planned.", plan.steps.len());

            Ok(())
        }

        Some("help") | None => {
            println!("Usage:");
            println!("  daia plan <capability>");
            println!("  daia help");

            Ok(())
        }

        Some(command) => Err(format!("unknown command '{command}'")),
    }
}
