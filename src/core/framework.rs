//! Agent framework adapter entry point.

use std::path::{Path, PathBuf};

use rusqlite::{params, OptionalExtension};

use crate::core::{
    db::{open_migrated_connection, DbPathResolution},
    error::{Error, Result},
    symlink::LinkKind,
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
        link_kind: parse_link_kind(&link_kind).map_err(|error| {
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

fn parse_link_kind(value: &str) -> std::result::Result<LinkKind, ParseLinkKindError> {
    match value {
        "file" => Ok(LinkKind::File),
        "directory" => Ok(LinkKind::Directory),
        _ => Err(ParseLinkKindError(value.to_string())),
    }
}

#[derive(Debug)]
struct ParseLinkKindError(String);

impl std::fmt::Display for ParseLinkKindError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "unknown link kind `{}`", self.0)
    }
}

impl std::error::Error for ParseLinkKindError {}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn int_to_bool(value: i64) -> bool {
    value != 0
}

fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    format!("unix:{seconds}")
}

#[cfg(test)]
mod tests {
    use super::{
        default_init_mappings, disable_framework, enable_framework, enabled_framework_mappings,
        list_all_mappings, list_frameworks, show_framework,
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
}
