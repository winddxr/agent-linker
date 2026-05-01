//! Linkable item registry entry point.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension};

use crate::core::{
    db::{open_migrated_connection, DbPathResolution},
    error::{Error, Result},
    linkable::{
        parse_item_type, parse_link_kind, parse_source_ownership, parse_source_type,
        validate_item_name, validate_optional_alias, validate_project_relative_target_dir,
        validate_resource_source, validate_skill_source, LinkableItem, LinkableItemType,
        SourceOwnership, SourceType,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddLinkableItem {
    pub item_type: LinkableItemType,
    pub name: String,
    pub alias: Option<String>,
    pub source_path: PathBuf,
    pub default_target_dir: Option<PathBuf>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub items: Vec<LinkableItem>,
}

pub struct RegistryStore {
    connection: Connection,
}

impl RegistryStore {
    pub fn open(resolution: &DbPathResolution) -> Result<Self> {
        Ok(Self {
            connection: open_migrated_connection(resolution)?,
        })
    }

    pub fn add_item(&self, request: AddLinkableItem) -> Result<LinkableItem> {
        add_item_with_connection(&self.connection, request)
    }

    pub fn list_items(&self, item_type: LinkableItemType) -> Result<Vec<LinkableItem>> {
        list_items_with_connection(&self.connection, item_type)
    }

    pub fn find_item(&self, item_type: LinkableItemType, identifier: &str) -> Result<LinkableItem> {
        find_item_with_connection(&self.connection, item_type, identifier)
    }

    pub fn find_any_item(&self, identifier: &str) -> Result<LinkableItem> {
        find_any_item_with_connection(&self.connection, identifier)
    }

    pub fn create_group(&self, name: &str, description: Option<&str>) -> Result<Group> {
        create_group_with_connection(&self.connection, name, description)
    }

    pub fn list_groups(&self) -> Result<Vec<Group>> {
        list_groups_with_connection(&self.connection)
    }

    pub fn show_group(&self, identifier: &str) -> Result<Group> {
        find_group_with_connection(&self.connection, identifier)
    }

    pub fn rename_group(&self, identifier: &str, new_name: &str) -> Result<Group> {
        rename_group_with_connection(&self.connection, identifier, new_name)
    }

    pub fn delete_group(&self, identifier: &str) -> Result<Group> {
        delete_group_with_connection(&self.connection, identifier)
    }

    pub fn add_group_items(&self, group: &str, items: &[String]) -> Result<Group> {
        add_group_items_with_connection(&self.connection, group, items)
    }

    pub fn remove_group_items(&self, group: &str, items: &[String]) -> Result<Group> {
        remove_group_items_with_connection(&self.connection, group, items)
    }
}

pub fn add_item(resolution: &DbPathResolution, request: AddLinkableItem) -> Result<LinkableItem> {
    RegistryStore::open(resolution)?.add_item(request)
}

pub fn list_items(
    resolution: &DbPathResolution,
    item_type: LinkableItemType,
) -> Result<Vec<LinkableItem>> {
    RegistryStore::open(resolution)?.list_items(item_type)
}

pub fn show_item(
    resolution: &DbPathResolution,
    item_type: LinkableItemType,
    identifier: &str,
) -> Result<LinkableItem> {
    RegistryStore::open(resolution)?.find_item(item_type, identifier)
}

pub fn rename_item(
    resolution: &DbPathResolution,
    item_type: LinkableItemType,
    identifier: &str,
    new_name: &str,
) -> Result<LinkableItem> {
    validate_item_name(new_name, "name")?;
    let connection = open_migrated_connection(resolution)?;
    let item = find_item_with_connection(&connection, item_type, identifier)?;
    ensure_name_available(&connection, item_type, new_name, Some(&item.id))?;
    let future_link_name = match (item_type, item.alias.as_deref()) {
        (_, Some(alias)) => alias.to_string(),
        (LinkableItemType::Skill, None) => new_name.to_string(),
        (LinkableItemType::Resource, None) => item.link_name(),
    };
    ensure_link_name_available(&connection, item_type, &future_link_name, Some(&item.id))?;

    connection.execute(
        r#"
        UPDATE linkable_items
        SET name = ?2, updated_at = ?3
        WHERE id = ?1
        "#,
        params![item.id, new_name, timestamp()],
    )?;

    find_item_with_connection(&connection, item_type, new_name)
}

pub fn remove_item(
    resolution: &DbPathResolution,
    item_type: LinkableItemType,
    identifier: &str,
) -> Result<LinkableItem> {
    let connection = open_migrated_connection(resolution)?;
    let item = find_item_with_connection(&connection, item_type, identifier)?;
    connection.execute("DELETE FROM linkable_items WHERE id = ?1", params![item.id])?;
    Ok(item)
}

pub fn refresh_item(
    resolution: &DbPathResolution,
    item_type: LinkableItemType,
    identifier: &str,
) -> Result<LinkableItem> {
    let connection = open_migrated_connection(resolution)?;
    let item = find_item_with_connection(&connection, item_type, identifier)?;
    let source = validate_source_for_type(item_type, &item.source_path)?;

    if source.source_kind != item.source_kind {
        return Err(Error::invalid_arguments(format!(
            "{} `{}` source kind changed from {} to {}",
            item_type, item.name, item.source_kind, source.source_kind
        )));
    }

    connection.execute(
        r#"
        UPDATE linkable_items
        SET source_path = ?2, updated_at = ?3
        WHERE id = ?1
        "#,
        params![
            item.id,
            path_to_registry_text(&source.absolute_path, "source path")?,
            timestamp()
        ],
    )?;

    find_item_with_connection(&connection, item_type, identifier)
}

pub fn create_group(
    resolution: &DbPathResolution,
    name: &str,
    description: Option<&str>,
) -> Result<Group> {
    RegistryStore::open(resolution)?.create_group(name, description)
}

pub fn list_groups(resolution: &DbPathResolution) -> Result<Vec<Group>> {
    RegistryStore::open(resolution)?.list_groups()
}

pub fn show_group(resolution: &DbPathResolution, identifier: &str) -> Result<Group> {
    RegistryStore::open(resolution)?.show_group(identifier)
}

pub fn rename_group(
    resolution: &DbPathResolution,
    identifier: &str,
    new_name: &str,
) -> Result<Group> {
    RegistryStore::open(resolution)?.rename_group(identifier, new_name)
}

pub fn delete_group(resolution: &DbPathResolution, identifier: &str) -> Result<Group> {
    RegistryStore::open(resolution)?.delete_group(identifier)
}

pub fn add_group_items(
    resolution: &DbPathResolution,
    group: &str,
    items: &[String],
) -> Result<Group> {
    RegistryStore::open(resolution)?.add_group_items(group, items)
}

pub fn remove_group_items(
    resolution: &DbPathResolution,
    group: &str,
    items: &[String],
) -> Result<Group> {
    RegistryStore::open(resolution)?.remove_group_items(group, items)
}

fn add_item_with_connection(
    connection: &Connection,
    request: AddLinkableItem,
) -> Result<LinkableItem> {
    validate_item_name(&request.name, "name")?;
    validate_optional_alias(request.alias.as_deref())?;
    let source = validate_source_for_type(request.item_type, &request.source_path)?;

    if let Some(target_dir) = &request.default_target_dir {
        validate_project_relative_target_dir(target_dir)?;
    } else if request.item_type == LinkableItemType::Resource {
        return Err(Error::invalid_arguments(
            "resource add requires --target-dir for the default project link path",
        ));
    }

    ensure_name_available(connection, request.item_type, &request.name, None)?;
    let candidate = link_name_for_request(&request, &source.absolute_path);
    ensure_link_name_available(connection, request.item_type, &candidate, None)?;

    let now = timestamp();
    let id = generate_id(request.item_type, &request.name);
    connection.execute(
        r#"
        INSERT INTO linkable_items
            (id, kind, name, alias, source_path, source_kind, target_dir, description,
             version, commit_hash, created_at, updated_at, source_type, source_ownership,
             repo_url, repo_commit)
        VALUES
            (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, ?9, ?9, ?10, ?11, NULL, NULL)
        "#,
        params![
            id,
            request.item_type.as_str(),
            request.name,
            request.alias,
            path_to_registry_text(&source.absolute_path, "source path")?,
            source.source_kind.to_string(),
            request
                .default_target_dir
                .as_ref()
                .map(|path| path_to_registry_text(path, "target directory"))
                .transpose()?,
            request.description,
            now,
            SourceType::LocalPath.as_str(),
            SourceOwnership::External.as_str(),
        ],
    )?;

    find_item_by_id(connection, &id)
}

fn list_items_with_connection(
    connection: &Connection,
    item_type: LinkableItemType,
) -> Result<Vec<LinkableItem>> {
    let mut statement = connection.prepare(
        r#"
        SELECT id, kind, name, alias, source_path, source_kind, target_dir, description,
               created_at, updated_at, source_type, source_ownership, repo_url, repo_commit
        FROM linkable_items
        WHERE kind = ?1
        ORDER BY name
        "#,
    )?;
    let rows = statement.query_map(params![item_type.as_str()], item_from_row)?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

fn find_item_with_connection(
    connection: &Connection,
    item_type: LinkableItemType,
    identifier: &str,
) -> Result<LinkableItem> {
    let mut statement = connection.prepare(
        r#"
        SELECT id, kind, name, alias, source_path, source_kind, target_dir, description,
               created_at, updated_at, source_type, source_ownership, repo_url, repo_commit
        FROM linkable_items
        WHERE kind = ?1 AND (id = ?2 OR name = ?2 OR alias = ?2)
        ORDER BY name
        "#,
    )?;
    let item = statement
        .query_row(params![item_type.as_str(), identifier], item_from_row)
        .optional()?;

    item.ok_or_else(|| Error::database(format!("unknown {} `{identifier}`", item_type.as_str())))
}

fn find_any_item_with_connection(
    connection: &Connection,
    identifier: &str,
) -> Result<LinkableItem> {
    let mut matches = Vec::new();

    for item_type in [LinkableItemType::Skill, LinkableItemType::Resource] {
        let mut statement = connection.prepare(
            r#"
            SELECT id, kind, name, alias, source_path, source_kind, target_dir, description,
                   created_at, updated_at, source_type, source_ownership, repo_url, repo_commit
            FROM linkable_items
            WHERE kind = ?1 AND (id = ?2 OR name = ?2 OR alias = ?2)
            ORDER BY name
            "#,
        )?;
        let rows = statement.query_map(params![item_type.as_str(), identifier], item_from_row)?;
        for row in rows {
            matches.push(row?);
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

fn find_item_by_id(connection: &Connection, id: &str) -> Result<LinkableItem> {
    connection
        .query_row(
            r#"
            SELECT id, kind, name, alias, source_path, source_kind, target_dir, description,
                   created_at, updated_at, source_type, source_ownership, repo_url, repo_commit
            FROM linkable_items
            WHERE id = ?1
            "#,
            params![id],
            item_from_row,
        )
        .map_err(Error::from)
}

fn create_group_with_connection(
    connection: &Connection,
    name: &str,
    description: Option<&str>,
) -> Result<Group> {
    validate_item_name(name, "group name")?;
    ensure_group_name_available(connection, name, None)?;

    let now = timestamp();
    let id = generate_group_id(name);
    connection.execute(
        r#"
        INSERT INTO groups (id, name, description, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?4)
        "#,
        params![id, name, description, now],
    )?;

    find_group_with_connection(connection, name)
}

fn list_groups_with_connection(connection: &Connection) -> Result<Vec<Group>> {
    let mut statement = connection.prepare(
        r#"
        SELECT id, name, description, created_at, updated_at
        FROM groups
        ORDER BY name
        "#,
    )?;
    let rows = statement.query_map([], group_from_row)?;
    let mut groups = Vec::new();
    for row in rows {
        let mut group = row?;
        group.items = group_items_with_connection(connection, &group.id)?;
        groups.push(group);
    }
    Ok(groups)
}

fn find_group_with_connection(connection: &Connection, identifier: &str) -> Result<Group> {
    let mut group = connection
        .query_row(
            r#"
            SELECT id, name, description, created_at, updated_at
            FROM groups
            WHERE id = ?1 OR name = ?1
            "#,
            params![identifier],
            group_from_row,
        )
        .optional()?
        .ok_or_else(|| Error::database(format!("unknown group `{identifier}`")))?;

    group.items = group_items_with_connection(connection, &group.id)?;
    Ok(group)
}

fn rename_group_with_connection(
    connection: &Connection,
    identifier: &str,
    new_name: &str,
) -> Result<Group> {
    validate_item_name(new_name, "group name")?;
    let group = find_group_with_connection(connection, identifier)?;
    ensure_group_name_available(connection, new_name, Some(&group.id))?;

    connection.execute(
        r#"
        UPDATE groups
        SET name = ?2, updated_at = ?3
        WHERE id = ?1
        "#,
        params![group.id, new_name, timestamp()],
    )?;

    find_group_with_connection(connection, new_name)
}

fn delete_group_with_connection(connection: &Connection, identifier: &str) -> Result<Group> {
    let group = find_group_with_connection(connection, identifier)?;
    connection.execute("DELETE FROM groups WHERE id = ?1", params![group.id])?;
    Ok(group)
}

fn add_group_items_with_connection(
    connection: &Connection,
    group_identifier: &str,
    item_identifiers: &[String],
) -> Result<Group> {
    if item_identifiers.is_empty() {
        return Err(Error::invalid_arguments(
            "group add requires at least one item",
        ));
    }

    let group = find_group_with_connection(connection, group_identifier)?;
    let mut items = Vec::new();
    let mut seen = BTreeSet::new();
    for identifier in item_identifiers {
        let item = find_any_item_with_connection(connection, identifier)?;
        if !seen.insert(item.id.clone()) {
            return Err(Error::invalid_arguments(format!(
                "group add contains duplicate item `{}`",
                item.name
            )));
        }
        let exists: Option<String> = connection
            .query_row(
                "SELECT item_id FROM group_items WHERE group_id = ?1 AND item_id = ?2",
                params![group.id, item.id],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_some() {
            return Err(Error::invalid_arguments(format!(
                "group `{}` already contains item `{}`",
                group.name, item.name
            )));
        }
        items.push(item);
    }

    for item in items {
        connection.execute(
            r#"
            INSERT INTO group_items (group_id, item_id, created_at)
            VALUES (?1, ?2, ?3)
            "#,
            params![group.id, item.id, timestamp()],
        )?;
    }

    find_group_with_connection(connection, &group.id)
}

fn remove_group_items_with_connection(
    connection: &Connection,
    group_identifier: &str,
    item_identifiers: &[String],
) -> Result<Group> {
    if item_identifiers.is_empty() {
        return Err(Error::invalid_arguments(
            "group remove requires at least one item",
        ));
    }

    let group = find_group_with_connection(connection, group_identifier)?;
    let mut items = Vec::new();
    let mut seen = BTreeSet::new();
    for identifier in item_identifiers {
        let item = find_any_item_with_connection(connection, identifier)?;
        if !seen.insert(item.id.clone()) {
            return Err(Error::invalid_arguments(format!(
                "group remove contains duplicate item `{}`",
                item.name
            )));
        }
        let exists: Option<String> = connection
            .query_row(
                "SELECT item_id FROM group_items WHERE group_id = ?1 AND item_id = ?2",
                params![group.id, item.id],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(Error::invalid_arguments(format!(
                "group `{}` does not contain item `{}`",
                group.name, item.name
            )));
        }
        items.push(item);
    }

    for item in items {
        connection.execute(
            "DELETE FROM group_items WHERE group_id = ?1 AND item_id = ?2",
            params![group.id, item.id],
        )?;
    }

    find_group_with_connection(connection, &group.id)
}

fn group_items_with_connection(
    connection: &Connection,
    group_id: &str,
) -> Result<Vec<LinkableItem>> {
    let mut statement = connection.prepare(
        r#"
        SELECT i.id, i.kind, i.name, i.alias, i.source_path, i.source_kind, i.target_dir,
               i.description, i.created_at, i.updated_at, i.source_type, i.source_ownership,
               i.repo_url, i.repo_commit
        FROM group_items gi
        JOIN linkable_items i ON i.id = gi.item_id
        WHERE gi.group_id = ?1
        ORDER BY i.kind, i.name
        "#,
    )?;
    let rows = statement.query_map(params![group_id], item_from_row)?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

fn group_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Group> {
    Ok(Group {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        items: Vec::new(),
    })
}

fn item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LinkableItem> {
    let item_type =
        parse_item_type(&row.get::<_, String>(1)?).map_err(|error| conversion_error(1, error))?;
    let source_kind =
        parse_link_kind(&row.get::<_, String>(5)?).map_err(|error| conversion_error(5, error))?;
    let source_type = parse_source_type(&row.get::<_, String>(10)?)
        .map_err(|error| conversion_error(10, error))?;
    let source_ownership = parse_source_ownership(&row.get::<_, String>(11)?)
        .map_err(|error| conversion_error(11, error))?;

    Ok(LinkableItem {
        id: row.get(0)?,
        item_type,
        name: row.get(2)?,
        alias: row.get(3)?,
        source_path: PathBuf::from(row.get::<_, String>(4)?),
        source_kind,
        default_target_dir: row.get::<_, Option<String>>(6)?.map(PathBuf::from),
        description: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        source_type,
        source_ownership,
        repo_url: row.get(12)?,
        repo_commit: row.get(13)?,
    })
}

fn conversion_error(index: usize, error: Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(error))
}

fn validate_source_for_type(
    item_type: LinkableItemType,
    source_path: &Path,
) -> Result<crate::core::linkable::ValidatedSource> {
    match item_type {
        LinkableItemType::Skill => validate_skill_source(source_path),
        LinkableItemType::Resource => validate_resource_source(source_path),
    }
}

fn ensure_name_available(
    connection: &Connection,
    item_type: LinkableItemType,
    name: &str,
    excluding_id: Option<&str>,
) -> Result<()> {
    let existing = connection
        .query_row(
            r#"
            SELECT id, source_path
            FROM linkable_items
            WHERE kind = ?1 AND name = ?2
            "#,
            params![item_type.as_str(), name],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;

    if let Some((id, source_path)) = existing {
        if excluding_id == Some(id.as_str()) {
            return Ok(());
        }
        return Err(Error::invalid_arguments(format!(
            "{} `{name}` is already registered from {}",
            item_type, source_path
        )));
    }

    Ok(())
}

fn ensure_link_name_available(
    connection: &Connection,
    item_type: LinkableItemType,
    link_name: &str,
    excluding_id: Option<&str>,
) -> Result<()> {
    for item in list_items_with_connection(connection, item_type)? {
        if excluding_id == Some(item.id.as_str()) {
            continue;
        }
        if item.link_name() == link_name {
            return Err(Error::invalid_arguments(format!(
                "{} link name `{link_name}` is already used by `{}`",
                item_type, item.name
            )));
        }
    }
    Ok(())
}

fn ensure_group_name_available(
    connection: &Connection,
    name: &str,
    excluding_id: Option<&str>,
) -> Result<()> {
    let existing = connection
        .query_row(
            "SELECT id FROM groups WHERE name = ?1",
            params![name],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    if let Some(id) = existing {
        if excluding_id == Some(id.as_str()) {
            return Ok(());
        }
        return Err(Error::invalid_arguments(format!(
            "group `{name}` already exists"
        )));
    }

    Ok(())
}

fn link_name_for_request(request: &AddLinkableItem, source_path: &Path) -> String {
    if let Some(alias) = &request.alias {
        return alias.clone();
    }

    match request.item_type {
        LinkableItemType::Skill => request.name.clone(),
        LinkableItemType::Resource => source_path
            .file_name()
            .and_then(|name| name.to_str())
            .map_or_else(|| request.name.clone(), str::to_string),
    }
}

fn path_to_registry_text(path: &Path, label: &str) -> Result<String> {
    path.to_str().map(str::to_string).ok_or_else(|| {
        Error::invalid_arguments(format!(
            "{label} must be valid UTF-8 to store in the global registry"
        ))
    })
}

fn generate_id(item_type: LinkableItemType, name: &str) -> String {
    format!(
        "{}:{}:{}",
        item_type.as_str(),
        name,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    )
}

fn generate_group_id(name: &str) -> String {
    format!(
        "group:{}:{}",
        name,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    )
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
        add_group_items, add_item, create_group, delete_group, list_items, refresh_item,
        remove_group_items, remove_item, rename_group, rename_item, show_group, show_item,
        AddLinkableItem, LinkableItemType,
    };
    use crate::core::db::{migrate_database, DbPathReason, DbPathResolution};
    use crate::core::symlink::LinkKind;
    #[cfg(unix)]
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
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
                "agent-linker-registry-{label}-{}-{unique}",
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

    fn resolution(temp_dir: &Path) -> DbPathResolution {
        DbPathResolution {
            path: temp_dir.join("agent-linker.db"),
            reason: DbPathReason::ExplicitDatabaseEnv,
        }
    }

    #[test]
    fn skill_registration_validates_source_and_calculates_default_link_path() {
        let temp_dir = TestDir::new("skill-add");
        let skill_dir = temp_dir.path().join("skill-source");
        fs::create_dir(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "Use this skill.").unwrap();
        let resolution = resolution(temp_dir.path());
        migrate_database(&resolution).unwrap();

        let item = add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Skill,
                name: "writer".to_string(),
                alias: Some("writing-helper".to_string()),
                source_path: skill_dir.clone(),
                default_target_dir: None,
                description: None,
            },
        )
        .unwrap();

        assert_eq!(item.source_kind, LinkKind::Directory);
        assert!(item.source_path.is_absolute());
        assert_eq!(item.link_name(), "writing-helper");
        assert_eq!(
            item.default_project_link_path().unwrap(),
            PathBuf::from(".agents")
                .join("skills")
                .join("writing-helper")
        );
    }

    #[test]
    fn skill_registration_rejects_missing_or_empty_skill_md() {
        let temp_dir = TestDir::new("skill-invalid");
        let skill_dir = temp_dir.path().join("skill-source");
        fs::create_dir(&skill_dir).unwrap();
        let resolution = resolution(temp_dir.path());
        migrate_database(&resolution).unwrap();

        let missing = add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Skill,
                name: "bad".to_string(),
                alias: None,
                source_path: skill_dir.clone(),
                default_target_dir: None,
                description: None,
            },
        )
        .unwrap_err();
        assert!(missing.to_string().contains("SKILL.md"));

        fs::write(skill_dir.join("SKILL.md"), "  \n").unwrap();
        let empty = add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Skill,
                name: "empty".to_string(),
                alias: None,
                source_path: skill_dir,
                default_target_dir: None,
                description: None,
            },
        )
        .unwrap_err();
        assert!(empty.to_string().contains("must not be empty"));
    }

    #[test]
    fn resource_registration_detects_file_and_directory_sources() {
        let temp_dir = TestDir::new("resource-kind");
        let file_source = temp_dir.path().join("notes.md");
        fs::write(&file_source, "notes").unwrap();
        let dir_source = temp_dir.path().join("assets");
        fs::create_dir(&dir_source).unwrap();
        let resolution = resolution(temp_dir.path());
        migrate_database(&resolution).unwrap();

        let file = add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Resource,
                name: "notes".to_string(),
                alias: None,
                source_path: file_source,
                default_target_dir: Some(PathBuf::from(".agents").join("resources")),
                description: None,
            },
        )
        .unwrap();
        assert_eq!(file.source_kind, LinkKind::File);
        assert_eq!(
            file.default_project_link_path().unwrap(),
            PathBuf::from(".agents").join("resources").join("notes.md")
        );

        let dir = add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Resource,
                name: "assets".to_string(),
                alias: Some("shared-assets".to_string()),
                source_path: dir_source,
                default_target_dir: Some(PathBuf::from("vendor")),
                description: None,
            },
        )
        .unwrap();
        assert_eq!(dir.source_kind, LinkKind::Directory);
        assert_eq!(
            dir.default_project_link_path().unwrap(),
            PathBuf::from("vendor").join("shared-assets")
        );
    }

    #[test]
    fn resource_registration_requires_project_target_directory() {
        let temp_dir = TestDir::new("resource-target-required");
        let file_source = temp_dir.path().join("notes.md");
        fs::write(&file_source, "notes").unwrap();
        let resolution = resolution(temp_dir.path());
        migrate_database(&resolution).unwrap();

        let error = add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Resource,
                name: "notes".to_string(),
                alias: None,
                source_path: file_source.clone(),
                default_target_dir: None,
                description: None,
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("--target-dir"));

        let absolute_target = add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Resource,
                name: "absolute-target".to_string(),
                alias: None,
                source_path: file_source,
                default_target_dir: Some(temp_dir.path().join("out")),
                description: None,
            },
        )
        .unwrap_err();
        assert!(absolute_target.to_string().contains("relative"));
    }

    #[cfg(unix)]
    #[test]
    fn registry_rejects_non_utf8_source_paths() {
        let temp_dir = TestDir::new("non-utf8-source");
        let source = temp_dir.path().join(PathBuf::from(OsString::from_vec(vec![
            b'n', b'o', b't', 0xff,
        ])));
        fs::write(&source, "resource").unwrap();
        let resolution = resolution(temp_dir.path());
        migrate_database(&resolution).unwrap();

        let error = add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Resource,
                name: "non-utf8".to_string(),
                alias: None,
                source_path: source,
                default_target_dir: Some(PathBuf::from(".agents").join("resources")),
                description: None,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("valid UTF-8"));
    }

    #[test]
    fn registry_rejects_same_name_and_same_alias_conflicts() {
        let temp_dir = TestDir::new("conflicts");
        let skill_a = temp_dir.path().join("a");
        let skill_b = temp_dir.path().join("b");
        fs::create_dir(&skill_a).unwrap();
        fs::create_dir(&skill_b).unwrap();
        fs::write(skill_a.join("SKILL.md"), "a").unwrap();
        fs::write(skill_b.join("SKILL.md"), "b").unwrap();
        let resolution = resolution(temp_dir.path());
        migrate_database(&resolution).unwrap();

        add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Skill,
                name: "dup".to_string(),
                alias: Some("common".to_string()),
                source_path: skill_a,
                default_target_dir: None,
                description: None,
            },
        )
        .unwrap();

        let same_name = add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Skill,
                name: "dup".to_string(),
                alias: Some("other".to_string()),
                source_path: skill_b.clone(),
                default_target_dir: None,
                description: None,
            },
        )
        .unwrap_err();
        assert!(same_name.to_string().contains("already registered"));

        let same_alias = add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Skill,
                name: "unique".to_string(),
                alias: Some("common".to_string()),
                source_path: skill_b,
                default_target_dir: None,
                description: None,
            },
        )
        .unwrap_err();
        assert!(same_alias.to_string().contains("link name"));
    }

    #[test]
    fn rename_and_refresh_do_not_change_source_contents() {
        let temp_dir = TestDir::new("rename-refresh");
        let source = temp_dir.path().join("skill-source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), "stable").unwrap();
        let resolution = resolution(temp_dir.path());
        migrate_database(&resolution).unwrap();

        add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Skill,
                name: "old".to_string(),
                alias: None,
                source_path: source.clone(),
                default_target_dir: None,
                description: None,
            },
        )
        .unwrap();

        let renamed = rename_item(&resolution, LinkableItemType::Skill, "old", "new").unwrap();
        assert_eq!(renamed.name, "new");
        let refreshed = refresh_item(&resolution, LinkableItemType::Skill, "new").unwrap();
        assert_eq!(refreshed.source_kind, LinkKind::Directory);
        assert_eq!(
            fs::read_to_string(source.join("SKILL.md")).unwrap(),
            "stable"
        );
    }

    #[test]
    fn show_and_remove_operate_only_on_registry_rows() {
        let temp_dir = TestDir::new("show-remove");
        let source = temp_dir.path().join("skill-source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), "stable").unwrap();
        let resolution = resolution(temp_dir.path());
        migrate_database(&resolution).unwrap();

        add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Skill,
                name: "stored".to_string(),
                alias: None,
                source_path: source.clone(),
                default_target_dir: None,
                description: None,
            },
        )
        .unwrap();

        assert_eq!(
            show_item(&resolution, LinkableItemType::Skill, "stored")
                .unwrap()
                .source_path,
            fs::canonicalize(&source).unwrap()
        );

        let removed = remove_item(&resolution, LinkableItemType::Skill, "stored").unwrap();
        assert_eq!(removed.name, "stored");
        assert!(source.join("SKILL.md").is_file());
        assert!(show_item(&resolution, LinkableItemType::Skill, "stored").is_err());
    }

    #[test]
    fn refresh_reports_kind_mismatch() {
        let temp_dir = TestDir::new("refresh-kind-mismatch");
        let source = temp_dir.path().join("resource");
        fs::write(&source, "file").unwrap();
        let resolution = resolution(temp_dir.path());
        migrate_database(&resolution).unwrap();

        add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Resource,
                name: "res".to_string(),
                alias: None,
                source_path: source.clone(),
                default_target_dir: Some(PathBuf::from(".agents").join("resources")),
                description: None,
            },
        )
        .unwrap();

        fs::remove_file(&source).unwrap();
        fs::create_dir(&source).unwrap();
        let error = refresh_item(&resolution, LinkableItemType::Resource, "res").unwrap_err();
        assert!(error.to_string().contains("source kind changed"));
    }

    #[test]
    fn list_items_is_scoped_by_type() {
        let temp_dir = TestDir::new("list");
        let skill = temp_dir.path().join("skill");
        fs::create_dir(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "skill").unwrap();
        let resource = temp_dir.path().join("resource.md");
        fs::write(&resource, "resource").unwrap();
        let resolution = resolution(temp_dir.path());
        migrate_database(&resolution).unwrap();

        add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Skill,
                name: "skill".to_string(),
                alias: None,
                source_path: skill,
                default_target_dir: None,
                description: None,
            },
        )
        .unwrap();
        add_item(
            &resolution,
            AddLinkableItem {
                item_type: LinkableItemType::Resource,
                name: "resource".to_string(),
                alias: None,
                source_path: resource,
                default_target_dir: Some(PathBuf::from(".agents").join("resources")),
                description: None,
            },
        )
        .unwrap();

        assert_eq!(
            list_items(&resolution, LinkableItemType::Skill)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            list_items(&resolution, LinkableItemType::Resource)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn group_lifecycle_tracks_items_without_touching_sources() {
        let temp_dir = TestDir::new("group-lifecycle");
        let source = temp_dir.path().join("skill-source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), "stable").unwrap();
        let resolution = resolution(temp_dir.path());
        migrate_database(&resolution).unwrap();

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
        let group = create_group(&resolution, "daily", None).unwrap();
        assert_eq!(group.items.len(), 0);

        let updated = add_group_items(&resolution, "daily", &["writer".to_string()]).unwrap();
        assert_eq!(updated.items.len(), 1);
        assert_eq!(
            show_group(&resolution, "daily").unwrap().items[0].name,
            "writer"
        );

        let duplicate = add_group_items(&resolution, "daily", &["writer".to_string()]).unwrap_err();
        assert!(duplicate.to_string().contains("already contains"));

        let missing = add_group_items(&resolution, "daily", &["missing".to_string()]).unwrap_err();
        assert!(missing.to_string().contains("unknown linkable item"));

        let renamed = rename_group(&resolution, "daily", "workday").unwrap();
        assert_eq!(renamed.name, "workday");

        let removed = remove_group_items(&resolution, "workday", &["writer".to_string()]).unwrap();
        assert!(removed.items.is_empty());
        let deleted = delete_group(&resolution, "workday").unwrap();
        assert_eq!(deleted.name, "workday");
        assert!(source.join("SKILL.md").is_file());
        assert!(show_group(&resolution, "workday").is_err());
    }
}
