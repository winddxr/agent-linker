use std::process::ExitCode;

fn main() -> ExitCode {
    agent_linker::cli::run_from_env()
}
