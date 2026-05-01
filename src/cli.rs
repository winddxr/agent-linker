use crate::{
    commands::{self, Command},
    core::error::{Error, Result},
};

const HELP: &str = "\
Agent Linker

Usage: aglink <COMMAND>

Commands:
  init        Initialize Agent Linker project files
  config      Manage global configuration
  db          Manage the global database (path, migrate, check)
  framework   Manage Agent framework mappings (list, show, enable, disable, mapping list)
  skill       Manage registered skills (add, list, show, rename, remove, refresh)
  resource    Manage registered resources (add, list, show, rename, remove, refresh)
  group       Manage item groups
  link        Link a registered item into the current project
  unlink      Remove managed links from the current project
  status      Show current project link status
  clean       Clean managed links
  doctor      Run diagnostics

Options:
  -h, --help  Print help
";

enum CliRequest {
    Help,
    Command(Command),
}

pub fn run_from_env() -> Result<()> {
    run(std::env::args())
}

pub fn run<I, S>(args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    match parse_args(args.into_iter().map(Into::into))? {
        CliRequest::Help => {
            print!("{HELP}");
            Ok(())
        }
        CliRequest::Command(command) => commands::run(command),
    }
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<CliRequest> {
    let _program = args.next();
    let Some(command) = args.next() else {
        return Ok(CliRequest::Help);
    };

    match command.as_str() {
        "-h" | "--help" => Ok(CliRequest::Help),
        "init" => {
            let remaining: Vec<String> = args.collect();
            if remaining.is_empty() {
                Ok(CliRequest::Command(Command::Init))
            } else {
                Err(Error::invalid_arguments(
                    "command `init` does not accept arguments",
                ))
            }
        }
        "config" => Ok(CliRequest::Command(Command::Config(args.collect()))),
        "db" => Ok(CliRequest::Command(Command::Db(args.collect()))),
        "framework" => Ok(CliRequest::Command(Command::Framework(args.collect()))),
        "skill" => Ok(CliRequest::Command(Command::Skill(args.collect()))),
        "resource" => Ok(CliRequest::Command(Command::Resource(args.collect()))),
        "group" => Ok(CliRequest::Command(Command::Group(args.collect()))),
        "link" => Ok(CliRequest::Command(Command::Link(args.collect()))),
        "unlink" => Ok(CliRequest::Command(Command::Unlink(args.collect()))),
        "status" => Ok(CliRequest::Command(Command::Status(args.collect()))),
        "clean" => Ok(CliRequest::Command(Command::Clean(args.collect()))),
        "doctor" => Ok(CliRequest::Command(Command::Doctor(args.collect()))),
        other => Err(Error::invalid_arguments(format!(
            "unknown command `{other}`"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_args, CliRequest};
    use crate::commands::Command;

    #[test]
    fn no_args_requests_help() {
        let request = parse_args(["aglink"].into_iter().map(String::from)).unwrap();

        assert!(matches!(request, CliRequest::Help));
    }

    #[test]
    fn help_flag_requests_help() {
        let request = parse_args(["aglink", "--help"].into_iter().map(String::from)).unwrap();

        assert!(matches!(request, CliRequest::Help));
    }

    #[test]
    fn top_level_command_is_parsed() {
        let request = parse_args(["aglink", "db", "path"].into_iter().map(String::from)).unwrap();

        assert!(matches!(request, CliRequest::Command(Command::Db(args)) if args == ["path"]));
    }
}
