use crate::core::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Init,
    Config(Vec<String>),
    Db(Vec<String>),
    Framework(Vec<String>),
    Skill(Vec<String>),
    Resource(Vec<String>),
    Group(Vec<String>),
    Link(Vec<String>),
    Unlink(Vec<String>),
    Status(Vec<String>),
    Clean(Vec<String>),
    Doctor(Vec<String>),
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Command::Init => "init",
            Command::Config(_) => "config",
            Command::Db(_) => "db",
            Command::Framework(_) => "framework",
            Command::Skill(_) => "skill",
            Command::Resource(_) => "resource",
            Command::Group(_) => "group",
            Command::Link(_) => "link",
            Command::Unlink(_) => "unlink",
            Command::Status(_) => "status",
            Command::Clean(_) => "clean",
            Command::Doctor(_) => "doctor",
        }
    }
}

pub fn run(command: Command) -> Result<()> {
    Err(Error::not_implemented(format!(
        "command `{}` is not implemented yet",
        command.name()
    )))
}
