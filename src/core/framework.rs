//! Agent framework adapter entry point.

use std::path::{Path, PathBuf};

use crate::core::symlink::LinkKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameworkDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub display_name: &'static str,
    pub built_in: bool,
    pub enabled_by_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameworkMapping {
    pub id: &'static str,
    pub framework_id: &'static str,
    pub framework_name: &'static str,
    pub item_id: &'static str,
    pub item_name: &'static str,
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
        id: "claude",
        name: "claude",
        display_name: "Claude",
        built_in: true,
        enabled_by_default: true,
    }
}

pub fn default_init_mappings() -> Vec<FrameworkMapping> {
    let claude = built_in_claude();

    vec![
        FrameworkMapping {
            id: "init:claude:agents-md",
            framework_id: claude.id,
            framework_name: claude.name,
            item_id: "project:agents-md",
            item_name: "AGENTS.md",
            source_path: PathBuf::from("AGENTS.md"),
            link_path: PathBuf::from("CLAUDE.md"),
            link_kind: LinkKind::File,
            required: true,
        },
        FrameworkMapping {
            id: "init:claude:skills-dir",
            framework_id: claude.id,
            framework_name: claude.name,
            item_id: "project:skills-dir",
            item_name: ".agents/skills",
            source_path: PathBuf::from(".agents").join("skills"),
            link_path: PathBuf::from(".claude").join("skills"),
            link_kind: LinkKind::Directory,
            required: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::default_init_mappings;
    use crate::core::symlink::LinkKind;
    use std::path::PathBuf;

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
}
