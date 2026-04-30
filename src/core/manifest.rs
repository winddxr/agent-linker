//! Project `.agents/links.toml` manifest entry point.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::core::{
    error::{Error, Result},
    framework::{default_init_mappings, FrameworkMapping},
    symlink::{
        default_provider, ensure_symlink, CreateSymlinkOptions, CreateSymlinkOutcome, LinkKind,
        SymlinkBackend, SymlinkProvider,
    },
};

const MANIFEST_VERSION: u32 = 1;
const MANIFEST_PATH: &str = ".agents/links.toml";
const MANAGED_GITIGNORE_BEGIN: &str = "# BEGIN aglink managed";
const MANAGED_GITIGNORE_END: &str = "# END aglink managed";
const MANAGED_GITIGNORE_PATTERNS: &[&str] = &[".claude/", ".agents/skills/", ".agents/links.toml"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRecord {
    pub id: String,
    pub scope: String,
    pub framework_name: String,
    pub item_id: String,
    pub item_name: String,
    pub source_path: PathBuf,
    pub link_path: PathBuf,
    pub link_kind: LinkKind,
    pub provider_backend: SymlinkBackend,
    pub created_by_command: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub schema_version: u32,
    pub links: Vec<LinkRecord>,
}

impl Manifest {
    pub fn empty() -> Self {
        Self {
            schema_version: MANIFEST_VERSION,
            links: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != MANIFEST_VERSION {
            return Err(Error::manifest(format!(
                "unsupported manifest schema_version {}; expected {MANIFEST_VERSION}",
                self.schema_version
            )));
        }

        let mut seen_ids = BTreeSet::new();
        for link in &self.links {
            require_non_empty("id", &link.id)?;
            require_non_empty("scope", &link.scope)?;
            require_non_empty("framework_name", &link.framework_name)?;
            require_non_empty("item_id", &link.item_id)?;
            require_non_empty("item_name", &link.item_name)?;
            require_non_empty("created_by_command", &link.created_by_command)?;
            require_non_empty("created_at", &link.created_at)?;
            require_non_empty("updated_at", &link.updated_at)?;

            if link.source_path.as_os_str().is_empty() {
                return Err(Error::manifest(format!(
                    "manifest link `{}` has empty source_path",
                    link.id
                )));
            }

            if link.link_path.as_os_str().is_empty() {
                return Err(Error::manifest(format!(
                    "manifest link `{}` has empty link_path",
                    link.id
                )));
            }

            if !seen_ids.insert(link.id.clone()) {
                return Err(Error::manifest(format!(
                    "manifest contains duplicate link id `{}`",
                    link.id
                )));
            }
        }

        Ok(())
    }

    pub fn upsert(&mut self, incoming: LinkRecord) {
        if let Some(existing) = self.links.iter_mut().find(|link| link.id == incoming.id) {
            let created_at = existing.created_at.clone();
            *existing = incoming;
            existing.created_at = created_at;
            return;
        }

        self.links.push(incoming);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitProjectReport {
    pub project_root: PathBuf,
    pub manifest_path: PathBuf,
    pub gitignore_path: PathBuf,
    pub created_paths: Vec<PathBuf>,
    pub preserved_paths: Vec<PathBuf>,
    pub link_outcomes: Vec<InitLinkOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitLinkOutcome {
    pub id: String,
    pub source_path: PathBuf,
    pub link_path: PathBuf,
    pub outcome: CreateSymlinkOutcome,
}

pub fn init_current_project() -> Result<InitProjectReport> {
    let project_root = std::env::current_dir()?;
    let mut provider = default_provider();
    init_project_with_provider(&project_root, provider.as_mut())
}

pub fn init_project_with_provider(
    project_root: &Path,
    provider: &mut dyn SymlinkProvider,
) -> Result<InitProjectReport> {
    validate_project_root(project_root)?;

    let mut report = InitProjectReport {
        project_root: project_root.to_path_buf(),
        manifest_path: manifest_path(project_root),
        gitignore_path: project_root.join(".gitignore"),
        created_paths: Vec::new(),
        preserved_paths: Vec::new(),
        link_outcomes: Vec::new(),
    };

    ensure_real_file(project_root, Path::new("AGENTS.md"), &mut report)?;
    ensure_real_dir(project_root, Path::new(".agents"), &mut report)?;
    ensure_real_dir(
        project_root,
        &PathBuf::from(".agents").join("skills"),
        &mut report,
    )?;

    let mut manifest = load_manifest(project_root)?;

    ensure_framework_parent_dirs(project_root, &default_init_mappings(), &mut report)?;
    update_gitignore(project_root, &mut report)?;

    for mapping in default_init_mappings() {
        let source = mapping.source_in(project_root);
        let link = mapping.link_in(project_root);
        let outcome = ensure_symlink(
            provider,
            &source,
            &link,
            mapping.link_kind,
            CreateSymlinkOptions::new(),
        )?;

        report.link_outcomes.push(InitLinkOutcome {
            id: mapping.id.to_string(),
            source_path: mapping.source_path.clone(),
            link_path: mapping.link_path.clone(),
            outcome,
        });

        manifest.upsert(record_from_mapping(&mapping, provider.backend()));
    }

    manifest.validate()?;
    save_manifest(project_root, &manifest)?;

    Ok(report)
}

pub fn manifest_path(project_root: &Path) -> PathBuf {
    project_root.join(MANIFEST_PATH)
}

pub fn load_manifest(project_root: &Path) -> Result<Manifest> {
    let path = manifest_path(project_root);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Manifest::empty());
        }
        Err(error) => return Err(error.into()),
    };

    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(Error::manifest(format!(
            "manifest path must be a real file: {}",
            path.display()
        )));
    }

    parse_manifest(&fs::read_to_string(&path)?)
}

pub fn save_manifest(project_root: &Path, manifest: &Manifest) -> Result<()> {
    manifest.validate()?;

    let path = manifest_path(project_root);
    let Some(parent) = path.parent() else {
        return Err(Error::manifest("manifest path has no parent"));
    };

    fs::create_dir_all(parent)?;
    fs::write(path, serialize_manifest(manifest))?;
    Ok(())
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

fn ensure_framework_parent_dirs(
    project_root: &Path,
    mappings: &[FrameworkMapping],
    report: &mut InitProjectReport,
) -> Result<()> {
    let mut parents = BTreeSet::new();
    for mapping in mappings {
        if let Some(parent) = mapping.link_path.parent() {
            if !parent.as_os_str().is_empty() {
                parents.insert(parent.to_path_buf());
            }
        }
    }

    for parent in parents {
        ensure_real_dir(project_root, &parent, report)?;
    }

    Ok(())
}

fn ensure_real_file(
    project_root: &Path,
    relative_path: &Path,
    report: &mut InitProjectReport,
) -> Result<()> {
    let path = project_root.join(relative_path);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(Error::project(format!(
            "{} must be a real file, not a symlink",
            relative_path.display()
        ))),
        Ok(metadata) if metadata.is_file() => {
            report.preserved_paths.push(relative_path.to_path_buf());
            Ok(())
        }
        Ok(metadata) if metadata.is_dir() => Err(Error::project(format!(
            "{} must be a real file, found directory",
            relative_path.display()
        ))),
        Ok(_) => Err(Error::project(format!(
            "{} must be a regular file",
            relative_path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, "")?;
            report.created_paths.push(relative_path.to_path_buf());
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn ensure_real_dir(
    project_root: &Path,
    relative_path: &Path,
    report: &mut InitProjectReport,
) -> Result<()> {
    let path = project_root.join(relative_path);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(Error::project(format!(
            "{} must be a real directory, not a symlink",
            relative_path.display()
        ))),
        Ok(metadata) if metadata.is_dir() => {
            report.preserved_paths.push(relative_path.to_path_buf());
            Ok(())
        }
        Ok(metadata) if metadata.is_file() => Err(Error::project(format!(
            "{} must be a directory, found file",
            relative_path.display()
        ))),
        Ok(_) => Err(Error::project(format!(
            "{} must be a regular directory",
            relative_path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(&path)?;
            report.created_paths.push(relative_path.to_path_buf());
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn update_gitignore(project_root: &Path, report: &mut InitProjectReport) -> Result<()> {
    let path = project_root.join(".gitignore");
    let block = managed_gitignore_block();

    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(Error::project(
                ".gitignore must be a real file when it exists",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::write(&path, block)?;
            report.created_paths.push(PathBuf::from(".gitignore"));
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    }

    let original = fs::read_to_string(&path)?;
    let updated = replace_or_append_managed_block(&original, &block)?;
    if updated != original {
        fs::write(&path, updated)?;
    }
    report.preserved_paths.push(PathBuf::from(".gitignore"));
    Ok(())
}

fn managed_gitignore_block() -> String {
    let mut block = String::new();
    block.push_str(MANAGED_GITIGNORE_BEGIN);
    block.push('\n');
    for pattern in MANAGED_GITIGNORE_PATTERNS {
        block.push_str(pattern);
        block.push('\n');
    }
    block.push_str(MANAGED_GITIGNORE_END);
    block.push('\n');
    block
}

fn replace_or_append_managed_block(original: &str, block: &str) -> Result<String> {
    let begin = original.find(MANAGED_GITIGNORE_BEGIN);
    let end = original.find(MANAGED_GITIGNORE_END);

    match (begin, end) {
        (Some(begin), Some(end)) if begin <= end => {
            let after_end = end + MANAGED_GITIGNORE_END.len();
            let mut updated = String::new();
            updated.push_str(&original[..begin]);
            if !updated.is_empty() && !updated.ends_with('\n') {
                updated.push('\n');
            }
            updated.push_str(block);
            let remainder = original[after_end..].trim_start_matches(['\r', '\n']);
            if !remainder.is_empty() {
                updated.push_str(remainder);
                if !updated.ends_with('\n') {
                    updated.push('\n');
                }
            }
            Ok(updated)
        }
        (None, None) => {
            let mut updated = original.to_string();
            if !updated.is_empty() && !updated.ends_with('\n') {
                updated.push('\n');
            }
            if !updated.is_empty() {
                updated.push('\n');
            }
            updated.push_str(block);
            Ok(updated)
        }
        _ => Err(Error::project(
            ".gitignore contains a partial aglink managed block",
        )),
    }
}

fn record_from_mapping(mapping: &FrameworkMapping, backend: SymlinkBackend) -> LinkRecord {
    let now = timestamp();

    LinkRecord {
        id: mapping.id.to_string(),
        scope: "project".to_string(),
        framework_name: mapping.framework_name.to_string(),
        item_id: mapping.item_id.to_string(),
        item_name: mapping.item_name.to_string(),
        source_path: mapping.source_path.clone(),
        link_path: mapping.link_path.clone(),
        link_kind: mapping.link_kind,
        provider_backend: backend,
        created_by_command: "init".to_string(),
        created_at: now.clone(),
        updated_at: now,
    }
}

fn timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    format!("unix:{seconds}")
}

fn serialize_manifest(manifest: &Manifest) -> String {
    let mut output = format!("schema_version = {}\n", manifest.schema_version);

    for link in &manifest.links {
        output.push('\n');
        output.push_str("[[links]]\n");
        push_kv(&mut output, "id", &link.id);
        push_kv(&mut output, "scope", &link.scope);
        push_kv(&mut output, "framework_name", &link.framework_name);
        push_kv(&mut output, "item_id", &link.item_id);
        push_kv(&mut output, "item_name", &link.item_name);
        push_kv(
            &mut output,
            "source_path",
            &path_to_manifest(&link.source_path),
        );
        push_kv(&mut output, "link_path", &path_to_manifest(&link.link_path));
        push_kv(&mut output, "link_kind", &link.link_kind.to_string());
        push_kv(
            &mut output,
            "provider_backend",
            &link.provider_backend.to_string(),
        );
        push_kv(&mut output, "created_by_command", &link.created_by_command);
        push_kv(&mut output, "created_at", &link.created_at);
        push_kv(&mut output, "updated_at", &link.updated_at);
    }

    output
}

fn push_kv(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push_str(" = \"");
    output.push_str(&escape_manifest_string(value));
    output.push_str("\"\n");
}

fn parse_manifest(input: &str) -> Result<Manifest> {
    let mut schema_version = None;
    let mut links = Vec::new();
    let mut current = None;

    for (index, raw_line) in input.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line == "[[links]]" {
            if let Some(table) = current.take() {
                links.push(link_from_table(table)?);
            }
            current = Some(BTreeMap::new());
            continue;
        }

        let (key, value) = parse_key_value(line, line_number)?;
        if key == "schema_version" && current.is_none() {
            if schema_version.is_some() {
                return Err(Error::manifest("manifest schema_version is duplicated"));
            }
            schema_version = Some(value.parse::<u32>().map_err(|_| {
                Error::manifest(format!("invalid schema_version on line {line_number}"))
            })?);
            continue;
        }

        let Some(table) = current.as_mut() else {
            return Err(Error::manifest(format!(
                "manifest key `{key}` appears outside [[links]] on line {line_number}"
            )));
        };

        if table
            .insert(key, parse_string_value(&value, line_number)?)
            .is_some()
        {
            return Err(Error::manifest(format!(
                "manifest key is duplicated on line {line_number}"
            )));
        }
    }

    if let Some(table) = current {
        links.push(link_from_table(table)?);
    }

    let Some(schema_version) = schema_version else {
        return Err(Error::manifest("manifest is missing schema_version"));
    };

    let manifest = Manifest {
        schema_version,
        links,
    };
    manifest.validate()?;
    Ok(manifest)
}

fn parse_key_value(line: &str, line_number: usize) -> Result<(String, String)> {
    let Some((key, value)) = line.split_once('=') else {
        return Err(Error::manifest(format!(
            "invalid manifest line {line_number}: expected key = value"
        )));
    };

    let key = key.trim();
    if key.is_empty() {
        return Err(Error::manifest(format!(
            "invalid manifest line {line_number}: empty key"
        )));
    }

    Ok((key.to_string(), value.trim().to_string()))
}

fn parse_string_value(value: &str, line_number: usize) -> Result<String> {
    if !value.starts_with('"') || !value.ends_with('"') || value.len() < 2 {
        return Err(Error::manifest(format!(
            "invalid manifest string on line {line_number}"
        )));
    }

    unescape_manifest_string(&value[1..value.len() - 1], line_number)
}

fn link_from_table(mut table: BTreeMap<String, String>) -> Result<LinkRecord> {
    let id = required_field(&mut table, "id")?;
    let link = LinkRecord {
        id,
        scope: required_field(&mut table, "scope")?,
        framework_name: required_field(&mut table, "framework_name")?,
        item_id: required_field(&mut table, "item_id")?,
        item_name: required_field(&mut table, "item_name")?,
        source_path: PathBuf::from(required_field(&mut table, "source_path")?),
        link_path: PathBuf::from(required_field(&mut table, "link_path")?),
        link_kind: parse_link_kind(&required_field(&mut table, "link_kind")?)?,
        provider_backend: parse_backend(&required_field(&mut table, "provider_backend")?)?,
        created_by_command: required_field(&mut table, "created_by_command")?,
        created_at: required_field(&mut table, "created_at")?,
        updated_at: required_field(&mut table, "updated_at")?,
    };

    if let Some(extra) = table.keys().next() {
        return Err(Error::manifest(format!(
            "manifest link `{}` contains unknown key `{extra}`",
            link.id
        )));
    }

    Ok(link)
}

fn required_field(table: &mut BTreeMap<String, String>, key: &str) -> Result<String> {
    table
        .remove(key)
        .ok_or_else(|| Error::manifest(format!("manifest link is missing `{key}`")))
}

fn parse_link_kind(value: &str) -> Result<LinkKind> {
    match value {
        "file" => Ok(LinkKind::File),
        "directory" => Ok(LinkKind::Directory),
        other => Err(Error::manifest(format!("unknown link_kind `{other}`"))),
    }
}

fn parse_backend(value: &str) -> Result<SymlinkBackend> {
    match value {
        "std" => Ok(SymlinkBackend::Std),
        "windows-broker" => Ok(SymlinkBackend::WindowsBroker),
        "windows-std-fallback" => Ok(SymlinkBackend::WindowsStdFallback),
        "external-ln" => Ok(SymlinkBackend::ExternalLn),
        "mock" => Ok(SymlinkBackend::Mock),
        other => Err(Error::manifest(format!(
            "unknown provider_backend `{other}`"
        ))),
    }
}

fn require_non_empty(field: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        Err(Error::manifest(format!(
            "manifest field `{field}` is empty"
        )))
    } else {
        Ok(())
    }
}

fn path_to_manifest(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn escape_manifest_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn unescape_manifest_string(value: &str, line_number: usize) -> Result<String> {
    let mut output = String::new();
    let mut chars = value.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }

        let Some(escaped) = chars.next() else {
            return Err(Error::manifest(format!(
                "invalid escape at end of line {line_number}"
            )));
        };

        match escaped {
            '\\' => output.push('\\'),
            '"' => output.push('"'),
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            other => {
                return Err(Error::manifest(format!(
                    "unsupported escape `\\{other}` on line {line_number}"
                )));
            }
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{
        init_project_with_provider, load_manifest, managed_gitignore_block, parse_manifest,
        replace_or_append_managed_block, save_manifest, Manifest,
    };
    use crate::core::{
        error::Error,
        symlink::{LinkKind, MockEntry, MockSymlinkProvider},
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
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "agent-linker-init-test-{}-{unique}",
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

    fn seeded_provider(project_root: &Path) -> MockSymlinkProvider {
        let mut provider = MockSymlinkProvider::new();
        provider.add_dir(project_root);
        provider.add_file(project_root.join("AGENTS.md"));
        provider.add_dir(project_root.join(".agents"));
        provider.add_dir(project_root.join(".agents").join("skills"));
        provider.add_dir(project_root.join(".claude"));
        provider
    }

    #[test]
    fn manifest_round_trips_generated_schema() {
        let temp_dir = TestDir::new();
        let mut provider = seeded_provider(temp_dir.path());

        init_project_with_provider(temp_dir.path(), &mut provider).unwrap();
        let manifest = load_manifest(temp_dir.path()).unwrap();
        save_manifest(temp_dir.path(), &manifest).unwrap();
        let reparsed = load_manifest(temp_dir.path()).unwrap();

        assert_eq!(manifest, reparsed);
        assert_eq!(manifest.links.len(), 2);
    }

    #[test]
    fn damaged_manifest_fails_without_overwrite() {
        let temp_dir = TestDir::new();
        fs::create_dir(temp_dir.path().join(".agents")).unwrap();
        let manifest_path = temp_dir.path().join(".agents").join("links.toml");
        fs::write(&manifest_path, "not valid").unwrap();

        let mut provider = seeded_provider(temp_dir.path());
        let error = init_project_with_provider(temp_dir.path(), &mut provider).unwrap_err();

        assert!(matches!(error, Error::Manifest(_)));
        assert_eq!(fs::read_to_string(manifest_path).unwrap(), "not valid");
    }

    #[test]
    fn init_creates_project_files_links_manifest_and_gitignore() {
        let temp_dir = TestDir::new();
        let mut provider = seeded_provider(temp_dir.path());

        let report = init_project_with_provider(temp_dir.path(), &mut provider).unwrap();

        assert!(temp_dir.path().join("AGENTS.md").is_file());
        assert!(temp_dir.path().join(".agents").is_dir());
        assert!(temp_dir.path().join(".agents").join("skills").is_dir());
        assert!(temp_dir.path().join(".claude").is_dir());
        assert!(temp_dir.path().join(".agents").join("links.toml").is_file());
        assert_eq!(report.link_outcomes.len(), 2);
        assert!(matches!(
            provider.entry(&temp_dir.path().join("CLAUDE.md")),
            Some(MockEntry::Symlink {
                kind: LinkKind::File,
                ..
            })
        ));
        assert!(matches!(
            provider.entry(&temp_dir.path().join(".claude").join("skills")),
            Some(MockEntry::Symlink {
                kind: LinkKind::Directory,
                ..
            })
        ));

        let gitignore = fs::read_to_string(temp_dir.path().join(".gitignore")).unwrap();
        assert_eq!(gitignore, managed_gitignore_block());

        let manifest = load_manifest(temp_dir.path()).unwrap();
        assert_eq!(manifest.links.len(), 2);
        assert!(manifest.links.iter().any(|link| {
            link.framework_name == "claude"
                && link.source_path == PathBuf::from("AGENTS.md")
                && link.link_path == PathBuf::from("CLAUDE.md")
        }));
    }

    #[test]
    fn repeated_init_is_idempotent_and_preserves_agents_content() {
        let temp_dir = TestDir::new();
        fs::write(temp_dir.path().join("AGENTS.md"), "user content\n").unwrap();
        fs::write(temp_dir.path().join(".gitignore"), "target/\n").unwrap();

        let mut provider = seeded_provider(temp_dir.path());
        init_project_with_provider(temp_dir.path(), &mut provider).unwrap();
        let first_manifest =
            fs::read_to_string(temp_dir.path().join(".agents").join("links.toml")).unwrap();

        let mut provider = provider;
        init_project_with_provider(temp_dir.path(), &mut provider).unwrap();

        assert_eq!(
            fs::read_to_string(temp_dir.path().join("AGENTS.md")).unwrap(),
            "user content\n"
        );
        let gitignore = fs::read_to_string(temp_dir.path().join(".gitignore")).unwrap();
        assert!(gitignore.starts_with("target/\n"));
        assert_eq!(gitignore.matches("# BEGIN aglink managed").count(), 1);
        assert!(
            fs::read_to_string(temp_dir.path().join(".agents").join("links.toml"))
                .unwrap()
                .contains("init:claude:agents-md")
        );
        assert!(parse_manifest(&first_manifest).is_ok());
    }

    #[test]
    fn init_rejects_real_file_at_claude_link() {
        let temp_dir = TestDir::new();
        let mut provider = seeded_provider(temp_dir.path());
        provider.add_file(temp_dir.path().join("CLAUDE.md"));

        let error = init_project_with_provider(temp_dir.path(), &mut provider).unwrap_err();

        assert!(matches!(error, Error::Symlink(_)));
    }

    #[test]
    fn init_rejects_real_directory_at_claude_skills_link() {
        let temp_dir = TestDir::new();
        let mut provider = seeded_provider(temp_dir.path());
        provider.add_dir(temp_dir.path().join(".claude").join("skills"));

        let error = init_project_with_provider(temp_dir.path(), &mut provider).unwrap_err();

        assert!(matches!(error, Error::Symlink(_)));
    }

    #[test]
    fn init_rejects_wrong_symlink_target() {
        let temp_dir = TestDir::new();
        let mut provider = seeded_provider(temp_dir.path());
        provider.add_file(temp_dir.path().join("OTHER.md"));
        provider.add_symlink(
            temp_dir.path().join("CLAUDE.md"),
            temp_dir.path().join("OTHER.md"),
            LinkKind::File,
        );

        let error = init_project_with_provider(temp_dir.path(), &mut provider).unwrap_err();

        assert!(matches!(error, Error::Symlink(_)));
    }

    #[test]
    fn gitignore_managed_block_preserves_user_content() {
        let original = "target/\n\n# BEGIN aglink managed\nold\n# END aglink managed\nlogs/\n";
        let updated =
            replace_or_append_managed_block(original, &managed_gitignore_block()).unwrap();

        assert!(updated.starts_with("target/\n\n"));
        assert!(updated.contains(".claude/"));
        assert!(updated.ends_with("logs/\n"));
        assert_eq!(updated.matches("# BEGIN aglink managed").count(), 1);
    }

    #[test]
    fn existing_empty_manifest_is_invalid() {
        let error = parse_manifest("").unwrap_err();

        assert!(matches!(error, Error::Manifest(_)));
        assert_eq!(Manifest::empty().links.len(), 0);
    }
}
