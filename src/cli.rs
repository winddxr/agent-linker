use std::{
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use clap::{error::ErrorKind, Parser, Subcommand, ValueEnum};
use serde::Serialize;

use crate::{
    commands::{
        self, CleanCommand, Command, CommandReport, ConfigCommand, DbCommand, DoctorCommand,
        FrameworkCommand, GroupCommand, LinkCommand, LinkableCommand, StatusCommand, UnlinkCommand,
        UnlinkTarget,
    },
    core::{
        error::{Error, Result},
        linkable::LinkableItem,
        project_links::{
            CleanMode, CleanOptions, LinkItemReport, LinkItemRequest, LinkOptions, StatusReport,
            UnlinkOptions,
        },
        symlink::{CreateSymlinkOutcome, LinkKind, LinkStatus, RemoveSymlinkOutcome},
    },
};

#[derive(Debug, Parser)]
#[command(name = "aglink", about = "Agent Linker")]
struct Cli {
    #[arg(short, long, global = true)]
    quiet: bool,

    #[arg(long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    Init,
    Config {
        #[command(subcommand)]
        command: CliConfigCommand,
    },
    Db {
        #[command(subcommand)]
        command: CliDbCommand,
    },
    Framework {
        #[command(subcommand)]
        command: CliFrameworkCommand,
    },
    Skill {
        #[command(subcommand)]
        command: CliSkillCommand,
    },
    Resource {
        #[command(subcommand)]
        command: CliResourceCommand,
    },
    Group {
        #[command(subcommand)]
        command: CliGroupCommand,
    },
    Link(CliLinkArgs),
    Unlink(CliUnlinkArgs),
    Status(CliStatusArgs),
    Clean(CliCleanArgs),
    Doctor,
}

#[derive(Debug, Subcommand)]
enum CliConfigCommand {
    Path,
    List,
    Get { key: String },
    Set { key: String, value: String },
    Unset { key: String },
}

#[derive(Debug, Subcommand)]
enum CliDbCommand {
    Path,
    Migrate,
    Backup { path: Option<PathBuf> },
    Check,
}

#[derive(Debug, Subcommand)]
enum CliFrameworkCommand {
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
    Mapping {
        #[command(subcommand)]
        command: CliFrameworkMappingCommand,
    },
}

#[derive(Debug, Subcommand)]
enum CliFrameworkMappingCommand {
    List {
        name: Option<String>,
    },
    Add {
        name: String,
        source: PathBuf,
        link: PathBuf,
        #[arg(long, value_enum)]
        kind: CliLinkKind,
    },
    Remove {
        name: String,
        link: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliLinkKind {
    File,
    #[value(alias = "directory")]
    Dir,
}

impl From<CliLinkKind> for LinkKind {
    fn from(value: CliLinkKind) -> Self {
        match value {
            CliLinkKind::File => LinkKind::File,
            CliLinkKind::Dir => LinkKind::Directory,
        }
    }
}

#[derive(Debug, Subcommand)]
enum CliSkillCommand {
    Add {
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        alias: Option<String>,
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

#[derive(Debug, Subcommand)]
enum CliResourceCommand {
    Add {
        path: PathBuf,
        #[arg(long = "target-dir")]
        target_dir: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        alias: Option<String>,
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

#[derive(Debug, Subcommand)]
enum CliGroupCommand {
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
        #[arg(required = true)]
        items: Vec<String>,
    },
    Remove {
        group: String,
        #[arg(required = true)]
        items: Vec<String>,
    },
    Link {
        group: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        force: bool,
    },
    Unlink {
        group: String,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, clap::Args)]
struct CliLinkArgs {
    #[arg(conflicts_with = "group")]
    name: Option<String>,

    #[arg(long)]
    group: Option<String>,

    #[arg(long = "as")]
    as_name: Option<String>,

    #[arg(long = "target-dir")]
    target_dir: Option<PathBuf>,

    #[arg(long)]
    dry_run: bool,

    #[arg(long)]
    force: bool,
}

#[derive(Debug, clap::Args)]
struct CliUnlinkArgs {
    #[arg(conflicts_with_all = ["group", "all"])]
    name: Option<String>,

    #[arg(long, conflicts_with = "all")]
    group: Option<String>,

    #[arg(long)]
    all: bool,

    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, clap::Args)]
struct CliStatusArgs {
    #[arg(long)]
    json: bool,
}

#[derive(Debug, clap::Args)]
struct CliCleanArgs {
    #[arg(long)]
    broken: bool,

    #[arg(long = "missing-source")]
    missing_source: bool,

    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Clone, Copy)]
struct OutputOptions {
    quiet: bool,
    verbose: bool,
}

pub fn run_from_env() -> ExitCode {
    run_and_print(std::env::args_os())
}

pub fn run_and_print<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            let exit_code = parse_error_exit_code(&error);
            let _ = error.print();
            return exit_code;
        }
    };

    let options = OutputOptions {
        quiet: cli.quiet,
        verbose: cli.verbose,
    };
    let command = match command_from_cli(cli) {
        Ok(command) => command,
        Err(error) => {
            print_error(&error, options);
            return ExitCode::FAILURE;
        }
    };

    let report = match commands::run(command) {
        Ok(report) => report,
        Err(error) => {
            print_error(&error, options);
            return ExitCode::FAILURE;
        }
    };

    if let Err(error) = write_report(io::stdout(), &report, options) {
        eprintln!("aglink: {error}");
        return ExitCode::FAILURE;
    }

    if report.is_success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn parse_error_exit_code(error: &clap::Error) -> ExitCode {
    if matches!(
        error.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    ) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn command_from_cli(cli: Cli) -> Result<Command> {
    let verbose = cli.verbose;
    match cli.command {
        CliCommand::Init => Ok(Command::Init),
        CliCommand::Config { command } => Ok(Command::Config(match command {
            CliConfigCommand::Path => ConfigCommand::Path,
            CliConfigCommand::List => ConfigCommand::List,
            CliConfigCommand::Get { key } => ConfigCommand::Get { key },
            CliConfigCommand::Set { key, value } => ConfigCommand::Set { key, value },
            CliConfigCommand::Unset { key } => ConfigCommand::Unset { key },
        })),
        CliCommand::Db { command } => Ok(Command::Db(match command {
            CliDbCommand::Path => DbCommand::Path,
            CliDbCommand::Migrate => DbCommand::Migrate,
            CliDbCommand::Backup { path } => DbCommand::Backup { path },
            CliDbCommand::Check => DbCommand::Check,
        })),
        CliCommand::Framework { command } => Ok(Command::Framework(framework_command(command))),
        CliCommand::Skill { command } => Ok(Command::Skill(skill_command(command))),
        CliCommand::Resource { command } => Ok(Command::Resource(resource_command(command))),
        CliCommand::Group { command } => Ok(Command::Group(group_command(command))),
        CliCommand::Link(args) => Ok(Command::Link(link_command(args)?)),
        CliCommand::Unlink(args) => Ok(Command::Unlink(unlink_command(args)?)),
        CliCommand::Status(args) => Ok(Command::Status(StatusCommand { json: args.json })),
        CliCommand::Clean(args) => Ok(Command::Clean(clean_command(args)?)),
        CliCommand::Doctor => Ok(Command::Doctor(DoctorCommand { verbose })),
    }
}

fn framework_command(command: CliFrameworkCommand) -> FrameworkCommand {
    match command {
        CliFrameworkCommand::List => FrameworkCommand::List,
        CliFrameworkCommand::Show { name } => FrameworkCommand::Show { name },
        CliFrameworkCommand::Enable { name } => FrameworkCommand::Enable { name },
        CliFrameworkCommand::Disable { name } => FrameworkCommand::Disable { name },
        CliFrameworkCommand::Mapping { command } => match command {
            CliFrameworkMappingCommand::List { name } => FrameworkCommand::MappingList { name },
            CliFrameworkMappingCommand::Add {
                name,
                source,
                link,
                kind,
            } => FrameworkCommand::MappingAdd {
                name,
                source,
                link,
                kind: kind.into(),
            },
            CliFrameworkMappingCommand::Remove { name, link } => {
                FrameworkCommand::MappingRemove { name, link }
            }
        },
    }
}

fn skill_command(command: CliSkillCommand) -> LinkableCommand {
    match command {
        CliSkillCommand::Add { path, name, alias } => LinkableCommand::Add {
            path,
            name,
            alias,
            target_dir: None,
        },
        CliSkillCommand::List => LinkableCommand::List,
        CliSkillCommand::Show { name } => LinkableCommand::Show { name },
        CliSkillCommand::Rename { old, new } => LinkableCommand::Rename { old, new },
        CliSkillCommand::Remove { name } => LinkableCommand::Remove { name },
        CliSkillCommand::Refresh { name } => LinkableCommand::Refresh { name },
    }
}

fn resource_command(command: CliResourceCommand) -> LinkableCommand {
    match command {
        CliResourceCommand::Add {
            path,
            target_dir,
            name,
            alias,
        } => LinkableCommand::Add {
            path,
            name,
            alias,
            target_dir: Some(target_dir),
        },
        CliResourceCommand::List => LinkableCommand::List,
        CliResourceCommand::Show { name } => LinkableCommand::Show { name },
        CliResourceCommand::Rename { old, new } => LinkableCommand::Rename { old, new },
        CliResourceCommand::Remove { name } => LinkableCommand::Remove { name },
        CliResourceCommand::Refresh { name } => LinkableCommand::Refresh { name },
    }
}

fn group_command(command: CliGroupCommand) -> GroupCommand {
    match command {
        CliGroupCommand::Create { name } => GroupCommand::Create { name },
        CliGroupCommand::List => GroupCommand::List,
        CliGroupCommand::Show { name } => GroupCommand::Show { name },
        CliGroupCommand::Rename { old, new } => GroupCommand::Rename { old, new },
        CliGroupCommand::Delete { name } => GroupCommand::Delete { name },
        CliGroupCommand::Add { group, items } => GroupCommand::Add { group, items },
        CliGroupCommand::Remove { group, items } => GroupCommand::Remove { group, items },
        CliGroupCommand::Link {
            group,
            dry_run,
            force,
        } => GroupCommand::Link {
            group,
            options: LinkOptions { dry_run, force },
        },
        CliGroupCommand::Unlink { group, dry_run } => GroupCommand::Unlink {
            group,
            options: UnlinkOptions { dry_run },
        },
    }
}

fn link_command(args: CliLinkArgs) -> Result<LinkCommand> {
    let options = LinkOptions {
        dry_run: args.dry_run,
        force: args.force,
    };

    match (args.name, args.group) {
        (Some(_), Some(_)) => Err(Error::invalid_arguments(
            "usage: aglink link <name> [--as <link-name>] [--target-dir <dir>] [--dry-run] [--force] | aglink link --group <group> [--dry-run] [--force]",
        )),
        (None, Some(group)) => {
            if args.as_name.is_some() || args.target_dir.is_some() {
                return Err(Error::invalid_arguments(
                    "--as and --target-dir are only valid for item links",
                ));
            }
            Ok(LinkCommand::Group { group, options })
        }
        (Some(identifier), None) => Ok(LinkCommand::Item {
            request: LinkItemRequest {
                identifier,
                link_name_override: args.as_name,
                target_dir_override: args.target_dir,
            },
            options,
        }),
        (None, None) => Err(Error::invalid_arguments(
            "usage: aglink link <name> [--as <link-name>] [--target-dir <dir>] [--dry-run] [--force] | aglink link --group <group> [--dry-run] [--force]",
        )),
    }
}

fn unlink_command(args: CliUnlinkArgs) -> Result<UnlinkCommand> {
    let selected = args.name.is_some() as u8 + args.group.is_some() as u8 + args.all as u8;
    if selected != 1 {
        return Err(Error::invalid_arguments(
            "usage: aglink unlink <name>|--group <group>|--all [--dry-run]",
        ));
    }

    let target = if let Some(group) = args.group {
        UnlinkTarget::Group(group)
    } else if args.all {
        UnlinkTarget::Item(None)
    } else {
        UnlinkTarget::Item(args.name)
    };

    Ok(UnlinkCommand {
        target,
        options: UnlinkOptions {
            dry_run: args.dry_run,
        },
    })
}

fn clean_command(args: CliCleanArgs) -> Result<CleanCommand> {
    if args.broken && args.missing_source {
        return Err(Error::invalid_arguments(
            "usage: aglink clean [--broken|--missing-source] [--dry-run]",
        ));
    }

    let mode = if args.broken {
        CleanMode::Broken
    } else if args.missing_source {
        CleanMode::MissingSource
    } else {
        CleanMode::Default
    };

    Ok(CleanCommand {
        options: CleanOptions {
            mode,
            dry_run: args.dry_run,
        },
    })
}

fn write_report(
    mut writer: impl Write,
    report: &CommandReport,
    options: OutputOptions,
) -> io::Result<()> {
    if matches!(report, CommandReport::Status { json: true, .. }) {
        return write_status_json(writer, report);
    }

    if options.quiet {
        return Ok(());
    }

    for line in report_lines(report, options.verbose) {
        writeln!(writer, "{line}")?;
    }
    Ok(())
}

fn write_status_json(mut writer: impl Write, report: &CommandReport) -> io::Result<()> {
    let CommandReport::Status { report, .. } = report else {
        return Ok(());
    };
    serde_json::to_writer_pretty(&mut writer, &JsonStatusReport::from(report))
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
    writeln!(writer)?;
    Ok(())
}

fn report_lines(report: &CommandReport, verbose: bool) -> Vec<String> {
    match report {
        CommandReport::Init(report) => {
            let mut lines = vec![format!(
                "Initialized Agent Linker project at {}",
                report.project_root.display()
            )];
            if verbose {
                lines.push(format!("Manifest: {}", report.manifest_path.display()));
                lines.push(format!("Gitignore: {}", report.gitignore_path.display()));
                lines.push(format!("Managed links: {}", report.link_outcomes.len()));
            }
            lines
        }
        CommandReport::ConfigPath(resolution) | CommandReport::DbPath(resolution) => vec![
            format!("Path: {}", resolution.path.display()),
            format!("Reason: {}", resolution.reason.as_str()),
        ],
        CommandReport::ConfigList(entries) => {
            if entries.is_empty() {
                vec!["Config: empty".to_string()]
            } else {
                entries
                    .iter()
                    .map(|entry| format!("{}\t{}", entry.key, entry.value))
                    .collect()
            }
        }
        CommandReport::ConfigGet { key, entry } => match entry {
            Some(entry) => vec![format!("{}={}", entry.key, entry.value)],
            None => vec![format!("{key}: unset")],
        },
        CommandReport::ConfigSet(entry) => vec![format!("Set config `{}`", entry.key)],
        CommandReport::ConfigUnset(report) => vec![format!(
            "{} config `{}`",
            if report.removed {
                "Unset"
            } else {
                "Config already unset"
            },
            report.key
        )],
        CommandReport::DbMigrate(report) => vec![
            format!("Database: {}", report.path.display()),
            format!(
                "Schema: {} -> {}",
                report.previous_version, report.current_version
            ),
        ],
        CommandReport::DbBackup(report) => vec![
            format!("Backup: {}", report.backup_path.display()),
            format!("Source: {}", report.source_path.display()),
            format!("Bytes: {}", report.bytes),
        ],
        CommandReport::DbCheck(report) => {
            let mut lines = vec![
                format!("Database: {}", report.path.display()),
                format!("Exists: {}", yes_no(report.exists)),
                format!("Writable: {}", yes_no(report.writable)),
                format!(
                    "Schema: {} / latest {}",
                    report
                        .schema_version
                        .map_or_else(|| "missing".to_string(), |version| version.to_string()),
                    report.latest_schema_version
                ),
                format!(
                    "Status: {}",
                    if report.is_ok() {
                        "ok"
                    } else {
                        "needs attention"
                    }
                ),
            ];
            if verbose {
                lines.push(format!("Reason: {}", report.reason.as_str()));
                if let Some(count) = report.framework_count {
                    lines.push(format!("Frameworks: {count}"));
                }
                if let Some(count) = report.mapping_count {
                    lines.push(format!("Mappings: {count}"));
                }
            }
            lines
        }
        CommandReport::FrameworkList(frameworks) => frameworks
            .iter()
            .map(|framework| {
                format!(
                    "{}\t{}\t{}",
                    framework.name,
                    framework.display_name,
                    enabled_disabled(framework.enabled)
                )
            })
            .collect(),
        CommandReport::FrameworkShow(framework) => {
            let mut lines = vec![
                format!("Id: {}", framework.id),
                format!("Name: {}", framework.name),
                format!("Display name: {}", framework.display_name),
                format!("Enabled: {}", yes_no(framework.enabled)),
                format!("Mappings: {}", framework.mappings.len()),
            ];
            if verbose {
                lines.push(format!("Built in: {}", yes_no(framework.built_in)));
                lines.push(format!(
                    "Enabled by default: {}",
                    yes_no(framework.enabled_by_default)
                ));
            }
            for mapping in &framework.mappings {
                lines.push(format_mapping(mapping));
            }
            lines
        }
        CommandReport::FrameworkEnabled { name, enabled } => vec![format!(
            "{} framework `{name}`",
            if *enabled { "Enabled" } else { "Disabled" }
        )],
        CommandReport::FrameworkMappingList(mappings) => {
            mappings.iter().map(format_mapping).collect()
        }
        CommandReport::FrameworkMappingAdded(mapping) => {
            vec![format!("Added framework mapping {}", mapping.id)]
        }
        CommandReport::FrameworkMappingRemoved(mapping) => {
            vec![format!("Removed framework mapping {}", mapping.id)]
        }
        CommandReport::LinkableAdded(item) => {
            let mut lines = vec![format!("Added {} `{}`", item.item_type, item.name)];
            if verbose {
                lines.extend(linkable_details(item));
            }
            lines
        }
        CommandReport::LinkableList { item_type, items } => {
            if items.is_empty() {
                vec![format!("No {} entries", item_type.as_str())]
            } else {
                items
                    .iter()
                    .map(|item| {
                        format!(
                            "{}\t{}\t{}\t{}",
                            item.name,
                            item.link_name(),
                            item.source_kind,
                            item.source_path.display()
                        )
                    })
                    .collect()
            }
        }
        CommandReport::LinkableShow(item) => linkable_details(item),
        CommandReport::LinkableRenamed(item) => {
            vec![format!("Renamed {} to `{}`", item.item_type, item.name)]
        }
        CommandReport::LinkableRemoved(item) => {
            vec![format!("Removed {} `{}`", item.item_type, item.name)]
        }
        CommandReport::LinkableRefreshed(item) => {
            vec![format!("Refreshed {} `{}`", item.item_type, item.name)]
        }
        CommandReport::GroupCreated(group) => vec![format!("Created group `{}`", group.name)],
        CommandReport::GroupList(groups) => groups
            .iter()
            .map(|group| format!("{}\t{} items", group.name, group.items.len()))
            .collect(),
        CommandReport::GroupShow(group) => group_details(group),
        CommandReport::GroupRenamed(group) => vec![format!("Renamed group to `{}`", group.name)],
        CommandReport::GroupDeleted(group) => {
            vec![format!(
                "Deleted group `{}`; sources were not modified",
                group.name
            )]
        }
        CommandReport::GroupUpdated(group) => {
            vec![format!(
                "Updated group `{}`: {} items",
                group.name,
                group.items.len()
            )]
        }
        CommandReport::LinkItem(report) => link_item_lines(report, verbose),
        CommandReport::LinkGroup(report) => {
            let mut lines = vec![format!(
                "{} group `{}`: {} items",
                if report.dry_run {
                    "Would link"
                } else {
                    "Linked"
                },
                report.group_name,
                report.reports.len()
            )];
            for item in &report.reports {
                lines.push(format!(
                    "{}\t{}\t{}",
                    link_outcome_label(item.outcome, item.dry_run).to_lowercase(),
                    item.item_name,
                    item.link_path.display()
                ));
            }
            if verbose {
                lines.push(format!("Manifest: {}", report.manifest_path.display()));
            }
            lines
        }
        CommandReport::Unlink(report) => {
            let mut lines = vec![format!(
                "{} managed links: {}",
                if report.dry_run {
                    "Would remove"
                } else {
                    "Removed"
                },
                report.outcomes.len()
            )];
            for entry in &report.outcomes {
                lines.push(format!(
                    "{}\t{}\t{}",
                    remove_outcome_label(entry.outcome, report.dry_run),
                    entry.record.item_name,
                    entry.record.link_path.display()
                ));
            }
            if verbose {
                lines.push(format!("Manifest: {}", report.manifest_path.display()));
            }
            lines
        }
        CommandReport::Status { report, .. } => status_lines(report, verbose),
        CommandReport::Clean(report) => {
            let mut lines = vec![format!(
                "{} managed links: {} removed, {} stale records dropped",
                if report.dry_run {
                    "Would clean"
                } else {
                    "Cleaned"
                },
                report.removed.len(),
                report.dropped_missing.len()
            )];
            for entry in &report.removed {
                lines.push(format!(
                    "{}\t{}\t{}",
                    remove_outcome_label(entry.outcome, report.dry_run),
                    entry.record.item_name,
                    entry.record.link_path.display()
                ));
            }
            for record in &report.dropped_missing {
                lines.push(format!(
                    "{}\t{}\t{}",
                    if report.dry_run {
                        "would-drop"
                    } else {
                        "dropped"
                    },
                    record.item_name,
                    record.link_path.display()
                ));
            }
            if verbose {
                lines.push(format!("Manifest: {}", report.manifest_path.display()));
            }
            lines
        }
        CommandReport::Doctor(report) => {
            let mut lines = vec![format!(
                "Doctor: {}",
                if report.ok { "ok" } else { "needs attention" }
            )];
            for check in &report.checks {
                lines.push(format!(
                    "{}\t{}\t{}",
                    if check.ok { "ok" } else { "fail" },
                    check.name,
                    check.summary
                ));
                for detail in &check.details {
                    lines.push(format!("  {detail}"));
                }
            }
            lines
        }
    }
}

fn link_item_lines(report: &LinkItemReport, verbose: bool) -> Vec<String> {
    let mut lines = vec![format!(
        "{} {} `{}`",
        link_outcome_label(report.outcome, report.dry_run),
        report.item_type.as_str(),
        report.item_name
    )];
    if verbose {
        lines.push(format!("Link: {}", report.link_path.display()));
        lines.push(format!("Source: {}", report.source_path.display()));
        lines.push(format!("Backend: {}", report.provider_backend));
        lines.push(format!("Manifest: {}", report.manifest_path.display()));
        for dir in &report.created_dirs {
            lines.push(format!(
                "{} directory: {}",
                if report.dry_run {
                    "Would create"
                } else {
                    "Created"
                },
                dir.display()
            ));
        }
    }
    lines
}

fn status_lines(report: &StatusReport, verbose: bool) -> Vec<String> {
    let mut lines = vec![format!("Managed links: {}", report.entries.len())];
    if verbose {
        lines.insert(0, format!("Project: {}", report.project_root.display()));
        lines.insert(1, format!("Manifest: {}", report.manifest_path.display()));
    }
    for entry in &report.entries {
        lines.push(format!(
            "{}\t{}\t{}\t{}",
            status_label(&entry.status),
            entry.record.item_name,
            entry.record.link_path.display(),
            entry.record.source_path.display()
        ));
    }
    lines
}

fn linkable_details(item: &LinkableItem) -> Vec<String> {
    let mut lines = vec![
        format!("Id: {}", item.id),
        format!("Type: {}", item.item_type),
        format!("Name: {}", item.name),
        format!("Link name: {}", item.link_name()),
        format!("Source: {}", item.source_path.display()),
        format!("Source kind: {}", item.source_kind),
        format!("Source type: {}", item.source_type.as_str()),
        format!("Source ownership: {}", item.source_ownership.as_str()),
    ];
    if let Some(alias) = &item.alias {
        lines.push(format!("Alias: {alias}"));
    }
    if let Some(target_dir) = &item.default_target_dir {
        lines.push(format!("Default target dir: {}", target_dir.display()));
    }
    if let Ok(default_link) = item.default_project_link_path() {
        lines.push(format!("Default link: {}", default_link.display()));
    }
    if let Some(description) = &item.description {
        lines.push(format!("Description: {description}"));
    }
    lines
}

fn group_details(group: &crate::core::registry::Group) -> Vec<String> {
    let mut lines = vec![
        format!("Id: {}", group.id),
        format!("Name: {}", group.name),
        format!("Items: {}", group.items.len()),
    ];
    if let Some(description) = &group.description {
        lines.push(format!("Description: {description}"));
    }
    for item in &group.items {
        lines.push(format!(
            "  {}\t{}\t{}",
            item.item_type,
            item.name,
            item.source_path.display()
        ));
    }
    lines
}

fn format_mapping(mapping: &crate::core::framework::StoredFrameworkMapping) -> String {
    format!(
        "{}\t{}\t{} -> {}\t{}{}",
        mapping.framework_id,
        mapping.id,
        mapping.source_path.display(),
        mapping.link_path.display(),
        mapping.link_kind,
        if mapping.required { "\trequired" } else { "" }
    )
}

fn link_outcome_label(outcome: CreateSymlinkOutcome, dry_run: bool) -> &'static str {
    match (outcome, dry_run) {
        (CreateSymlinkOutcome::Created, false) => "Created",
        (CreateSymlinkOutcome::AlreadyCorrect, false) => "Already linked",
        (CreateSymlinkOutcome::ReplacedWrongSymlink, false) => "Updated",
        (CreateSymlinkOutcome::Created, true) => "Would create",
        (CreateSymlinkOutcome::AlreadyCorrect, true) => "Already linked",
        (CreateSymlinkOutcome::ReplacedWrongSymlink, true) => "Would update",
    }
}

fn remove_outcome_label(outcome: RemoveSymlinkOutcome, dry_run: bool) -> &'static str {
    match (outcome, dry_run) {
        (RemoveSymlinkOutcome::Removed, false) => "removed",
        (RemoveSymlinkOutcome::Missing, false) => "missing",
        (RemoveSymlinkOutcome::Removed, true) => "would-remove",
        (RemoveSymlinkOutcome::Missing, true) => "would-drop-missing",
    }
}

fn status_label(status: &LinkStatus) -> &'static str {
    match status {
        LinkStatus::Missing => "missing",
        LinkStatus::CorrectSymlink { .. } => "ok",
        LinkStatus::WrongSymlinkTarget { .. } => "wrong-target",
        LinkStatus::BrokenSymlink { .. } => "broken",
        LinkStatus::ExistingRealFile => "real-file",
        LinkStatus::ExistingRealDirectory => "real-directory",
        LinkStatus::UnsupportedFileType { .. } => "unsupported",
    }
}

