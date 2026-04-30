# AGENTS.md

## Project Overview

Agent Linker is a Rust CLI project for managing Agent instructions, skills, and resources through controlled symlinks. The CLI command is `aglink`.

Correctness and safety of real files, symlink targets, and project link state are higher priority than convenience.

## Source Of Truth

- Use `dev-docs/architecture.md` as the architecture entry point and current source of truth.
- Treat `docs/architecture_decisions.md` as historical only; **do not read or use it** as a fact source.
- Do not read other local docs unless the user asks or a strict condition below applies.

## Current Stack And Commands

- Target implementation: Rust CLI, MVP single crate with internal modules.
- Current known executable name: `aglink`.
- No build/test command is defined in the current architecture docs. Before running build or tests, inspect actual project config and use discovered commands; do not invent scripts.

## Architecture Rules

- Keep dependency direction as `cli -> commands -> core`.
- `commands` must not directly write SQL, read/write `.agents/links.toml`, or call platform symlink APIs.
- All symlink behavior goes through Symlink Core; Windows defaults to the Broker backend.
- Global SQLite stores registry/config/framework/group data; `.agents/links.toml` stores only current project symlink state.
- MVP does not use `.agents/config.toml`, Rust workspace splitting, runtime plugins, or custom Linkable Item types.

## Safety Boundaries

- Do not overwrite user real files or real directories during link operations.
- `--force` may replace an incorrect symlink only; it must not delete real files or directories.
- Keep this file minimal. Move conditional detail to `dev-docs/` instead of expanding root context.

## Sandbox Notes
Use this section to record commands that should be run with escalation immediately in this workspace, without first attempting a non-escalated run.
- `git add` — direct escalation required; sandboxed execution consistently fails with Git index lock or permission errors.
- `git commit` — direct escalation required; sandboxed execution consistently fails with Git index lock or permission errors.

## Verification

- For documentation changes, verify new links and files exist and keep root docs navigational.
- For code changes, read only the relevant conditional docs below, then run available tests or report why they could not be run.
- Do not mark link behavior complete from code inspection alone; verify with mock providers, temporary manifests, temporary databases, or platform-specific tests as appropriate.

## Conditional Context

Read these only when the condition matches:

- Architecture overview or implementation sequencing -> `dev-docs/architecture.md`.
- AI staged development plan and checkable task progress -> `dev-docs/ai-development-todo.md`.
- Symlink lifecycle, provider abstraction, errors, conflict behavior, or Windows Broker -> `dev-docs/modules/01-symlink-core.md`.
- `aglink init`, `AGENTS.md`, `.agents/`, `CLAUDE.md`, `.gitignore`, or manifest behavior -> `dev-docs/modules/02-project-init-manifest.md`.
- Agent framework mappings or Claude compatibility links -> `dev-docs/modules/03-framework-adapter.md`.
- SQLite, config, registry persistence, groups, migrations, or database path resolution -> `dev-docs/modules/04-global-store-registry.md`.
- Skill, Resource, source ownership, aliases, target paths, or Linkable Item rules -> `dev-docs/modules/05-linkable-item.md`.
- CLI command shape, user-facing command behavior, output modes, or `doctor` -> `dev-docs/modules/06-cli-command-surface.md`.
- Code module boundaries, extension policy, or test strategy -> `dev-docs/modules/07-code-architecture-verification.md`.
- Editing this file or other harness instructions -> `dev-docs/howto-write-harness-agents-md.md`.
