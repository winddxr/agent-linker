//! Project link/status/unlink orchestration.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::core::{
    db,
    db::DbPathResolution,
    error::{Error, Result},
    linkable::{
        validate_optional_alias, validate_project_relative_target_dir, validate_resource_source,
        validate_skill_source, LinkableItem, LinkableItemType,
    },
    manifest::{load_manifest, manifest_path, save_manifest, LinkRecord},
    registry,
    symlink::{
        default_provider, ensure_symlink, CreateSymlinkOptions, CreateSymlinkOutcome, LinkStatus,
        RemoveSymlinkOutcome, SymlinkBackend, SymlinkError, SymlinkErrorKind, SymlinkProvider,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkItemRequest {
    pub identifier: String,
    pub link_name_override: Option<String>,
    pub target_dir_override: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkItemReport {
    pub project_root: PathBuf,
    pub manifest_path: PathBuf,
    pub item_name: String,
    pub item_type: LinkableItemType,
    pub source_path: PathBuf,
    pub link_path: PathBuf,
    pub outcome: CreateSymlinkOutcome,
    pub provider_backend: SymlinkBackend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusReport {
    pub project_root: PathBuf,
    pub manifest_path: PathBuf,
    pub entries: Vec<StatusEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    pub record: LinkRecord,
    pub absolute_source_path: PathBuf,
    pub absolute_link_path: PathBuf,
    pub status: LinkStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlinkReport {
    pub project_root: PathBuf,
    pub manifest_path: PathBuf,
    pub outcomes: Vec<UnlinkEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlinkEntry {
    pub record: LinkRecord,
    pub absolute_link_path: PathBuf,
    pub outcome: RemoveSymlinkOutcome,
}

pub fn link_current_project(request: LinkItemRequest) -> Result<LinkItemReport> {
    let project_root = std::env::current_dir()?;
    let mut provider = default_provider();
    let resolution = db::resolve_database_path()?;
    link_project_with_provider_and_db(&project_root, provider.as_mut(), &resolution, request)
}

pub fn link_project_with_provider(
    project_root: &Path,
    provider: &mut dyn SymlinkProvider,
    request: LinkItemRequest,
) -> Result<LinkItemReport> {
    let resolution = db::resolve_database_path()?;
    link_project_with_provider_and_db(project_root, provider, &resolution, request)
}

pub fn link_project_with_provider_and_db(
    project_root: &Path,
    provider: &mut dyn SymlinkProvider,
    resolution: &DbPathResolution,
    request: LinkItemRequest,
) -> Result<LinkItemReport> {
    validate_project_root(project_root)?;
    let item = find_registry_item(resolution, &request.identifier)?;
    validate_registered_source(&item)?;

    let link_path = project_link_path_for_item(
        &item,
        request.link_name_override.as_deref(),
        request.target_dir_override.as_deref(),
    )?;
    ensure_link_parent(project_root, &link_path)?;

    let source_path = item.source_path.clone();
    let absolute_link_path = project_root.join(&link_path);
    let mut manifest = load_manifest(project_root)?;
    let record_id = manifest_record_id(&item, &link_path);

    if let Some(existing) = manifest
        .links
        .iter()
        .find(|link| link.link_path == link_path && link.id != record_id)
    {
        return Err(Error::manifest(format!(
            "project link path `{}` is already managed by `{}`",
            link_path.display(),
            existing.item_name
        )));
    }

    let outcome = ensure_symlink(
        provider,
        &source_path,
        &absolute_link_path,
        item.source_kind,
        CreateSymlinkOptions::new(),
    )?;

    let record = link_record_from_item(&item, &link_path, provider.backend(), &record_id);
    manifest.upsert(record);
    save_manifest(project_root, &manifest)?;

    Ok(LinkItemReport {
        project_root: project_root.to_path_buf(),
        manifest_path: manifest_path(project_root),
        item_name: item.name,
        item_type: item.item_type,
        source_path,
        link_path,
        outcome,
        provider_backend: provider.backend(),
    })
}

pub fn status_current_project() -> Result<StatusReport> {
    let project_root = std::env::current_dir()?;
    let provider = default_provider();
    status_project_with_provider(&project_root, provider.as_ref())
}

pub fn status_project_with_provider(
    project_root: &Path,
    provider: &dyn SymlinkProvider,
) -> Result<StatusReport> {
    validate_project_root(project_root)?;
    let manifest = load_manifest(project_root)?;
    let mut entries = Vec::new();

    for record in manifest.links {
        let absolute_source_path = project_path(project_root, &record.source_path);
        let absolute_link_path = project_root.join(&record.link_path);
        let status =
            provider.link_status(&absolute_source_path, &absolute_link_path, record.link_kind)?;
        entries.push(StatusEntry {
            record,
            absolute_source_path,
            absolute_link_path,
            status,
        });
    }

    Ok(StatusReport {
        project_root: project_root.to_path_buf(),
        manifest_path: manifest_path(project_root),
        entries,
    })
}

pub fn unlink_current_project(identifier: Option<String>) -> Result<UnlinkReport> {
    let project_root = std::env::current_dir()?;
    let mut provider = default_provider();
    unlink_project_with_provider(&project_root, provider.as_mut(), identifier)
}

pub fn unlink_project_with_provider(
    project_root: &Path,
    provider: &mut dyn SymlinkProvider,
    identifier: Option<String>,
) -> Result<UnlinkReport> {
    validate_project_root(project_root)?;
    let mut manifest = load_manifest(project_root)?;
    let selected_indexes = select_unlink_indexes(&manifest.links, identifier.as_deref())?;
    preflight_unlink(project_root, provider, &manifest.links, &selected_indexes)?;
    let mut outcomes = Vec::new();
    let mut remaining = Vec::new();

    for (index, record) in manifest.links.into_iter().enumerate() {
        if selected_indexes.contains(&index) {
            let absolute_link_path = project_root.join(&record.link_path);
            let outcome = provider.remove_symlink(&absolute_link_path)?;
            outcomes.push(UnlinkEntry {
                record,
                absolute_link_path,
                outcome,
            });
        } else {
            remaining.push(record);
        }
    }

    manifest.links = remaining;
    save_manifest(project_root, &manifest)?;

    Ok(UnlinkReport {
        project_root: project_root.to_path_buf(),
        manifest_path: manifest_path(project_root),
        outcomes,
    })
}

fn preflight_unlink(
    project_root: &Path,
    provider: &dyn SymlinkProvider,
    records: &[LinkRecord],
    selected_indexes: &[usize],
) -> Result<()> {
    for index in selected_indexes {
        let record = &records[*index];
        let absolute_source_path = project_path(project_root, &record.source_path);
        let absolute_link_path = project_root.join(&record.link_path);
        match provider.link_status(&absolute_source_path, &absolute_link_path, record.link_kind)? {
            LinkStatus::ExistingRealFile => {
                return Err(SymlinkError::new(
                    SymlinkErrorKind::ExistingRealFile,
                    provider.backend(),
                )
                .with_source(absolute_source_path)
                .with_link(absolute_link_path)
                .into());
            }
            LinkStatus::ExistingRealDirectory => {
                return Err(SymlinkError::new(
                    SymlinkErrorKind::ExistingRealDirectory,
                    provider.backend(),
                )
                .with_source(absolute_source_path)
                .with_link(absolute_link_path)
                .into());
            }
            LinkStatus::UnsupportedFileType { path } => {
                return Err(SymlinkError::new(
                    SymlinkErrorKind::UnsupportedLinkKind,
                    provider.backend(),
                )
                .with_source(absolute_source_path)
                .with_link(absolute_link_path)
                .with_detail(format!("unsupported file type at {}", path.display()))
                .into());
            }
            LinkStatus::Missing
            | LinkStatus::CorrectSymlink { .. }
            | LinkStatus::WrongSymlinkTarget { .. }
            | LinkStatus::BrokenSymlink { .. } => {}
        }
    }

    Ok(())
}

fn find_registry_item(resolution: &DbPathResolution, identifier: &str) -> Result<LinkableItem> {
    let mut matches = Vec::new();

    for item_type in [LinkableItemType::Skill, LinkableItemType::Resource] {
        for item in registry::list_items(resolution, item_type)? {
            if item.id == identifier
                || item.name == identifier
                || item.alias.as_deref() == Some(identifier)
            {
                matches.push(item);
            }
        }
    }

    match matches.len() {
        0 => Err(Error::database(format!(
            "unknown linkable item `{identifier}`"
        ))),
        1 => Ok(matches.remove(0)),
        _ => Err(Error::invalid_arguments(format!(
            "linkable item `{identifier}` is ambiguous; use a registry id"
        ))),
    }
}

fn validate_registered_source(item: &LinkableItem) -> Result<()> {
    let source = match item.item_type {
        LinkableItemType::Skill => validate_skill_source(&item.source_path)?,
        LinkableItemType::Resource => validate_resource_source(&item.source_path)?,
    };

    if source.source_kind != item.source_kind {
        return Err(Error::invalid_arguments(format!(
            "{} `{}` source kind changed from {} to {}",
            item.item_type, item.name, item.source_kind, source.source_kind
        )));
    }

    Ok(())
}

fn project_link_path_for_item(
    item: &LinkableItem,
    link_name_override: Option<&str>,
    target_dir_override: Option<&Path>,
) -> Result<PathBuf> {
    validate_optional_alias(link_name_override)?;

    let link_name = link_name_override
        .map(str::to_string)
        .unwrap_or_else(|| item.link_name());

    match item.item_type {
        LinkableItemType::Skill => {
            if target_dir_override.is_some() {
                return Err(Error::invalid_arguments(
                    "aglink link --target-dir is only valid for resources",
                ));
            }
            Ok(PathBuf::from(".agents").join("skills").join(link_name))
        }
        LinkableItemType::Resource => {
            let target_dir = match target_dir_override {
                Some(target_dir) => {
                    validate_project_relative_target_dir(target_dir)?;
                    target_dir.to_path_buf()
                }
                None => item.default_target_dir.clone().ok_or_else(|| {
                    Error::invalid_arguments(format!(
                        "resource `{}` does not have a default target directory; pass --target-dir <dir>",
                        item.name
                    ))
                })?,
            };
            Ok(target_dir.join(link_name))
        }
    }
}

fn ensure_link_parent(project_root: &Path, relative_link_path: &Path) -> Result<()> {
    if relative_link_path.is_absolute()
        || relative_link_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(Error::invalid_arguments(format!(
            "link path must stay inside the project: {}",
            relative_link_path.display()
        )));
    }

    let Some(parent) = relative_link_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    else {
        return Ok(());
    };

    let mut current = project_root.to_path_buf();
    for component in parent.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::project(format!(
                    "link parent must be a real directory, not a symlink: {}",
                    current.display()
                )));
            }
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                return Err(Error::project(format!(
                    "link parent path exists but is not a directory: {}",
                    current.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&current)?,
            Err(error) => return Err(error.into()),
        }
    }

    Ok(())
}

fn link_record_from_item(
    item: &LinkableItem,
    link_path: &Path,
    backend: SymlinkBackend,
    record_id: &str,
) -> LinkRecord {
    let now = timestamp();
    LinkRecord {
        id: record_id.to_string(),
        scope: "project".to_string(),
        framework_name: "registry".to_string(),
        item_id: item.id.clone(),
        item_name: item.name.clone(),
        source_path: item.source_path.clone(),
        link_path: link_path.to_path_buf(),
        link_kind: item.source_kind,
        provider_backend: backend,
        created_by_command: "link".to_string(),
        created_at: now.clone(),
        updated_at: now,
    }
}

fn manifest_record_id(item: &LinkableItem, link_path: &Path) -> String {
    format!(
        "link:{}:{}",
        item.id,
        link_path.to_string_lossy().replace('\\', "/")
    )
}

fn select_unlink_indexes(records: &[LinkRecord], identifier: Option<&str>) -> Result<Vec<usize>> {
    let Some(identifier) = identifier else {
        return Ok((0..records.len()).collect());
    };

    let matches: Vec<usize> = records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            if record.id == identifier
                || record.item_id == identifier
                || record.item_name == identifier
                || record.link_path == Path::new(identifier)
                || record.link_path.file_name().and_then(|name| name.to_str()) == Some(identifier)
            {
                Some(index)
            } else {
                None
            }
        })
        .collect();

    match matches.len() {
        0 => Err(Error::manifest(format!(
            "no managed project link matches `{identifier}`"
        ))),
        1 => Ok(matches),
        _ => Err(Error::invalid_arguments(format!(
            "managed project link `{identifier}` is ambiguous; use a manifest id or link path"
        ))),
    }
}