fn status_code(status: &LinkStatus) -> &'static str {
    match status {
        LinkStatus::Missing => "missing",
        LinkStatus::CorrectSymlink { .. } => "correct_symlink",
        LinkStatus::WrongSymlinkTarget { .. } => "wrong_symlink_target",
        LinkStatus::BrokenSymlink { .. } => "broken_symlink",
        LinkStatus::ExistingRealFile => "existing_real_file",
        LinkStatus::ExistingRealDirectory => "existing_real_directory",
        LinkStatus::UnsupportedFileType { .. } => "unsupported_file_type",
    }
}

fn enabled_disabled(enabled: bool) -> &'static str {
    if enabled {
        "enabled"
    } else {
        "disabled"
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn print_error(error: &Error, _options: OutputOptions) {
    eprintln!("aglink: {error}");
}

#[derive(Debug, Serialize)]
struct JsonStatusReport {
    project_root: String,
    manifest_path: String,
    links: Vec<JsonStatusEntry>,
}

impl From<&StatusReport> for JsonStatusReport {
    fn from(report: &StatusReport) -> Self {
        Self {
            project_root: report.project_root.to_string_lossy().to_string(),
            manifest_path: report.manifest_path.to_string_lossy().to_string(),
            links: report
                .entries
                .iter()
                .map(|entry| JsonStatusEntry {
                    id: entry.record.id.clone(),
                    item_id: entry.record.item_id.clone(),
                    item_name: entry.record.item_name.clone(),
                    link_path: entry.record.link_path.to_string_lossy().to_string(),
                    source_path: entry.record.source_path.to_string_lossy().to_string(),
                    link_kind: entry.record.link_kind.to_string(),
                    provider_backend: entry.record.provider_backend.to_string(),
                    status: status_code(&entry.status).to_string(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct JsonStatusEntry {
    id: String,
    item_id: String,
    item_name: String,
    link_path: String,
    source_path: String,
    link_kind: String,
    provider_backend: String,
    status: String,
}

#[cfg(test)]
mod tests {
    use super::{
        clean_command, link_command, parse_error_exit_code, unlink_command, write_report, Cli,
        CliCleanArgs, CliLinkArgs, CliUnlinkArgs, OutputOptions,
    };
    use crate::{
        commands::{CommandReport, LinkCommand, UnlinkTarget},
        core::{
            manifest::LinkRecord,
            project_links::{CleanMode, StatusEntry, StatusReport},
            symlink::{LinkKind, LinkStatus, SymlinkBackend},
        },
    };
    use clap::Parser;
    use std::{path::PathBuf, process::ExitCode};

    #[test]
    fn help_exits_successfully() {
        let error = Cli::try_parse_from(["aglink", "--help"]).unwrap_err();
        assert_eq!(parse_error_exit_code(&error), ExitCode::SUCCESS);
    }

    #[test]
    fn link_requires_item_or_group() {
        let error = link_command(CliLinkArgs {
            name: None,
            group: None,
            as_name: None,
            target_dir: None,
            dry_run: false,
            force: false,
        })
        .unwrap_err();
        assert!(error.to_string().contains("usage"));
    }

    #[test]
    fn link_group_rejects_item_only_flags() {
        let error = link_command(CliLinkArgs {
            name: None,
            group: Some("daily".to_string()),
            as_name: Some("writer".to_string()),
            target_dir: None,
            dry_run: false,
            force: false,
        })
        .unwrap_err();
        assert!(error.to_string().contains("--as"));
    }

    #[test]
    fn link_item_preserves_dry_run_and_force() {
        let command = link_command(CliLinkArgs {
            name: Some("writer".to_string()),
            group: None,
            as_name: Some("helper".to_string()),
            target_dir: Some(PathBuf::from("docs")),
            dry_run: true,
            force: true,
        })
        .unwrap();

        assert!(matches!(
            command,
            LinkCommand::Item {
                request,
                options
            } if request.identifier == "writer"
                && request.link_name_override.as_deref() == Some("helper")
                && options.dry_run
                && options.force
        ));
    }

    #[test]
    fn unlink_requires_exactly_one_target() {
        let error = unlink_command(CliUnlinkArgs {
            name: Some("writer".to_string()),
            group: Some("daily".to_string()),
            all: false,
            dry_run: false,
        })
        .unwrap_err();
        assert!(error.to_string().contains("usage"));
    }

    #[test]
    fn unlink_all_uses_none_identifier() {
        let command = unlink_command(CliUnlinkArgs {
            name: None,
            group: None,
            all: true,
            dry_run: true,
        })
        .unwrap();

        assert!(matches!(command.target, UnlinkTarget::Item(None)));
        assert!(command.options.dry_run);
    }

    #[test]
    fn clean_modes_are_exclusive() {
        let error = clean_command(CliCleanArgs {
            broken: true,
            missing_source: true,
            dry_run: false,
        })
        .unwrap_err();
        assert!(error.to_string().contains("usage"));

        let command = clean_command(CliCleanArgs {
            broken: false,
            missing_source: true,
            dry_run: true,
        })
        .unwrap();
        assert_eq!(command.options.mode, CleanMode::MissingSource);
        assert!(command.options.dry_run);
    }

    #[test]
    fn status_json_uses_structured_escaping() {
        let report = CommandReport::Status {
            json: true,
            report: StatusReport {
                project_root: PathBuf::from("project\"root"),
                manifest_path: PathBuf::from(".agents").join("links.toml"),
                entries: vec![StatusEntry {
                    record: LinkRecord {
                        id: "id\"1".to_string(),
                        scope: "project".to_string(),
                        framework_name: "registry".to_string(),
                        item_id: "item\\1".to_string(),
                        item_name: "writer\nskill".to_string(),
                        source_path: PathBuf::from("source\"path"),
                        link_path: PathBuf::from(".agents").join("skills").join("writer"),
                        link_kind: LinkKind::Directory,
                        provider_backend: SymlinkBackend::Mock,
                        created_by_command: "link".to_string(),
                        created_at: "unix:1".to_string(),
                        updated_at: "unix:1".to_string(),
                    },
                    absolute_source_path: PathBuf::from("source"),
                    absolute_link_path: PathBuf::from("link"),
                    status: LinkStatus::Missing,
                }],
            },
        };

        let mut output = Vec::new();
        write_report(
            &mut output,
            &report,
            OutputOptions {
                quiet: true,
                verbose: false,
            },
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&output).unwrap();

        assert_eq!(value["project_root"], "project\"root");
        assert_eq!(value["links"][0]["id"], "id\"1");
        assert_eq!(value["links"][0]["item_name"], "writer\nskill");
        assert_eq!(value["links"][0]["status"], "missing");
    }
}
