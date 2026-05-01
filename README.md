# Agent Linker

Agent Linker is a Rust CLI for managing agent instructions, skills, and resources through controlled symlinks. The CLI binary is `aglink`.

Architecture and implementation sequencing live under `dev-docs/`; start with `dev-docs/architecture.md`.

## Development Commands

Use the discovered Cargo commands for local verification:

```powershell
cargo fmt --check
cargo check --locked
cargo test --locked
cargo run --locked -- --help
```

This workspace uses a default Cargo cache outside the repo. If sandboxed Cargo cannot write to that cache, run the same commands with the workspace-approved escalation path described in `AGENTS.md`.
