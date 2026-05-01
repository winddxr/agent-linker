use std::path::{Path, PathBuf};

use crate::core::{
    db::{
        self, ConfigEntry, ConfigUnsetReport, DbBackupReport, DbCheckReport, DbPathResolution,
        MigrationReport,
    },
    error::{Error, Result},
    framework::{self, AddFrameworkMapping, StoredFramework, StoredFrameworkMapping},
    linkable::{LinkableItem, LinkableItemType},
    manifest::{init_current_project, InitProjectReport},
    project_links::{
        clean_current_project_with_options, doctor_current_project,
        link_current_project_with_options, link_group_current_project_with_options,
        status_current_project, unlink_current_project_with_options,
        unlink_group_current_project_with_options, CleanOptions, CleanReport, DoctorReport,
        LinkGroupReport, LinkItemReport, LinkItemRequest, LinkOptions, StatusReport, UnlinkOptions,
        UnlinkReport,
    },
    registry::{self, AddLinkableItem, Group},
    symlink::LinkKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Init,
    Config(ConfigCommand),
    Db(DbCommand),
    Framework(FrameworkCommand),
    Skill(LinkableCommand),
    Resource(LinkableCommand),
    Group(GroupCommand),
    Link(LinkCommand),
    Unlink(UnlinkCommand),
    Status(StatusCommand),
    Clean(CleanCommand),
    Doctor(DoctorCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigCommand {
    Path,
    List,
    Get { key: String },
    Set { key: String, value: String },
    Unset { key: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbCommand {
    Path,
    Migrate,
    Backup { path: Option<PathBuf> },
    Check,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameworkCommand {
    List,
    Show {
        name: String,
    },
    Enable {
        name: String,
    },
    Disable {
        name: String,
    },
    MappingList {
        name: Option<String>,
    },
    MappingAdd {
        name: String,
        source: PathBuf,
        link: PathBuf,
        kind: LinkKind,
    },
    MappingRemove {
        name: String,
        link: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkableCommand {
    Add {
        path: PathBuf,
        name: Option<String>,
        alias: Option<String>,
        target_dir: Option<PathBuf>,
    },
    List,
    Show {
        name: String,
    },
    Rename {
        old: String,
        new: String,
    },
    Remove {
        name: String,
    },
    Refresh {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupCommand {
    Create {
        name: String,
    },
    List,
    Show {
        name: String,
    },
    Rename {
        old: String,
        new: String,
    },
    Delete {
        name: String,
    },
    Add {
        group: String,
        items: Vec<String>,
    },
    Remove {
        group: String,
        items: Vec<String>,
    },
    Link {
        group: String,
        options: LinkOptions,
    },
    Unlink {
        group: String,
        options: UnlinkOptions,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkCommand {
    Item {
        request: LinkItemRequest,
        options: LinkOptions,
    },
    Group {
        group: String,
        options: LinkOptions,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnlinkTarget {
    Item(Option<String>),
    Group(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlinkCommand {
    pub target: UnlinkTarget,
    pub options: UnlinkOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusCommand {
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanCommand {
    pub options: CleanOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorCommand {
    pub verbose: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandReport {
    Init(InitProjectReport),
    ConfigPath(DbPathResolution),
    ConfigList(Vec<ConfigEntry>),
    ConfigGet {
        key: String,
        entry: Option<ConfigEntry>,
    },
    ConfigSet(ConfigEntry),
    ConfigUnset(ConfigUnsetReport),
    DbPath(DbPathResolution),
    DbMigrate(MigrationReport),
    DbBackup(DbBackupReport),
    DbCheck(DbCheckReport),
    FrameworkList(Vec<StoredFramework>),
    FrameworkShow(StoredFramework),
    FrameworkEnabled {
        name: String,
        enabled: bool,
    },
    FrameworkMappingList(Vec<StoredFrameworkMapping>),
    FrameworkMappingAdded(StoredFrameworkMapping),
    FrameworkMappingRemoved(StoredFrameworkMapping),
    LinkableAdded(LinkableItem),
    LinkableList {
        item_type: LinkableItemType,
        items: Vec<LinkableItem>,
    },
    LinkableShow(LinkableItem),
    LinkableRenamed(LinkableItem),
    LinkableRemoved(LinkableItem),
    LinkableRefreshed(LinkableItem),
    GroupCreated(Group),
    GroupList(Vec<Group>),
    GroupShow(Group),
    GroupRenamed(Group),
    GroupDeleted(Group),
    GroupUpdated(Group),
    LinkItem(LinkItemReport),
    LinkGroup(LinkGroupReport),
    Unlink(UnlinkReport),
    Status {
        report: StatusReport,
        json: bool,
    },
    Clean(CleanReport),
    Doctor(DoctorReport),
}

impl CommandReport {
    pub fn is_success(&self) -> bool {
        match self {
            CommandReport::Doctor(report) => report.ok,
            _ => true,
        }
    }
}

pub fn run(command: Command) -> Result<CommandReport> {
    match command {
        Command::Init => Ok(CommandReport::Init(init_current_project()?)),
        Command::Config(command) => run_config(command),
        Command::Db(command) => run_db(command),
        Command::Framework(command) => run_framework(command),
        Command::Skill(command) => run_linkable(command, LinkableItemType::Skill),
        Command::Resource(command) => run_linkable(command, LinkableItemType::Resource),
        Command::Group(command) => run_group(command),
        Command::Link(command) => run_link(command),
        Command::Unlink(command) => run_unlink(command),
        Command::Status(command) => {
            let report = status_current_project()?;
            Ok(CommandReport::Status {
                report,
                json: command.json,
            })
        }
        Command::Clean(command) => Ok(CommandReport::Clean(clean_current_project_with_options(
            command.options,
        )?)),
        Command::Doctor(command) => Ok(CommandReport::Doctor(doctor_current_project(
            command.verbose,
        ))),
    }
}

fn run_config(command: ConfigCommand) -> Result<CommandReport> {
    let resolution = db::resolve_database_path()?;
    match command {
        ConfigCommand::Path => Ok(CommandReport::ConfigPath(resolution)),
        ConfigCommand::List => Ok(CommandReport::ConfigList(db::list_config(&resolution)?)),
        ConfigCommand::Get { key } => {
            let entry = db::get_config(&resolution, &key)?;
            Ok(CommandReport::ConfigGet { key, entry })
        }
        ConfigCommand::Set { key, value } => Ok(CommandReport::ConfigSet(db::set_config(
            &resolution,
            &key,
            &value,
        )?)),
        ConfigCommand::Unset { key } => Ok(CommandReport::ConfigUnset(db::unset_config(
            &resolution,
            &key,
        )?)),
    }
}

fn run_db(command: DbCommand) -> Result<CommandReport> {
    match command {
        DbCommand::Path => Ok(CommandReport::DbPath(db::resolve_database_path()?)),
        DbCommand::Migrate => Ok(CommandReport::DbMigrate(db::migrate_default_database()?)),
        DbCommand::Backup { path } => Ok(CommandReport::DbBackup(db::backup_default_database(
            path.as_deref(),
        )?)),
        DbCommand::Check => Ok(CommandReport::DbCheck(db::check_default_database()?)),
    }
}

fn run_framework(command: FrameworkCommand) -> Result<CommandReport> {
    let resolution = db::resolve_database_path()?;
    match command {
        FrameworkCommand::List => Ok(CommandReport::FrameworkList(framework::list_frameworks(
            &resolution,
        )?)),
        FrameworkCommand::Show { name } => Ok(CommandReport::FrameworkShow(
            framework::show_framework(&resolution, &name)?,
        )),
        FrameworkCommand::Enable { name } => {
            framework::enable_framework(&resolution, &name)?;
            Ok(CommandReport::FrameworkEnabled {
                name,
                enabled: true,
            })
        }
        FrameworkCommand::Disable { name } => {
            framework::disable_framework(&resolution, &name)?;
            Ok(CommandReport::FrameworkEnabled {
                name,
                enabled: false,
            })
        }
        FrameworkCommand::MappingList { name } => {
            let mappings = if let Some(name) = name {
                framework::list_mappings_for_framework(&resolution, &name)?
            } else {
                framework::list_all_mappings(&resolution)?
            };
            Ok(CommandReport::FrameworkMappingList(mappings))
        }
        FrameworkCommand::MappingAdd {
            name,
            source,
            link,
            kind,
        } => Ok(CommandReport::FrameworkMappingAdded(
            framework::add_mapping(
                &resolution,
                AddFrameworkMapping {
                    framework: name,
                    source_path: source,
                    link_path: link,
                    link_kind: kind,
                },
            )?,
        )),
        FrameworkCommand::MappingRemove { name, link } => {
            Ok(CommandReport::FrameworkMappingRemoved(
                framework::remove_mapping(&resolution, &name, &link)?,
            ))
        }
    }
}

fn run_linkable(command: LinkableCommand, item_type: LinkableItemType) -> Result<CommandReport> {
    let resolution = db::resolve_database_path()?;
    match command {
        LinkableCommand::Add {
            path,
            name,
            alias,
            target_dir,
        } => {
            let name = match name {
                Some(name) => name,
                None => default_item_name(&path)?,
            };
            let item = registry::add_item(
                &resolution,
                AddLinkableItem {
                    item_type,
                    name,
                    alias,
                    source_path: path,
                    default_target_dir: target_dir,
                    description: None,
                },
            )?;
            Ok(CommandReport::LinkableAdded(item))
        }
        LinkableCommand::List => Ok(CommandReport::LinkableList {
            item_type,
            items: registry::list_items(&resolution, item_type)?,
        }),
        LinkableCommand::Show { name } => Ok(CommandReport::LinkableShow(registry::show_item(
            &resolution,
            item_type,
            &name,
        )?)),
        LinkableCommand::Rename { old, new } => Ok(CommandReport::LinkableRenamed(
            registry::rename_item(&resolution, item_type, &old, &new)?,
        )),
        LinkableCommand::Remove { name } => Ok(CommandReport::LinkableRemoved(
            registry::remove_item(&resolution, item_type, &name)?,
        )),
        LinkableCommand::Refresh { name } => Ok(CommandReport::LinkableRefreshed(
            registry::refresh_item(&resolution, item_type, &name)?,
        )),
    }
}

fn run_group(command: GroupCommand) -> Result<CommandReport> {
    let resolution = db::resolve_database_path()?;
    match command {
        GroupCommand::Create { name } => Ok(CommandReport::GroupCreated(registry::create_group(
            &resolution,
            &name,
            None,
        )?)),
        GroupCommand::List => Ok(CommandReport::GroupList(registry::list_groups(
            &resolution,
        )?)),
        GroupCommand::Show { name } => Ok(CommandReport::GroupShow(registry::show_group(
            &resolution,
            &name,
        )?)),
        GroupCommand::Rename { old, new } => Ok(CommandReport::GroupRenamed(
            registry::rename_group(&resolution, &old, &new)?,
        )),
        GroupCommand::Delete { name } => Ok(CommandReport::GroupDeleted(registry::delete_group(
            &resolution,
            &name,
        )?)),
        GroupCommand::Add { group, items } => Ok(CommandReport::GroupUpdated(
            registry::add_group_items(&resolution, &group, &items)?,
        )),
        GroupCommand::Remove { group, items } => Ok(CommandReport::GroupUpdated(
            registry::remove_group_items(&resolution, &group, &items)?,
        )),
        GroupCommand::Link { group, options } => Ok(CommandReport::LinkGroup(
            link_group_current_project_with_options(&group, options)?,
        )),
        GroupCommand::Unlink { group, options } => Ok(CommandReport::Unlink(
            unlink_group_current_project_with_options(&group, options)?,
        )),
    }
}

fn run_link(command: LinkCommand) -> Result<CommandReport> {
    match command {
        LinkCommand::Item { request, options } => Ok(CommandReport::LinkItem(
            link_current_project_with_options(request, options)?,
        )),
        LinkCommand::Group { group, options } => Ok(CommandReport::LinkGroup(
            link_group_current_project_with_options(&group, options)?,
        )),
    }
}

fn run_unlink(command: UnlinkCommand) -> Result<CommandReport> {
    match command.target {
        UnlinkTarget::Item(identifier) => Ok(CommandReport::Unlink(
            unlink_current_project_with_options(identifier, command.options)?,
        )),
        UnlinkTarget::Group(group) => Ok(CommandReport::Unlink(
            unlink_group_current_project_with_options(&group, command.options)?,
        )),
    }
}

fn default_item_name(path: &Path) -> Result<String> {
    path.file_stem()
        .or_else(|| path.file_name())
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| Error::invalid_arguments("source path must have a valid UTF-8 file name"))
}

#[cfg(test)]
mod tests {
    use super::{default_item_name, CommandReport};
    use std::path::PathBuf;

    #[test]
    fn default_item_name_uses_path_stem() {
        assert_eq!(
            default_item_name(&PathBuf::from("notes.md")).unwrap(),
            "notes"
        );
        assert_eq!(
            default_item_name(&PathBuf::from("skill-dir")).unwrap(),
            "skill-dir"
        );
    }

    #[test]
    fn doctor_report_controls_success_status() {
        let report = CommandReport::Doctor(crate::core::project_links::DoctorReport {
            checks: Vec::new(),
            ok: false,
        });
        assert!(!report.is_success());
    }

    #[test]
    fn command_layer_does_not_use_forbidden_low_level_apis() {
        let source = include_str!("mod.rs");
        for forbidden in [
            concat!("rusq", "lite"),
            concat!("std::", "fs"),
            concat!("std::", "os::"),
            concat!(".agents/", "links.toml"),
            concat!("load_", "manifest"),
            concat!("save_", "manifest"),
            concat!("ensure_", "symlink"),
            concat!("symlink_", "file"),
            concat!("symlink_", "dir"),
            concat!("read_", "link("),
        ] {
            assert!(
                !source.contains(forbidden),
                "commands layer must not contain `{forbidden}`"
            );
        }
    }
}
