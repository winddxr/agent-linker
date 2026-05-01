use crate::core::{
    db,
    error::{Error, Result},
    framework,
    linkable::{LinkableItem, LinkableItemType},
    manifest::{init_current_project, InitProjectReport},
    registry::{self, AddLinkableItem},
};

use std::path::PathBuf;

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
    match command {
        Command::Init => run_init(),
        Command::Db(args) => run_db(args),
        Command::Framework(args) => run_framework(args),
        Command::Skill(args) => run_linkable(args, LinkableItemType::Skill),
        Command::Resource(args) => run_linkable(args, LinkableItemType::Resource),
        other => Err(Error::not_implemented(format!(
            "command `{}` is not implemented yet",
            other.name()
        ))),
    }
}

fn run_init() -> Result<()> {
    let report = init_current_project()?;
    print_init_report(&report);
    Ok(())
}

fn print_init_report(report: &InitProjectReport) {
    println!(
        "Initialized Agent Linker project at {}",
        report.project_root.display()
    );
    println!("Manifest: {}", report.manifest_path.display());
    println!("Managed links: {}", report.link_outcomes.len());
}

fn run_linkable(args: Vec<String>, item_type: LinkableItemType) -> Result<()> {
    let resolution = db::resolve_database_path()?;

    match args.as_slice() {
        [subcommand, rest @ ..] if subcommand == "add" => {
            let request = parse_add_linkable(rest, item_type)?;
            let item = registry::add_item(&resolution, request)?;
            println!("Added {} `{}`", item_type, item.name);
            println!("Source: {}", item.source_path.display());
            println!(
                "Default link: {}",
                item.default_project_link_path()?.display()
            );
            Ok(())
        }
        [subcommand] if subcommand == "list" => {
            for item in registry::list_items(&resolution, item_type)? {
                println!(
                    "{}\t{}\t{}\t{}",
                    item.name,
                    item.link_name(),
                    item.source_kind,
                    item.source_path.display()
                );
            }
            Ok(())
        }
        [subcommand, identifier] if subcommand == "show" => {
            let item = registry::show_item(&resolution, item_type, identifier)?;
            print_linkable_item(&item)?;
            Ok(())
        }
        [subcommand, identifier, new_name] if subcommand == "rename" => {
            let item = registry::rename_item(&resolution, item_type, identifier, new_name)?;
            println!("Renamed {} to `{}`", item_type, item.name);
            Ok(())
        }
        [subcommand, identifier] if subcommand == "remove" => {
            let item = registry::remove_item(&resolution, item_type, identifier)?;
            println!("Removed {} `{}`", item_type, item.name);
            Ok(())
        }
        [subcommand, identifier] if subcommand == "refresh" => {
            let item = registry::refresh_item(&resolution, item_type, identifier)?;
            println!("Refreshed {} `{}`", item_type, item.name);
            println!("Source: {}", item.source_path.display());
            println!("Kind: {}", item.source_kind);
            Ok(())
        }
        [] => Err(Error::invalid_arguments(format!(
            "usage: aglink {} <add|list|show|rename|remove|refresh>",
            item_type.as_str()
        ))),
        _ => Err(Error::invalid_arguments(format!(
            "usage: aglink {} <add|list|show|rename|remove|refresh>",
            item_type.as_str()
        ))),
    }
}

fn parse_add_linkable(args: &[String], item_type: LinkableItemType) -> Result<AddLinkableItem> {
    let usage = match item_type {
        LinkableItemType::Skill => {
            "usage: aglink skill add <name> <source> [--alias <alias>] [--description <text>]"
        }
        LinkableItemType::Resource => {
            "usage: aglink resource add <name> <source> --target-dir <dir> [--alias <alias>] [--description <text>]"
        }
    };

    if args.len() < 2 {
        return Err(Error::invalid_arguments(usage));
    }

    let name = args[0].clone();
    let source_path = PathBuf::from(&args[1]);
    let mut alias = None;
    let mut description = None;
    let mut default_target_dir = None;
    let mut index = 2;

    while index < args.len() {
        match args[index].as_str() {
            "--alias" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(Error::invalid_arguments("--alias requires a value"));
                };
                alias = Some(value.clone());
            }
            "--description" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(Error::invalid_arguments("--description requires a value"));
                };
                description = Some(value.clone());
            }
            "--target-dir" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(Error::invalid_arguments("--target-dir requires a value"));
                };
                if item_type == LinkableItemType::Skill {
                    return Err(Error::invalid_arguments(
                        "skill add does not accept --target-dir",
                    ));
                }
                default_target_dir = Some(PathBuf::from(value));
            }
            other => {
                return Err(Error::invalid_arguments(format!(
                    "unknown argument `{other}`; {usage}"
                )));
            }
        }
        index += 1;
    }

    if item_type == LinkableItemType::Resource && default_target_dir.is_none() {
        return Err(Error::invalid_arguments(
            "resource add requires --target-dir <dir>",
        ));
    }

    Ok(AddLinkableItem {
        item_type,
        name,
        alias,
        source_path,
        default_target_dir,
        description,
    })
}

