use crate::core::{
    db,
    error::{Error, Result},
    framework,
    linkable::{LinkableItem, LinkableItemType},
    manifest::{init_current_project, InitProjectReport},
    project_links::{
        clean_current_project, doctor_current_project, link_current_project,
        link_group_current_project, status_current_project, unlink_current_project,
        unlink_group_current_project, CleanMode, CleanReport, DoctorReport, LinkGroupReport,
        LinkItemReport, LinkItemRequest, StatusReport, UnlinkReport,
    },
    registry::{self, AddLinkableItem, Group},
    symlink::{CreateSymlinkOutcome, LinkStatus, RemoveSymlinkOutcome},
};

use std::path::{Path, PathBuf};

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

pub fn run(command: Command) -> Result<()> {
    match command {
        Command::Init => run_init(),
        Command::Config(_) => Err(Error::not_implemented(
            "command `config` is not implemented yet",
        )),
        Command::Db(args) => run_db(args),
        Command::Framework(args) => run_framework(args),
        Command::Skill(args) => run_linkable(args, LinkableItemType::Skill),
        Command::Resource(args) => run_linkable(args, LinkableItemType::Resource),
        Command::Group(args) => run_group(args),
        Command::Link(args) => run_link(args),
        Command::Unlink(args) => run_unlink(args),
        Command::Status(args) => run_status(args),
        Command::Clean(args) => run_clean(args),
        Command::Doctor(args) => run_doctor(args),
    }
}

fn run_link(args: Vec<String>) -> Result<()> {
    if let [flag, group] = args.as_slice() {
        if flag == "--group" {
            let report = link_group_current_project(group)?;
            print_link_group_report(&report);
            return Ok(());
        }
    }

    let request = parse_link_request(&args)?;
    let report = link_current_project(request)?;
    print_link_report(&report);
    Ok(())
}

fn parse_link_request(args: &[String]) -> Result<LinkItemRequest> {
    let usage =
        "usage: aglink link <name> [--as <link-name>] [--target-dir <dir>] | aglink link --group <group>";
    let Some(identifier) = args.first() else {
        return Err(Error::invalid_arguments(usage));
    };
    if identifier.starts_with("--") {
        return Err(Error::invalid_arguments(usage));
    }

    let mut link_name_override = None;
    let mut target_dir_override = None;
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--as" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(Error::invalid_arguments("--as requires a value"));
                };
                link_name_override = Some(value.clone());
            }
            "--target-dir" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(Error::invalid_arguments("--target-dir requires a value"));
                };
                target_dir_override = Some(PathBuf::from(value));
            }
            other => {
                return Err(Error::invalid_arguments(format!(
                    "unknown argument `{other}`; {usage}"
                )));
            }
        }
        index += 1;
    }

    Ok(LinkItemRequest {
        identifier: identifier.clone(),
        link_name_override,
        target_dir_override,
    })
}

fn print_link_report(report: &LinkItemReport) {
    let action = match report.outcome {
        CreateSymlinkOutcome::Created => "Created",
        CreateSymlinkOutcome::AlreadyCorrect => "Already linked",
        CreateSymlinkOutcome::ReplacedWrongSymlink => "Updated",
    };
    println!(
        "{action} {} `{}`",
        report.item_type.as_str(),
        report.item_name
    );
    println!("Link: {}", report.link_path.display());
    println!("Source: {}", report.source_path.display());
    println!("Backend: {}", report.provider_backend);
    println!("Manifest: {}", report.manifest_path.display());
}

fn print_link_group_report(report: &LinkGroupReport) {
    println!(
        "Linked group `{}`: {} items",
        report.group_name,
        report.reports.len()
    );
    for item in &report.reports {
        let action = match item.outcome {
            CreateSymlinkOutcome::Created => "created",
            CreateSymlinkOutcome::AlreadyCorrect => "already-linked",
            CreateSymlinkOutcome::ReplacedWrongSymlink => "updated",
        };
        println!(
            "{}\t{}\t{}",
            action,
            item.item_name,
            item.link_path.display()
        );
    }
    println!("Manifest: {}", report.manifest_path.display());
}