fn project_path(project_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}

fn validate_project_root(project_root: &Path) -> Result<()> {
    let metadata = fs::metadata(project_root)?;
    if metadata.is_dir() {
        Ok(())
    } else {
        Err(Error::project(format!(
            "project root is not a directory: {}",
            project_root.display()
        )))
    }
}

fn timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    format!("unix:{seconds}")
}

#[cfg(test)]
mod tests {
    use super::{
        link_project_with_provider_and_db, status_project_with_provider,
        unlink_project_with_provider, LinkItemRequest,
    };
    use crate::core::{
        db::{migrate_database, DbPathReason, DbPathResolution},
        linkable::LinkableItemType,
        manifest::load_manifest,
        registry::{add_item, AddLinkableItem},
        symlink::{CreateSymlinkOutcome, LinkKind, LinkStatus, MockEntry, MockSymlinkProvider},
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "agent-linker-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn database(temp_dir: &TestDir) -> DbPathResolution {
        let resolution = DbPathResolution {
            path: temp_dir.path().join("agent-linker.db"),
            reason: DbPathReason::ExplicitDatabaseEnv,
        };
        migrate_database(&resolution).unwrap();
        resolution
    }

    fn seed_skill_source(project: &TestDir) -> PathBuf {
        let source = project.path().join("writer-skill");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), "write\n").unwrap();
        source
    }

    fn seed_provider(project_root: &Path, source: &Path) -> MockSymlinkProvider {
        let mut provider = MockSymlinkProvider::new();
        provider.add_dir(project_root);
        provider.add_dir(project_root.join(".agents"));
        provider.add_dir(project_root.join(".agents").join("skills"));
        provider.add_dir(fs::canonicalize(source).unwrap());
        provider
    }

    #[test]
    fn link_status_and_unlink_round_trip_for_skill() {
        let temp_dir = TestDir::new("project-links");
        fs::create_dir(temp_dir.path().join(".agents")).unwrap();
        fs::create_dir(temp_dir.path().join(".agents").join("skills")).unwrap();
        let source = seed_skill_source(&temp_dir);
        let resolution = database(&temp_dir);
        add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Skill,
                name: "writer".to_string(),
                alias: None,
                source_path: source.clone(),
                default_target_dir: None,
                description: None,
            },
        )
        .unwrap();

        let mut provider = seed_provider(temp_dir.path(), &source);
        let report = link_project_with_provider_and_db(
            temp_dir.path(),
            &mut provider,
            &resolution,
            LinkItemRequest {
                identifier: "writer".to_string(),
                link_name_override: None,
                target_dir_override: None,
            },
        )
        .unwrap();

        assert_eq!(report.outcome, CreateSymlinkOutcome::Created);
        assert_eq!(
            provider.entry(
                &temp_dir
                    .path()
                    .join(".agents")
                    .join("skills")
                    .join("writer")
            ),
            Some(&MockEntry::Symlink {
                target: fs::canonicalize(&source).unwrap(),
                kind: LinkKind::Directory
            })
        );
        assert_eq!(load_manifest(temp_dir.path()).unwrap().links.len(), 1);

        let status = status_project_with_provider(temp_dir.path(), &provider).unwrap();
        assert!(matches!(
            status.entries[0].status,
            LinkStatus::CorrectSymlink { .. }
        ));

        let unlink = unlink_project_with_provider(
            temp_dir.path(),
            &mut provider,
            Some("writer".to_string()),
        )
        .unwrap();
        assert_eq!(unlink.outcomes.len(), 1);
        assert_eq!(
            provider.entry(
                &temp_dir
                    .path()
                    .join(".agents")
                    .join("skills")
                    .join("writer")
            ),
            None
        );
        assert!(load_manifest(temp_dir.path()).unwrap().links.is_empty());
    }

    #[test]
    fn link_as_and_resource_target_dir_override_do_not_change_registry_defaults() {
        let temp_dir = TestDir::new("resource-links");
        let source = temp_dir.path().join("notes.md");
        fs::write(&source, "notes\n").unwrap();
        let resolution = database(&temp_dir);
        let item = add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Resource,
                name: "notes".to_string(),
                alias: None,
                source_path: source.clone(),
                default_target_dir: Some(PathBuf::from(".agents").join("resources")),
                description: None,
            },
        )
        .unwrap();

        let mut provider = MockSymlinkProvider::new();
        provider.add_dir(temp_dir.path());
        provider.add_dir(temp_dir.path().join("docs"));
        provider.add_file(fs::canonicalize(&source).unwrap());

        let report = link_project_with_provider_and_db(
            temp_dir.path(),
            &mut provider,
            &resolution,
            LinkItemRequest {
                identifier: "notes".to_string(),
                link_name_override: Some("project-notes.md".to_string()),
                target_dir_override: Some(PathBuf::from("docs")),
            },
        )
        .unwrap();

        assert_eq!(
            report.link_path,
            PathBuf::from("docs").join("project-notes.md")
        );
        assert_eq!(
            item.default_project_link_path().unwrap(),
            PathBuf::from(".agents").join("resources").join("notes.md")
        );
    }

    #[test]
    fn unlink_refuses_real_file_at_managed_path() {
        let temp_dir = TestDir::new("unlink-real-file");
        fs::create_dir(temp_dir.path().join(".agents")).unwrap();
        fs::create_dir(temp_dir.path().join(".agents").join("skills")).unwrap();
        let source = seed_skill_source(&temp_dir);
        let resolution = database(&temp_dir);
        add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Skill,
                name: "writer".to_string(),
                alias: None,
                source_path: source.clone(),
                default_target_dir: None,
                description: None,
            },
        )
        .unwrap();

        let mut provider = seed_provider(temp_dir.path(), &source);
        link_project_with_provider_and_db(
            temp_dir.path(),
            &mut provider,
            &resolution,
            LinkItemRequest {
                identifier: "writer".to_string(),
                link_name_override: None,
                target_dir_override: None,
            },
        )
        .unwrap();
        provider.add_file(
            temp_dir
                .path()
                .join(".agents")
                .join("skills")
                .join("writer"),
        );

        let error = unlink_project_with_provider(
            temp_dir.path(),
            &mut provider,
            Some("writer".to_string()),
        )
        .unwrap_err();

        assert!(matches!(error, crate::core::Error::Symlink(_)));
        assert_eq!(load_manifest(temp_dir.path()).unwrap().links.len(), 1);
    }
}
