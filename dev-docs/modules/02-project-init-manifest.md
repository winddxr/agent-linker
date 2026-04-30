# Project Init & Manifest

## 设计目标

`aglink init` 在当前工作目录建立 Agent Linker 项目结构，并通过 manifest 记录该项目实际由 `aglink` 管理的 symlink 状态。

## 职责边界

- 创建或确认项目真实源结构。
- 创建框架兼容 symlink。
- 维护 `.agents/links.toml`。
- 维护 `.gitignore` 中的 aglink managed block。
- 保证重复执行幂等且不覆盖用户真实文件或目录。

## 架构约束

- `aglink init` 只作用于当前工作目录，不支持 `aglink init <path>`。
- MVP 阶段 `init` 不提供 `--dry-run`、`--force`、`--verbose`。
- `AGENTS.md` 与 `.agents/skills/` 必须是真实源。
- `CLAUDE.md` 与 `.claude/skills/` 等框架路径必须是兼容性 symlink。
- 不创建 `.agents/config.toml` 或其他项目级配置文件。

## 接口契约

`init` 至少创建或确认：

- `AGENTS.md`
- `.agents/`
- `.agents/skills/`
- `.agents/links.toml`

内置 Claude 映射：

- `AGENTS.md` -> `CLAUDE.md`
- `.agents/skills/` -> `.claude/skills/`

manifest 固定路径：

- `.agents/links.toml`

manifest 至少记录：

- `id`
- `scope`
- `framework name`
- `item id`
- `item name`
- `source path`
- `link path`
- `link kind`
- `provider backend`
- `created by command`
- `created at`
- `updated at`

## 关键决策

- manifest 是项目链接状态清单，不是项目级配置文件。
- manifest 只记录 `aglink` 创建或接管管理的 symlink。
- manifest 支持 `status`、`unlink`、`clean`、审计和恢复。
- manifest 已存在且合法时合并或更新；损坏时失败且不覆盖。
- `.gitignore` 由 managed block 管理，不覆盖用户手写内容。
- MVP 默认 ignore patterns 为 `.claude/`、`.agents/skills/`、`.agents/links.toml`。

## 冲突规则

- `AGENTS.md` 不存在时创建空文件；存在真实文件时保留；存在目录或 symlink 时报错。
- `.agents/` 不存在时创建；存在目录时保留；存在非目录时报错。
- `.agents/skills/` 不存在时创建真实目录；存在目录时保留；存在非目录时报错。
- 框架兼容 link 不存在时创建；已是正确 symlink 时成功；错误 symlink、真实文件或真实目录时报错。

## 验收口径

- 重复执行 `init` 不改变用户已有真实文件内容。
- 框架兼容链接全部记录到 manifest。
- `.gitignore` 中用户内容保持不变，managed block 可追加或更新。
- 全局数据库不可用或迁移后，项目内已管理链接仍可通过 manifest 识别。