fn run_unlink(args: Vec<String>) -> Result<()> {
    if let [flag, group] = args.as_slice() {
        if flag == "--group" {
            let report = unlink_group_current_project(group)?;
            print_unlink_report(&report);
            return Ok(());
        }
    }

    let identifier = parse_unlink_request(&args)?;
    let report = unlink_current_project(identifier)?;
    print_unlink_report(&report);
    Ok(())
}

fn parse_unlink_request(args: &[String]) -> Result<Option<String>> {
    match args {
        [flag] if flag == "--all" => Ok(None),
        [identifier] if !identifier.starts_with("--") => Ok(Some(identifier.clone())),
        [] => Err(Error::invalid_arguments(
            "usage: aglink unlink <name>|--group <group>|--all",
        )),
        _ => Err(Error::invalid_arguments(
            "usage: aglink unlink <name>|--group <group>|--all",
        )),
    }
}

fn print_unlink_report(report: &UnlinkReport) {
    println!("Removed managed links: {}", report.outcomes.len());
    for entry in &report.outcomes {
        let action = match entry.outcome {
            RemoveSymlinkOutcome::Removed => "removed",
            RemoveSymlinkOutcome::Missing => "missing",
        };
        println!(
            "{}\t{}\t{}",
            action,
            entry.record.item_name,
            entry.record.link_path.display()
        );
    }
    println!("Manifest: {}", report.manifest_path.display());
}

fn run_status(args: Vec<String>) -> Result<()> {
    let json = match args.as_slice() {
        [] => false,
        [flag] if flag == "--json" => true,
        _ => {
            return Err(Error::invalid_arguments("usage: aglink status [--json]"));
        }
    };

    let report = status_current_project()?;
    if json {
        print_status_json(&report);
    } else {
        print_status_report(&report);
    }
    Ok(())
}

fn run_clean(args: Vec<String>) -> Result<()> {
    let mode = match args.as_slice() {
        [] => CleanMode::Default,
        [flag] if flag == "--broken" => CleanMode::Broken,
        [flag] if flag == "--missing-source" => CleanMode::MissingSource,
        _ => {
            return Err(Error::invalid_arguments(
                "usage: aglink clean [--broken|--missing-source]",
            ));
        }
    };

    let report = clean_current_project(mode)?;
    print_clean_report(&report);
    Ok(())
}

fn print_clean_report(report: &CleanReport) {
    println!(
        "Cleaned managed links: {} removed, {} stale records dropped",
        report.removed.len(),
        report.dropped_missing.len()
    );
    for entry in &report.removed {
        let action = match entry.outcome {
            RemoveSymlinkOutcome::Removed => "removed",
            RemoveSymlinkOutcome::Missing => "missing",
        };
        println!(
            "{}\t{}\t{}",
            action,
            entry.record.item_name,
            entry.record.link_path.display()
        );
    }
    for record in &report.dropped_missing {
        println!(
            "dropped\t{}\t{}",
            record.item_name,
            record.link_path.display()
        );
    }
    println!("Manifest: {}", report.manifest_path.display());
}

fn run_doctor(args: Vec<String>) -> Result<()> {
    let verbose = match args.as_slice() {
        [] => false,
        [flag] if flag == "--verbose" => true,
        _ => return Err(Error::invalid_arguments("usage: aglink doctor [--verbose]")),
    };

    let report = doctor_current_project(verbose);
    print_doctor_report(&report);
    if report.ok {
        Ok(())
    } else {
        Err(Error::project("doctor reported issues"))
    }
}

fn print_doctor_report(report: &DoctorReport) {
    println!(
        "Doctor: {}",
        if report.ok { "ok" } else { "needs attention" }
    );
    for check in &report.checks {
        println!(
            "{}\t{}\t{}",
            if check.ok { "ok" } else { "fail" },
            check.name,
            check.summary
        );
        for detail in &check.details {
            println!("  {detail}");
        }
    }
}

fn print_status_report(report: &StatusReport) {
    println!("Project: {}", report.project_root.display());
    println!("Manifest: {}", report.manifest_path.display());
    println!("Managed links: {}", report.entries.len());
    for entry in &report.entries {
        println!(
            "{}\t{}\t{}\t{}",
            status_label(&entry.status),
            entry.record.item_name,
            entry.record.link_path.display(),
            entry.record.source_path.display()
        );
    }
}

