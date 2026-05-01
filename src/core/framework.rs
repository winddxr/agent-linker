//! Agent framework adapter entry point.

use std::{
    path::{Component, Path, PathBuf},
    str::FromStr,
};

use rusqlite::{params, OptionalExtension};

use crate::core::{
    db::{open_migrated_connection, DbPathResolution},
    error::{Error, Result},
    symlink::LinkKind,
    util::{bool_to_i64, timestamp, timestamp_nanos},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameworkDefinition {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub built_in: bool,
    pub enabled_by_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameworkMapping {
    pub id: String,
    pub framework_id: String,
    pub framework_name: String,
    pub item_id: String,
    pub item_name: String,
    pub source_path: PathBuf,
    pub link_path: PathBuf,
    pub link_kind: LinkKind,
    pub required: bool,
}

impl FrameworkMapping {
    pub fn source_in(&self, project_root: &Path) -> PathBuf {
        project_root.join(&self.source_path)
    }

    pub fn link_in(&self, project_root: &Path) -> PathBuf {
        project_root.join(&self.link_path)
    }
}

pub fn built_in_claude() -> FrameworkDefinition {
    FrameworkDefinition {
        id: "claude".to_string(),
        name: "claude".to_string(),
        display_name: "Claude".to_string(),
        built_in: true,
        enabled_by_default: true,
    }
}

pub fn default_init_mappings() -> Vec<FrameworkMapping> {
    let claude = built_in_claude();

    vec![
        FrameworkMapping {
            id: "init:claude:agents-md".to_string(),
            framework_id: claude.id.clone(),
            framework_name: claude.name.clone(),
            item_id: "project:agents-md".to_string(),
            item_name: "AGENTS.md".to_string(),
            source_path: PathBuf::from("AGENTS.md"),
            link_path: PathBuf::from("CLAUDE.md"),
            link_kind: LinkKind::File,
            required: true,
        },
        FrameworkMapping {
            id: "init:claude:skills-dir".to_string(),
            framework_id: claude.id,
            framework_name: claude.name,
            item_id: "project:skills-dir".to_string(),
            item_name: ".agents/skills".to_string(),
            source_path: PathBuf::from(".agents").join("skills"),
            link_path: PathBuf::from(".claude").join("skills"),
            link_kind: LinkKind::Directory,
            required: true,
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFramework {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub built_in: bool,
    pub enabled_by_default: bool,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
    pub mappings: Vec<StoredFrameworkMapping>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFrameworkMapping {
    pub id: String,
    pub framework_id: String,
    pub source_path: PathBuf,
    pub link_path: PathBuf,
    pub link_kind: LinkKind,
    pub required: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddFrameworkMapping {
    pub framework: String,
    pub source_path: PathBuf,
    pub link_path: PathBuf,
    pub link_kind: LinkKind,
}

pub fn list_frameworks(resolution: &DbPathResolution) -> Result<Vec<StoredFramework>> {
    let connection = open_migrated_connection(resolution)?;
    let mut statement = connection.prepare(
        r#"
        SELECT id, name, display_name, built_in, enabled_by_default, enabled, created_at, updated_at
        FROM frameworks
        ORDER BY name
        "#,
    )?;

    let rows = statement.query_map([], |row| {
        Ok(StoredFramework {
            id: row.get(0)?,
            name: row.get(1)?,
            display_name: row.get(2)?,
            built_in: int_to_bool(row.get::<_, i64>(3)?),
            enabled_by_default: int_to_bool(row.get::<_, i64>(4)?),
            enabled: int_to_bool(row.get::<_, i64>(5)?),
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            mappings: Vec::new(),
        })
    })?;

    let mut frameworks = Vec::new();
    for row in rows {
        let mut framework = row?;
        framework.mappings = list_framework_mappings(&connection, Some(&framework.id))?;
        frameworks.push(framework);
    }

    Ok(frameworks)
}

pub fn show_framework(
    resolution: &DbPathResolution,
    framework_id: &str,
) -> Result<StoredFramework> {
    let connection = open_migrated_connection(resolution)?;
    let mut framework = connection
        .query_row(
            r#"
            SELECT id, name, display_name, built_in, enabled_by_default, enabled, created_at, updated_at
            FROM frameworks
            WHERE id = ?1 OR name = ?1
            "#,
            params![framework_id],
            |row| {
                Ok(StoredFramework {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    display_name: row.get(2)?,
                    built_in: int_to_bool(row.get::<_, i64>(3)?),
                    enabled_by_default: int_to_bool(row.get::<_, i64>(4)?),
                    enabled: int_to_bool(row.get::<_, i64>(5)?),
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    mappings: Vec::new(),
                })
            },
        )
        .optional()?
        .ok_or_else(|| Error::database(format!("unknown framework `{framework_id}`")))?;

    framework.mappings = list_framework_mappings(&connection, Some(&framework.id))?;
    Ok(framework)
}

pub fn enable_framework(resolution: &DbPathResolution, framework_id: &str) -> Result<()> {
    set_framework_enabled(resolution, framework_id, true)
}

pub fn disable_framework(resolution: &DbPathResolution, framework_id: &str) -> Result<()> {
    set_framework_enabled(resolution, framework_id, false)
}

pub fn list_all_mappings(resolution: &DbPathResolution) -> Result<Vec<StoredFrameworkMapping>> {
    let connection = open_migrated_connection(resolution)?;
    list_framework_mappings(&connection, None)
}

pub fn list_mappings_for_framework(
    resolution: &DbPathResolution,
    framework_id: &str,
) -> Result<Vec<StoredFrameworkMapping>> {
    let framework = show_framework(resolution, framework_id)?;
    Ok(framework.mappings)
}

pub fn enabled_framework_mappings(
    resolution: &DbPathResolution,
) -> Result<Vec<StoredFrameworkMapping>> {
    let connection = open_migrated_connection(resolution)?;
    let mut statement = connection.prepare(
        r#"
        SELECT m.id, m.framework_id, m.source_path, m.link_path, m.link_kind, m.required,
               m.created_at, m.updated_at
        FROM framework_mappings m
        JOIN frameworks f ON f.id = m.framework_id
        WHERE f.enabled = 1
        ORDER BY f.name, m.id
        "#,
    )?;

    let rows = statement.query_map([], stored_mapping_from_row)?;
    let mut mappings = Vec::new();
    for row in rows {
        mappings.push(row?);
    }
    Ok(mappings)
}

pub fn add_mapping(
    resolution: &DbPathResolution,
    request: AddFrameworkMapping,
) -> Result<StoredFrameworkMapping> {
    validate_mapping_path(&request.source_path, "source path")?;
    validate_mapping_path(&request.link_path, "link path")?;
    let connection = open_migrated_connection(resolution)?;
    let framework_id = find_framework_id(&connection, &request.framework)?;
    let source_text = path_to_db_text(&request.source_path, "source path")?;
    let link_text = path_to_db_text(&request.link_path, "link path")?;

    let existing: Option<String> = connection
        .query_row(
            r#"
            SELECT id
            FROM framework_mappings
            WHERE framework_id = ?1 AND link_path = ?2
            "#,
            params![framework_id, link_text],
            |row| row.get(0),
        )
        .optional()?;
    if existing.is_some() {
        return Err(Error::invalid_arguments(format!(
            "framework `{}` already has mapping for `{}`",
            request.framework,
            request.link_path.display()
        )));
    }

    let id = format!("mapping:{}:{}", framework_id, timestamp_nanos());
    let now = timestamp();
    connection.execute(
        r#"
        INSERT INTO framework_mappings
            (id, framework_id, source_path, link_path, link_kind, required, created_at, updated_at)
        VALUES
            (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6)
        "#,
        params![
            id,
            framework_id,
            source_text,
            link_text,
            request.link_kind.to_string(),
            now,
        ],
    )?;

    find_mapping_by_id(&connection, &id)
}

pub fn remove_mapping(
    resolution: &DbPathResolution,
    framework: &str,
    link_path: &Path,
) -> Result<StoredFrameworkMapping> {
    validate_mapping_path(link_path, "link path")?;
    let connection = open_migrated_connection(resolution)?;
    let framework_id = find_framework_id(&connection, framework)?;
    let link_text = path_to_db_text(link_path, "link path")?;
    let mapping = find_mapping_by_framework_and_link(&connection, &framework_id, &link_text)?;

    if mapping.required {
        return Err(Error::invalid_arguments(format!(
            "required framework mapping `{}` cannot be removed",
            mapping.id
        )));
    }

    connection.execute(
        "DELETE FROM framework_mappings WHERE id = ?1",
        params![&mapping.id],
    )?;
    Ok(mapping)
}

fn set_framework_enabled(
    resolution: &DbPathResolution,
    framework_id: &str,
    enabled: bool,
) -> Result<()> {
    let connection = open_migrated_connection(resolution)?;
    let changed = connection.execute(
        r#"
        UPDATE frameworks
        SET enabled = ?2, updated_at = ?3
        WHERE id = ?1 OR name = ?1
        "#,
        params![framework_id, bool_to_i64(enabled), timestamp()],
    )?;

    if changed == 0 {
        return Err(Error::database(format!(
            "unknown framework `{framework_id}`"
        )));
    }

    Ok(())
}

fn list_framework_mappings(
    connection: &rusqlite::Connection,
    framework_id: Option<&str>,
) -> Result<Vec<StoredFrameworkMapping>> {
    let (sql, bind_framework) = match framework_id {
        Some(framework_id) => (
            r#"
            SELECT id, framework_id, source_path, link_path, link_kind, required, created_at, updated_at
            FROM framework_mappings
            WHERE framework_id = ?1
            ORDER BY id
            "#,
            Some(framework_id),
        ),
        None => (
            r#"
            SELECT id, framework_id, source_path, link_path, link_kind, required, created_at, updated_at
            FROM framework_mappings
            ORDER BY framework_id, id
            "#,
            None,
        ),
    };

    let mut statement = connection.prepare(sql)?;
    let mut mappings = Vec::new();

    match bind_framework {
        Some(framework_id) => {
            let rows = statement.query_map(params![framework_id], stored_mapping_from_row)?;
            for row in rows {
                mappings.push(row?);
            }
        }
        None => {
            let rows = statement.query_map([], stored_mapping_from_row)?;
            for row in rows {
                mappings.push(row?);
            }
        }
    }

    Ok(mappings)
}

fn stored_mapping_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredFrameworkMapping> {
    let link_kind: String = row.get(4)?;
    Ok(StoredFrameworkMapping {
        id: row.get(0)?,
        framework_id: row.get(1)?,
        source_path: PathBuf::from(row.get::<_, String>(2)?),
        link_path: PathBuf::from(row.get::<_, String>(3)?),
        link_kind: LinkKind::from_str(&link_kind).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        required: int_to_bool(row.get::<_, i64>(5)?),
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn int_to_bool(value: i64) -> bool {
    value != 0
}

fn find_framework_id(connection: &rusqlite::Connection, framework: &str) -> Result<String> {
    connection
        .query_row(
            "SELECT id FROM frameworks WHERE id = ?1 OR name = ?1",
            params![framework],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| Error::database(format!("unknown framework `{framework}`")))
}

fn find_mapping_by_id(
    connection: &rusqlite::Connection,
    id: &str,
) -> Result<StoredFrameworkMapping> {
    connection
        .query_row(
            r#"
            SELECT id, framework_id, source_path, link_path, link_kind, required, created_at, updated_at
            FROM framework_mappings
            WHERE id = ?1
            "#,
            params![id],
            stored_mapping_from_row,
        )
        .map_err(Error::from)
}

fn find_mapping_by_framework_and_link(
    connection: &rusqlite::Connection,
    framework_id: &str,
    link_path: &str,
) -> Result<StoredFrameworkMapping> {
    connection
        .query_row(
            r#"
            SELECT id, framework_id, source_path, link_path, link_kind, required, created_at, updated_at
            FROM framework_mappings
            WHERE framework_id = ?1 AND link_path = ?2
            "#,
            params![framework_id, link_path],
            stored_mapping_from_row,
        )
        .optional()?
        .ok_or_else(|| {
            Error::database(format!(
                "framework `{framework_id}` has no mapping for `{link_path}`"
            ))
        })
}

fn validate_mapping_path(path: &Path, label: &str) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(Error::invalid_arguments(format!(
            "{label} must not be empty"
        )));
    }

    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(Error::invalid_arguments(format!(
            "{label} must stay inside the project: {}",
            path.display()
        )));
    }
    Ok(())
}

fn path_to_db_text(path: &Path, label: &str) -> Result<String> {
    path.to_str().map(str::to_string).ok_or_else(|| {
        Error::invalid_arguments(format!(
            "{label} must be valid UTF-8 to store in the global registry"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::{
        add_mapping, default_init_mappings, disable_framework, enable_framework,
        enabled_framework_mappings, list_all_mappings, list_frameworks, remove_mapping,
        show_framework, AddFrameworkMapping,
    };
    use crate::core::db::{migrate_database, DbPathReason, DbPathResolution};
    use crate::core::symlink::LinkKind;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn claude_init_mappings_are_built_in_defaults() {
        let mappings = default_init_mappings();

        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings[0].framework_name, "claude");
        assert_eq!(mappings[0].source_path, PathBuf::from("AGENTS.md"));
        assert_eq!(mappings[0].link_path, PathBuf::from("CLAUDE.md"));
        assert_eq!(mappings[0].link_kind, LinkKind::File);
        assert_eq!(
            mappings[1].source_path,
            PathBuf::from(".agents").join("skills")
        );
        assert_eq!(
            mappings[1].link_path,
            PathBuf::from(".claude").join("skills")
        );
        assert_eq!(mappings[1].link_kind, LinkKind::Directory);
    }

    #[test]
    fn framework_api_lists_shows_toggles_and_returns_mappings() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(
            "agent-linker-framework-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&temp_dir).unwrap();
        let resolution = DbPathResolution {
            path: temp_dir.join("agent-linker.db"),
            reason: DbPathReason::ExplicitDatabaseEnv,
        };

        migrate_database(&resolution).unwrap();

        let frameworks = list_frameworks(&resolution).unwrap();
        assert_eq!(frameworks.len(), 1);
        assert_eq!(frameworks[0].name, "claude");
        assert!(frameworks[0].enabled);
        assert_eq!(frameworks[0].mappings.len(), 2);

        let claude = show_framework(&resolution, "claude").unwrap();
        assert_eq!(claude.display_name, "Claude");

        let mappings = list_all_mappings(&resolution).unwrap();
        assert_eq!(mappings.len(), 2);
        assert!(mappings.iter().any(|mapping| {
            mapping.source_path == PathBuf::from("AGENTS.md")
                && mapping.link_path == PathBuf::from("CLAUDE.md")
                && mapping.link_kind == LinkKind::File
        }));

        disable_framework(&resolution, "claude").unwrap();
        assert!(enabled_framework_mappings(&resolution).unwrap().is_empty());
        enable_framework(&resolution, "claude").unwrap();
        assert_eq!(enabled_framework_mappings(&resolution).unwrap().len(), 2);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn framework_mapping_add_and_remove_manage_non_required_mappings() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(
            "agent-linker-framework-mapping-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&temp_dir).unwrap();
        let resolution = DbPathResolution {
            path: temp_dir.join("agent-linker.db"),
            reason: DbPathReason::ExplicitDatabaseEnv,
        };
        migrate_database(&resolution).unwrap();

        let mapping = add_mapping(
            &resolution,
            AddFrameworkMapping {
                framework: "claude".to_string(),
                source_path: PathBuf::from("AGENTS.md"),
                link_path: PathBuf::from(".claude").join("extra.md"),
                link_kind: LinkKind::File,
            },
        )
        .unwrap();

        assert!(!mapping.required);
        assert!(list_all_mappings(&resolution)
            .unwrap()
            .iter()
            .any(|stored| stored.id == mapping.id));

        let removed = remove_mapping(&resolution, "claude", &mapping.link_path).unwrap();
        assert_eq!(removed.id, mapping.id);

        let required =
            remove_mapping(&resolution, "claude", &PathBuf::from("CLAUDE.md")).unwrap_err();
        assert!(required.to_string().contains("required"));

        let _ = fs::remove_dir_all(temp_dir);
    }
}
