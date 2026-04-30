use std::process::ExitCode;

fn main() -> ExitCode {
    match agent_linker::cli::run_from_env() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("aglink: {error}");
            ExitCode::FAILURE
        }
    }
}
