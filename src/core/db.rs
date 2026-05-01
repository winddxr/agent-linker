//! Global SQLite database entry point.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};

use crate::core::{
    error::{Error, Result},
    symlink::LinkKind,
    util::{bool_to_i64, timestamp, timestamp_nanos},
};

const DB_FILE_NAME: &str = "agent-linker.db";
const DATA_DIR_NAME: &str = "agent-linker";
const LATEST_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbPathReason {
    ExplicitDatabaseEnv,
    ExplicitHomeEnv,
    PortableDatabase,
    PlatformDefault,
}

impl DbPathReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            DbPathReason::ExplicitDatabaseEnv => "AGLINK_DB",
            DbPathReason::ExplicitHomeEnv => "AGLINK_HOME",
            DbPathReason::PortableDatabase => "portable agent-linker.db next to executable",
            DbPathReason::PlatformDefault => "platform default",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbPathResolution {
    pub path: PathBuf,
    pub reason: DbPathReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetPlatform {
    Windows,
    Macos,
    Linux,
}

#[derive(Debug, Clone)]
pub struct DbPathContext {
    pub aglink_db: Option<PathBuf>,
    pub aglink_home: Option<PathBuf>,
    pub executable_path: Option<PathBuf>,
    pub appdata: Option<PathBuf>,
    pub home: Option<PathBuf>,
    pub xdg_data_home: Option<PathBuf>,
    pub platform: TargetPlatform,
}

impl DbPathContext {
    pub fn from_environment() -> Self {
        Self {
            aglink_db: env::var_os("AGLINK_DB")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            aglink_home: env::var_os("AGLINK_HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            executable_path: env::current_exe().ok(),
            appdata: env::var_os("APPDATA")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            home: home_from_environment(),
            xdg_data_home: env::var_os("XDG_DATA_HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            platform: current_platform(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    pub path: PathBuf,
    pub reason: DbPathReason,
    pub previous_version: u32,
    pub current_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbCheckReport {
    pub path: PathBuf,
    pub reason: DbPathReason,
    pub exists: bool,
    pub writable: bool,
    pub schema_version: Option<u32>,
    pub latest_schema_version: u32,
    pub framework_count: Option<u32>,
    pub mapping_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigUnsetReport {
    pub key: String,
    pub removed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbBackupReport {
    pub source_path: PathBuf,
    pub backup_path: PathBuf,
    pub bytes: u64,
}

impl DbCheckReport {
    pub fn is_ok(&self) -> bool {
        self.exists
            && self.writable
            && self.schema_version == Some(self.latest_schema_version)
            && self.framework_count.unwrap_or(0) > 0
            && self.mapping_count.unwrap_or(0) > 0
    }
}

pub fn resolve_database_path() -> Result<DbPathResolution> {
    resolve_database_path_with(&DbPathContext::from_environment())
}

pub fn resolve_database_path_with(context: &DbPathContext) -> Result<DbPathResolution> {
    if let Some(path) = &context.aglink_db {
        return Ok(DbPathResolution {
            path: path.clone(),
            reason: DbPathReason::ExplicitDatabaseEnv,
        });
    }

    if let Some(path) = &context.aglink_home {
        return Ok(DbPathResolution {
            path: path.join(DB_FILE_NAME),
            reason: DbPathReason::ExplicitHomeEnv,
        });
    }

    if let Some(executable_path) = &context.executable_path {
        if let Some(parent) = executable_path.parent() {
            let portable_path = parent.join(DB_FILE_NAME);
            if portable_path.is_file() {
                return Ok(DbPathResolution {
                    path: portable_path,
                    reason: DbPathReason::PortableDatabase,
                });
            }
        }
    }

    Ok(DbPathResolution {
        path: platform_default_path(context)?,
        reason: DbPathReason::PlatformDefault,
    })
}

pub fn migrate_default_database() -> Result<MigrationReport> {
    let resolution = resolve_database_path()?;
    migrate_database(&resolution)
}

pub fn migrate_database(resolution: &DbPathResolution) -> Result<MigrationReport> {
    let connection = open_writable_connection(&resolution.path)?;
    let previous_version = schema_version(&connection)?;
    migrate_connection(&connection, previous_version)?;
    seed_builtin_frameworks(&connection)?;
    let current_version = schema_version(&connection)?;

    Ok(MigrationReport {
        path: resolution.path.clone(),
        reason: resolution.reason,
        previous_version,
        current_version,
    })
}

pub fn check_default_database() -> Result<DbCheckReport> {
    let resolution = resolve_database_path()?;
    check_database(&resolution)
}

pub fn check_database(resolution: &DbPathResolution) -> Result<DbCheckReport> {
    if !resolution.path.exists() {
        return Ok(DbCheckReport {
            path: resolution.path.clone(),
            reason: resolution.reason,
            exists: false,
            writable: false,
            schema_version: None,
            latest_schema_version: LATEST_SCHEMA_VERSION,
            framework_count: None,
            mapping_count: None,
        });
    }

    let writable = open_writable_connection(&resolution.path).is_ok();
    let connection =
        Connection::open_with_flags(&resolution.path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let schema_version = schema_version(&connection)?;
    let (framework_count, mapping_count) = if schema_version >= LATEST_SCHEMA_VERSION {
        (
            Some(count_rows(&connection, CountTable::Frameworks)?),
            Some(count_rows(&connection, CountTable::FrameworkMappings)?),
        )
    } else {
        (None, None)
    };

    Ok(DbCheckReport {
        path: resolution.path.clone(),
        reason: resolution.reason,
        exists: true,
        writable,
        schema_version: Some(schema_version),
        latest_schema_version: LATEST_SCHEMA_VERSION,
        framework_count,
        mapping_count,
    })
}

pub fn list_config(resolution: &DbPathResolution) -> Result<Vec<ConfigEntry>> {
    let connection = open_migrated_connection(resolution)?;
    let mut statement = connection.prepare(
        r#"
        SELECT key, value, updated_at
        FROM config
        ORDER BY key
        "#,
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ConfigEntry {
            key: row.get(0)?,
            value: row.get(1)?,
            updated_at: row.get(2)?,
        })
    })?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries)
}

pub fn get_config(resolution: &DbPathResolution, key: &str) -> Result<Option<ConfigEntry>> {
    let connection = open_migrated_connection(resolution)?;
    connection
        .query_row(
            r#"
            SELECT key, value, updated_at
            FROM config
            WHERE key = ?1
            "#,
            params![key],
            |row| {
                Ok(ConfigEntry {
                    key: row.get(0)?,
                    value: row.get(1)?,
                    updated_at: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(Error::from)
}

pub fn set_config(resolution: &DbPathResolution, key: &str, value: &str) -> Result<ConfigEntry> {
    validate_config_key(key)?;
    let connection = open_migrated_connection(resolution)?;
    let now = timestamp();
    connection.execute(
        r#"
        INSERT INTO config (key, value, updated_at)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(key) DO UPDATE SET
            value = excluded.value,
            updated_at = excluded.updated_at
        "#,
        params![key, value, now],
    )?;
    get_config(resolution, key)?
        .ok_or_else(|| Error::database(format!("config `{key}` was not stored")))
}

pub fn unset_config(resolution: &DbPathResolution, key: &str) -> Result<ConfigUnsetReport> {
    validate_config_key(key)?;
    let connection = open_migrated_connection(resolution)?;
    let removed = connection.execute("DELETE FROM config WHERE key = ?1", params![key])? > 0;
    Ok(ConfigUnsetReport {
        key: key.to_string(),
        removed,
    })
}

pub fn backup_default_database(destination: Option<&Path>) -> Result<DbBackupReport> {
    let resolution = resolve_database_path()?;
    backup_database(&resolution, destination)
}

pub fn backup_database(
    resolution: &DbPathResolution,
    destination: Option<&Path>,
) -> Result<DbBackupReport> {
    migrate_database(resolution)?;
    let backup_path = destination.map_or_else(
        || default_backup_path(&resolution.path),
        |path| path.to_path_buf(),
    );

    if backup_path.exists() {
        return Err(Error::project(format!(
            "backup destination already exists: {}",
            backup_path.display()
        )));
    }

    if let Some(parent) = backup_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let bytes = fs::copy(&resolution.path, &backup_path)?;
    Ok(DbBackupReport {
        source_path: resolution.path.clone(),
        backup_path,
        bytes,
    })
}

pub fn open_migrated_default_connection() -> Result<Connection> {
    let resolution = resolve_database_path()?;
    open_migrated_connection(&resolution)
}

pub fn open_migrated_connection(resolution: &DbPathResolution) -> Result<Connection> {
    let connection = open_writable_connection(&resolution.path)?;
    let previous_version = schema_version(&connection)?;
    migrate_connection(&connection, previous_version)?;
    seed_builtin_frameworks(&connection)?;
    Ok(connection)
}

pub fn latest_schema_version() -> u32 {
    LATEST_SCHEMA_VERSION
}

fn open_writable_connection(path: &Path) -> Result<Connection> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    Connection::open(path).map_err(Error::from)
}

fn migrate_connection(connection: &Connection, current_version: u32) -> Result<()> {
    if current_version > LATEST_SCHEMA_VERSION {
        return Err(Error::database(format!(
            "database schema version {current_version} is newer than supported version {LATEST_SCHEMA_VERSION}"
        )));
    }

    if current_version < 1 {
        connection.execute_batch(
            r#"
            BEGIN;
            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS frameworks (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL,
                built_in INTEGER NOT NULL CHECK (built_in IN (0, 1)),
                enabled_by_default INTEGER NOT NULL CHECK (enabled_by_default IN (0, 1)),
                enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS framework_mappings (
                id TEXT PRIMARY KEY NOT NULL,
                framework_id TEXT NOT NULL,
                source_path TEXT NOT NULL,
                link_path TEXT NOT NULL,
                link_kind TEXT NOT NULL CHECK (link_kind IN ('file', 'directory')),
                required INTEGER NOT NULL CHECK (required IN (0, 1)),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (framework_id) REFERENCES frameworks(id) ON DELETE CASCADE
            );
            CREATE TABLE IF NOT EXISTS linkable_items (
                id TEXT PRIMARY KEY NOT NULL,
                kind TEXT NOT NULL CHECK (kind IN ('skill', 'resource')),
                name TEXT NOT NULL,
                alias TEXT,
                source_path TEXT NOT NULL,
                source_kind TEXT NOT NULL CHECK (source_kind IN ('file', 'directory')),
                target_dir TEXT,
                description TEXT,
                version TEXT,
                commit_hash TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_linkable_items_kind_name
                ON linkable_items(kind, name);
            CREATE TABLE IF NOT EXISTS groups (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS group_items (
                group_id TEXT NOT NULL,
                item_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (group_id, item_id),
                FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE,
                FOREIGN KEY (item_id) REFERENCES linkable_items(id) ON DELETE CASCADE
            );
            PRAGMA user_version = 1;
            COMMIT;
            "#,
        )?;
    }

    if current_version < 2 {
        connection.execute_batch(
            r#"
            BEGIN;
            ALTER TABLE linkable_items
                ADD COLUMN source_type TEXT NOT NULL DEFAULT 'local-path';
            ALTER TABLE linkable_items
                ADD COLUMN source_ownership TEXT NOT NULL DEFAULT 'external';
            ALTER TABLE linkable_items
                ADD COLUMN repo_url TEXT;
            ALTER TABLE linkable_items
                ADD COLUMN repo_commit TEXT;
            PRAGMA user_version = 2;
            COMMIT;
            "#,
        )?;
    }

    Ok(())
}

fn seed_builtin_frameworks(connection: &Connection) -> Result<()> {
    let now = timestamp();

    connection.execute(
        r#"
        INSERT INTO frameworks
            (id, name, display_name, built_in, enabled_by_default, enabled, created_at, updated_at)
        VALUES
            ('claude', 'claude', 'Claude', 1, 1, 1, ?1, ?1)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            display_name = excluded.display_name,
            built_in = excluded.built_in,
            enabled_by_default = excluded.enabled_by_default,
            updated_at = excluded.updated_at
        "#,
        params![now],
    )?;

    seed_mapping(
        connection,
        "init:claude:agents-md",
        "claude",
        "AGENTS.md",
        "CLAUDE.md",
        LinkKind::File,
        true,
        &now,
    )?;
    seed_mapping(
        connection,
        "init:claude:skills-dir",
        "claude",
        ".agents/skills",
        ".claude/skills",
        LinkKind::Directory,
        true,
        &now,
    )?;

    Ok(())
}

fn seed_mapping(
    connection: &Connection,
    id: &str,
    framework_id: &str,
    source_path: &str,
    link_path: &str,
    link_kind: LinkKind,
    required: bool,
    now: &str,
) -> Result<()> {
    connection.execute(
        r#"
        INSERT INTO framework_mappings
            (id, framework_id, source_path, link_path, link_kind, required, created_at, updated_at)
        VALUES
            (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
        ON CONFLICT(id) DO UPDATE SET
            framework_id = excluded.framework_id,
            source_path = excluded.source_path,
            link_path = excluded.link_path,
            link_kind = excluded.link_kind,
            required = excluded.required,
            updated_at = excluded.updated_at
        "#,
        params![
            id,
            framework_id,
            source_path,
            link_path,
            link_kind.to_string(),
            bool_to_i64(required),
            now,
        ],
    )?;
    Ok(())
}

fn schema_version(connection: &Connection) -> Result<u32> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    u32::try_from(version).map_err(|_| Error::database("database schema version is invalid"))
}

#[derive(Debug, Clone, Copy)]
enum CountTable {
    Frameworks,
    FrameworkMappings,
}

impl CountTable {
    const fn count_sql(self) -> &'static str {
        match self {
            CountTable::Frameworks => "SELECT COUNT(*) FROM frameworks",
            CountTable::FrameworkMappings => "SELECT COUNT(*) FROM framework_mappings",
        }
    }

    const fn name(self) -> &'static str {
        match self {
            CountTable::Frameworks => "frameworks",
            CountTable::FrameworkMappings => "framework_mappings",
        }
    }
}

fn count_rows(connection: &Connection, table: CountTable) -> Result<u32> {
    let count: i64 = connection.query_row(table.count_sql(), [], |row| row.get(0))?;
    u32::try_from(count)
        .map_err(|_| Error::database(format!("{} row count is invalid", table.name())))
}

fn platform_default_path(context: &DbPathContext) -> Result<PathBuf> {
    match context.platform {
        TargetPlatform::Windows => {
            let Some(appdata) = &context.appdata else {
                return Err(Error::database(
                    "APPDATA is required to resolve the Windows default database path",
                ));
            };
            Ok(appdata.join(DATA_DIR_NAME).join(DB_FILE_NAME))
        }
        TargetPlatform::Macos => {
            let Some(home) = &context.home else {
                return Err(Error::database(
                    "HOME is required to resolve the macOS default database path",
                ));
            };
            Ok(home
                .join("Library")
                .join("Application Support")
                .join(DATA_DIR_NAME)
                .join(DB_FILE_NAME))
        }
        TargetPlatform::Linux => {
            if let Some(xdg_data_home) = &context.xdg_data_home {
                return Ok(xdg_data_home.join(DATA_DIR_NAME).join(DB_FILE_NAME));
            }

            let Some(home) = &context.home else {
                return Err(Error::database(
                    "HOME is required to resolve the Linux default database path",
                ));
            };
            Ok(home
                .join(".local")
                .join("share")
                .join(DATA_DIR_NAME)
                .join(DB_FILE_NAME))
        }
    }
}

fn validate_config_key(key: &str) -> Result<()> {
    if key.trim().is_empty() {
        return Err(Error::invalid_arguments("config key must not be empty"));
    }
    Ok(())
}

fn default_backup_path(source_path: &Path) -> PathBuf {
    let parent = source_path.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!("agent-linker-backup-{}.db", timestamp_nanos()))
}

#[cfg(windows)]
fn current_platform() -> TargetPlatform {
    TargetPlatform::Windows
}

#[cfg(target_os = "macos")]
fn current_platform() -> TargetPlatform {
    TargetPlatform::Macos
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn current_platform() -> TargetPlatform {
    TargetPlatform::Linux
}

fn home_from_environment() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var_os("USERPROFILE")
            .or_else(|| env::var_os("HOME"))
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }

    #[cfg(not(windows))]
    {
        env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Error::database(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        backup_database, check_database, get_config, latest_schema_version, list_config,
        migrate_database, resolve_database_path_with, set_config, unset_config, DbPathContext,
        DbPathReason, DbPathResolution, TargetPlatform,
    };
    use crate::core::{
        framework::{disable_framework, enable_framework, list_frameworks},
        test_support::TestDir,
    };
    use std::{fs, path::Path};

    fn base_context(temp_dir: &Path) -> DbPathContext {
        DbPathContext {
            aglink_db: None,
            aglink_home: None,
            executable_path: Some(temp_dir.join("bin").join("aglink.exe")),
            appdata: Some(temp_dir.join("appdata")),
            home: Some(temp_dir.join("home")),
            xdg_data_home: Some(temp_dir.join("xdg")),
            platform: TargetPlatform::Linux,
        }
    }

    #[test]
    fn database_path_prefers_explicit_db_then_home_then_portable() {
        let temp_dir = TestDir::new("db-path-precedence");
        let portable_parent = temp_dir.path().join("bin");
        fs::create_dir(&portable_parent).unwrap();
        fs::write(portable_parent.join("agent-linker.db"), "").unwrap();

        let mut context = base_context(temp_dir.path());
        context.aglink_db = Some(temp_dir.path().join("explicit.db"));
        context.aglink_home = Some(temp_dir.path().join("aglink-home"));

        let explicit = resolve_database_path_with(&context).unwrap();
        assert_eq!(explicit.path, temp_dir.path().join("explicit.db"));
        assert_eq!(explicit.reason, DbPathReason::ExplicitDatabaseEnv);

        context.aglink_db = None;
        let home = resolve_database_path_with(&context).unwrap();
        assert_eq!(
            home.path,
            temp_dir.path().join("aglink-home").join("agent-linker.db")
        );
        assert_eq!(home.reason, DbPathReason::ExplicitHomeEnv);

        context.aglink_home = None;
        let portable = resolve_database_path_with(&context).unwrap();
        assert_eq!(portable.path, portable_parent.join("agent-linker.db"));
        assert_eq!(portable.reason, DbPathReason::PortableDatabase);
    }

    #[test]
    fn database_path_uses_platform_defaults() {
        let temp_dir = TestDir::new("db-path-default");
        let mut context = base_context(temp_dir.path());
        context.executable_path = None;

        context.platform = TargetPlatform::Windows;
        let windows = resolve_database_path_with(&context).unwrap();
        assert_eq!(
            windows.path,
            temp_dir
                .path()
                .join("appdata")
                .join("agent-linker")
                .join("agent-linker.db")
        );

        context.platform = TargetPlatform::Macos;
        let macos = resolve_database_path_with(&context).unwrap();
        assert_eq!(
            macos.path,
            temp_dir
                .path()
                .join("home")
                .join("Library")
                .join("Application Support")
                .join("agent-linker")
                .join("agent-linker.db")
        );

        context.platform = TargetPlatform::Linux;
        let linux = resolve_database_path_with(&context).unwrap();
        assert_eq!(
            linux.path,
            temp_dir
                .path()
                .join("xdg")
                .join("agent-linker")
                .join("agent-linker.db")
        );

        context.xdg_data_home = None;
        let linux_fallback = resolve_database_path_with(&context).unwrap();
        assert_eq!(
            linux_fallback.path,
            temp_dir
                .path()
                .join("home")
                .join(".local")
                .join("share")
                .join("agent-linker")
                .join("agent-linker.db")
        );
    }

    #[test]
    fn migration_seeds_claude_framework_and_mappings() {
        let temp_dir = TestDir::new("db-migrate");
        let resolution = DbPathResolution {
            path: temp_dir.path().join("store").join("agent-linker.db"),
            reason: DbPathReason::ExplicitDatabaseEnv,
        };

        let report = migrate_database(&resolution).unwrap();
        assert_eq!(report.previous_version, 0);
        assert_eq!(report.current_version, latest_schema_version());

        let check = check_database(&resolution).unwrap();
        assert!(check.is_ok());
        assert_eq!(check.framework_count, Some(1));
        assert_eq!(check.mapping_count, Some(2));

        let frameworks = list_frameworks(&resolution).unwrap();
        assert_eq!(frameworks.len(), 1);
        assert_eq!(frameworks[0].id, "claude");
        assert!(frameworks[0].enabled);
        assert_eq!(frameworks[0].mappings.len(), 2);
    }

    #[test]
    fn migration_seed_preserves_enabled_state() {
        let temp_dir = TestDir::new("db-seed-preserves-enabled");
        let resolution = DbPathResolution {
            path: temp_dir.path().join("agent-linker.db"),
            reason: DbPathReason::ExplicitDatabaseEnv,
        };

        migrate_database(&resolution).unwrap();
        disable_framework(&resolution, "claude").unwrap();
        migrate_database(&resolution).unwrap();
        assert!(!list_frameworks(&resolution).unwrap()[0].enabled);

        enable_framework(&resolution, "claude").unwrap();
        assert!(list_frameworks(&resolution).unwrap()[0].enabled);
    }

    #[test]
    fn config_round_trips_and_database_backup_does_not_overwrite() {
        let temp_dir = TestDir::new("db-config-backup");
        let resolution = DbPathResolution {
            path: temp_dir.path().join("agent-linker.db"),
            reason: DbPathReason::ExplicitDatabaseEnv,
        };
        migrate_database(&resolution).unwrap();

        let entry = set_config(&resolution, "output.mode", "quiet").unwrap();
        assert_eq!(entry.key, "output.mode");
        assert_eq!(entry.value, "quiet");
        assert_eq!(
            get_config(&resolution, "output.mode")
                .unwrap()
                .unwrap()
                .value,
            "quiet"
        );
        assert_eq!(list_config(&resolution).unwrap().len(), 1);

        let unset = unset_config(&resolution, "output.mode").unwrap();
        assert!(unset.removed);
        assert!(get_config(&resolution, "output.mode").unwrap().is_none());

        let backup_path = temp_dir.path().join("backup.db");
        let backup = backup_database(&resolution, Some(&backup_path)).unwrap();
        assert_eq!(backup.backup_path, backup_path);
        assert!(backup.bytes > 0);
        assert!(backup.backup_path.is_file());

        let overwrite = backup_database(&resolution, Some(&backup.backup_path)).unwrap_err();
        assert!(overwrite.to_string().contains("already exists"));
    }
}