fn print_linkable_item(item: &LinkableItem) -> Result<()> {
    println!("Id: {}", item.id);
    println!("Type: {}", item.item_type);
    println!("Name: {}", item.name);
    if let Some(alias) = &item.alias {
        println!("Alias: {alias}");
    }
    println!("Link name: {}", item.link_name());
    println!("Source: {}", item.source_path.display());
    println!("Source kind: {}", item.source_kind);
    println!("Source type: {}", item.source_type.as_str());
    println!("Source ownership: {}", item.source_ownership.as_str());
    if let Some(target_dir) = &item.default_target_dir {
        println!("Default target dir: {}", target_dir.display());
    }
    println!(
        "Default link: {}",
        item.default_project_link_path()?.display()
    );
    if let Some(description) = &item.description {
        println!("Description: {description}");
    }
    Ok(())
}

fn run_db(args: Vec<String>) -> Result<()> {
    match args.as_slice() {
        [subcommand] if subcommand == "path" => {
            let resolution = db::resolve_database_path()?;
            println!("Path: {}", resolution.path.display());
            println!("Reason: {}", resolution.reason.as_str());
            Ok(())
        }
        [subcommand] if subcommand == "migrate" => {
            let report = db::migrate_default_database()?;
            println!("Database: {}", report.path.display());
            println!("Reason: {}", report.reason.as_str());
            println!(
                "Schema: {} -> {}",
                report.previous_version, report.current_version
            );
            Ok(())
        }
        [subcommand] if subcommand == "check" => {
            let report = db::check_default_database()?;
            println!("Database: {}", report.path.display());
            println!("Reason: {}", report.reason.as_str());
            println!("Exists: {}", yes_no(report.exists));
            println!("Writable: {}", yes_no(report.writable));
            match report.schema_version {
                Some(version) => println!(
                    "Schema: {version} / latest {}",
                    report.latest_schema_version
                ),
                None => println!("Schema: missing / latest {}", report.latest_schema_version),
            }
            if let Some(count) = report.framework_count {
                println!("Frameworks: {count}");
            }
            if let Some(count) = report.mapping_count {
                println!("Mappings: {count}");
            }
            println!(
                "Status: {}",
                if report.is_ok() {
                    "ok"
                } else {
                    "needs attention"
                }
            );
            Ok(())
        }
        [] => Err(Error::invalid_arguments(
            "usage: aglink db <path|migrate|check>",
        )),
        _ => Err(Error::invalid_arguments(
            "usage: aglink db <path|migrate|check>",
        )),
    }
}

fn run_framework(args: Vec<String>) -> Result<()> {
    let resolution = db::resolve_database_path()?;

    match args.as_slice() {
        [subcommand] if subcommand == "list" => {
            for framework in framework::list_frameworks(&resolution)? {
                println!(
                    "{}\t{}\t{}",
                    framework.name,
                    framework.display_name,
                    enabled_disabled(framework.enabled)
                );
            }
            Ok(())
        }
        [subcommand, framework_id] if subcommand == "show" => {
            let framework = framework::show_framework(&resolution, framework_id)?;
            println!("Id: {}", framework.id);
            println!("Name: {}", framework.name);
            println!("Display name: {}", framework.display_name);
            println!("Built in: {}", yes_no(framework.built_in));
            println!(
                "Enabled by default: {}",
                yes_no(framework.enabled_by_default)
            );
            println!("Enabled: {}", yes_no(framework.enabled));
            println!("Mappings: {}", framework.mappings.len());
            for mapping in framework.mappings {
                println!(
                    "  {}\t{} -> {}\t{}{}",
                    mapping.id,
                    mapping.source_path.display(),
                    mapping.link_path.display(),
                    mapping.link_kind,
                    if mapping.required { "\trequired" } else { "" }
                );
            }
            Ok(())
        }
        [subcommand, framework_id] if subcommand == "enable" => {
            framework::enable_framework(&resolution, framework_id)?;
            println!("Enabled framework `{framework_id}`");
            Ok(())
        }
        [subcommand, framework_id] if subcommand == "disable" => {
            framework::disable_framework(&resolution, framework_id)?;
            println!("Disabled framework `{framework_id}`");
            Ok(())
        }
        [scope, subcommand] if scope == "mapping" && subcommand == "list" => {
            for mapping in framework::list_all_mappings(&resolution)? {
                print_mapping(&mapping);
            }
            Ok(())
        }
        [scope, subcommand, framework_id] if scope == "mapping" && subcommand == "list" => {
            for mapping in framework::list_mappings_for_framework(&resolution, framework_id)? {
                print_mapping(&mapping);
            }
            Ok(())
        }
        [] => Err(Error::invalid_arguments(
            "usage: aglink framework <list|show|enable|disable|mapping list>",
        )),
        _ => Err(Error::invalid_arguments(
            "usage: aglink framework <list|show|enable|disable|mapping list>",
        )),
    }
}

fn print_mapping(mapping: &framework::StoredFrameworkMapping) {
    println!(
        "{}\t{}\t{} -> {}\t{}{}",
        mapping.framework_id,
        mapping.id,
        mapping.source_path.display(),
        mapping.link_path.display(),
        mapping.link_kind,
        if mapping.required { "\trequired" } else { "" }
    );
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