fn print_status_json(report: &StatusReport) {
    println!("{{");
    println!(
        "  \"project_root\": \"{}\",",
        json_escape_path(&report.project_root)
    );
    println!(
        "  \"manifest_path\": \"{}\",",
        json_escape_path(&report.manifest_path)
    );
    println!("  \"links\": [");
    for (index, entry) in report.entries.iter().enumerate() {
        let comma = if index + 1 == report.entries.len() {
            ""
        } else {
            ","
        };
        println!("    {{");
        println!("      \"id\": \"{}\",", json_escape(&entry.record.id));
        println!(
            "      \"item_id\": \"{}\",",
            json_escape(&entry.record.item_id)
        );
        println!(
            "      \"item_name\": \"{}\",",
            json_escape(&entry.record.item_name)
        );
        println!(
            "      \"link_path\": \"{}\",",
            json_escape_path(&entry.record.link_path)
        );
        println!(
            "      \"source_path\": \"{}\",",
            json_escape_path(&entry.record.source_path)
        );
        println!("      \"link_kind\": \"{}\",", entry.record.link_kind);
        println!(
            "      \"provider_backend\": \"{}\",",
            entry.record.provider_backend
        );
        println!("      \"status\": \"{}\"", status_code(&entry.status));
        println!("    }}{comma}");
    }
    println!("  ]");
    println!("}}");
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

fn run_group(args: Vec<String>) -> Result<()> {
    let resolution = db::resolve_database_path()?;

    match args.as_slice() {
        [subcommand, name] if subcommand == "create" => {
            let group = registry::create_group(&resolution, name, None)?;
            println!("Created group `{}`", group.name);
            Ok(())
        }
        [subcommand] if subcommand == "list" => {
            for group in registry::list_groups(&resolution)? {
                println!("{}\t{} items", group.name, group.items.len());
            }
            Ok(())
        }
        [subcommand, name] if subcommand == "show" => {
            let group = registry::show_group(&resolution, name)?;
            print_group(&group);
            Ok(())
        }
        [subcommand, old, new] if subcommand == "rename" => {
            let group = registry::rename_group(&resolution, old, new)?;
            println!("Renamed group to `{}`", group.name);
            Ok(())
        }
        [subcommand, name] if subcommand == "delete" => {
            let group = registry::delete_group(&resolution, name)?;
            println!("Deleted group `{}`; sources were not modified", group.name);
            Ok(())
        }
        [subcommand, group, items @ ..] if subcommand == "add" => {
            let group = registry::add_group_items(&resolution, group, items)?;
            println!(
                "Updated group `{}`: {} items",
                group.name,
                group.items.len()
            );
            Ok(())
        }
        [subcommand, group, items @ ..] if subcommand == "remove" => {
            let group = registry::remove_group_items(&resolution, group, items)?;
            println!(
                "Updated group `{}`: {} items",
                group.name,
                group.items.len()
            );
            Ok(())
        }
        [subcommand, group] if subcommand == "link" => {
            let report = link_group_current_project(group)?;
            print_link_group_report(&report);
            Ok(())
        }
        [subcommand, group] if subcommand == "unlink" => {
            let report = unlink_group_current_project(group)?;
            print_unlink_report(&report);
            Ok(())
        }
        [] => Err(Error::invalid_arguments(
            "usage: aglink group <create|list|show|rename|delete|add|remove|link|unlink>",
        )),
        _ => Err(Error::invalid_arguments(
            "usage: aglink group <create|list|show|rename|delete|add|remove|link|unlink>",
        )),
    }
}

fn print_group(group: &Group) {
    println!("Id: {}", group.id);
    println!("Name: {}", group.name);
    if let Some(description) = &group.description {
        println!("Description: {description}");
    }
    println!("Items: {}", group.items.len());
    for item in &group.items {
        println!(
            "  {}\t{}\t{}",
            item.item_type,
            item.name,
            item.source_path.display()
        );
    }
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

fn json_escape_path(path: &Path) -> String {
    json_escape(&path.to_string_lossy())
}

fn json_escape(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                output.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => output.push(character),
        }
    }
    output
}
