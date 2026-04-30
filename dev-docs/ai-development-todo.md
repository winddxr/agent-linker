# AI Development TODO

本文件用于让 AI 分次、可验证地推进 Agent Linker 实现。每次开发会话应只认领一个小阶段，完成验证后再勾选任务。

## 使用规则

- 开始任务前先读根 `AGENTS.md`，再按任务条件读取对应模块文档。
- 每次会话优先完成一个最小闭环：实现、测试、文档或诊断只做同一阶段内的内容。
- 勾选任务前必须运行已发现的可用测试；如果测试命令不存在或无法运行，在任务旁记录原因。
- 不从代码检查直接判定链接行为完成；必须使用 mock provider、临时目录、临时 manifest、临时数据库或平台专项测试验证。
- 遇到架构冲突时，以 `dev-docs/architecture.md` 和对应模块文档为准，先更新 TODO 或设计文档，再继续实现。

## 阶段 0：项目骨架与命令发现

目标：建立 Rust 单 crate MVP 的最小可编译骨架，并明确后续 AI 会话使用的真实命令。

- [x] 检查仓库配置，确认是否已有 `Cargo.toml`、crate 名、bin 名和可用脚本；不 invent build/test 命令。（阶段 0 开始时未发现 `Cargo.toml` 或脚本；已创建 crate `agent_linker`，bin `aglink`。）
- [x] 若缺少 Rust 项目骨架，创建单 crate CLI 项目，bin 名为 `aglink`。
- [x] 建立模块边界：`cli -> commands -> core`，并创建 `core::{error, paths, symlink, manifest, db, registry, framework, linkable}` 的占位入口。
- [x] 添加最小 CLI 入口和 `aglink --help` 可运行验证。（实现已添加；本会话未取得实际运行结果，见验证备注。）
- [x] 记录实际验证命令到本阶段任务备注，供后续会话复用。

阶段 0 验证备注（2026-04-30）：

- 发现的基础验证命令：`cargo check`、`cargo test`、`cargo run -- --help`。
- 本会话普通沙箱运行 `cargo check`、`cargo test`、`cargo run -- --help` 均失败，错误为 `windows sandbox: setup refresh failed with status exit code: 1`。
- 本会话按规则尝试提权运行 Cargo 验证命令，但自动权限审核超时，未取得实际 Cargo 输出；后续会话应在可执行 shell 中复跑上述命令后再确认阶段 0 验收。

验收：`aglink --help` 能运行；项目能通过已发现的基础构建或检查命令。

## 阶段 1：Symlink Core

条件文档：`dev-docs/modules/01-symlink-core.md`、`dev-docs/modules/07-code-architecture-verification.md`。

目标：先把所有链接安全语义做成可测试 core 能力。

- [ ] 定义 `LinkKind`、`LinkStatus`、Provider trait 和结构化 symlink 错误分类。
- [ ] 实现 link status 检查，覆盖 missing、正确链接、错误链接、断链、真实文件、真实目录和不支持类型。
- [ ] 实现 create/remove/read 的 mock provider，并用单元测试锁定幂等与冲突语义。
- [ ] 实现 Unix 标准库 provider，平台条件编译限制在 symlink 模块内。
- [ ] 实现 Windows provider 选择策略：默认 Broker，标准库 symlink 只作为显式 fallback。
- [ ] 用测试证明 `--force` 语义只可替换错误 symlink，不可删除真实文件或真实目录。

验收：symlink core 单元测试通过；mock provider 能覆盖命令层后续测试需要的成功、跳过和冲突场景。

## 阶段 2：Project Init 与 Manifest

条件文档：`dev-docs/modules/02-project-init-manifest.md`、`dev-docs/modules/03-framework-adapter.md`。

目标：完成 `aglink init` 的真实源结构、Claude 兼容链接和 `.agents/links.toml` 管理。

- [ ] 实现 `.agents/links.toml` schema、读写、合法性校验和损坏 manifest 失败语义。
- [ ] 实现项目路径创建规则：`AGENTS.md`、`.agents/`、`.agents/skills/`。
- [ ] 实现 `.gitignore` managed block，保证用户内容不被覆盖。
- [ ] 实现内置 Claude mapping：`AGENTS.md -> CLAUDE.md`、`.agents/skills/ -> .claude/skills/`。
- [ ] 实现 `aglink init` command，command 层不得直接读写 manifest 或调用平台 symlink API。
- [ ] 用临时目录集成测试验证重复 `init` 幂等、不修改已有真实 `AGENTS.md` 内容。
- [ ] 用冲突测试验证真实 `CLAUDE.md`、真实 `.claude/skills/`、错误 symlink 均按设计失败。

验收：`aglink init` 在临时项目内能创建真实源、兼容链接、manifest 和 `.gitignore` managed block；重复执行结果稳定。

## 阶段 3：Global Store、Registry 与 Framework Adapter

条件文档：`dev-docs/modules/03-framework-adapter.md`、`dev-docs/modules/04-global-store-registry.md`。

目标：完成全局 SQLite 基础设施和内置框架数据模型。

- [ ] 实现数据库路径解析：默认平台路径、portable `agent-linker.db`、`AGLINK_DB`、`AGLINK_HOME`。
- [ ] 实现 `db path`，输出实际路径和解析原因。
- [ ] 建立版本化 migration，至少覆盖 config、framework、mapping、linkable item、group 所需表。
- [ ] 实现 `db migrate`、`db check` 的 core API 和 CLI 命令。
- [ ] 实现内置 `claude` framework 与默认 mapping 的初始化或 migration seed。
- [ ] 实现 framework list/show/enable/disable/mapping list 的最小命令。
- [ ] 用临时数据库测试 migration、framework seed、路径覆盖和诊断输出。

验收：全局数据库可初始化、迁移、检查；Framework Adapter 可通过结构化 API 提供 init/link 所需 mapping。

## 阶段 4：Linkable Item 注册

条件文档：`dev-docs/modules/05-linkable-item.md`、`dev-docs/modules/04-global-store-registry.md`。

目标：让 Skill 和 Resource 能进入全局 Registry，但不改变当前项目链接状态。

- [ ] 实现 Linkable Item 领域模型，`source_path` 持久化为绝对路径。
- [ ] 实现 Skill 注册校验：source 必须是目录且包含非空 `SKILL.md`。
- [ ] 实现 Resource 注册校验：source 可为文件或目录，注册或链接时必须有目标目录。
- [ ] 实现默认 link name 和默认项目相对 link path 计算。
- [ ] 实现 skill add/list/show/rename/remove/refresh。
- [ ] 实现 resource add/list/show/rename/remove/refresh。
- [ ] 用临时数据库和临时 source 测试同名冲突、alias、kind 检测和 refresh 后 kind 不一致报错。

验收：Registry 命令不创建、不删除、不修改任何当前项目 symlink；source 内容不被复制、移动或删除。

## 阶段 5：统一 Link、Unlink 与 Status

条件文档：`dev-docs/modules/01-symlink-core.md`、`dev-docs/modules/02-project-init-manifest.md`、`dev-docs/modules/05-linkable-item.md`、`dev-docs/modules/06-cli-command-surface.md`。

目标：把全局 Registry 条目链接到当前项目，并以 manifest 作为项目状态来源。

- [ ] 实现 `aglink link <name>`，根据 Linkable Item 类型计算项目相对 link path。
- [ ] 实现 `--as <link-name>`，仅覆盖本次链接名，不修改 Registry alias。
- [ ] 实现 Resource 的 `--target-dir <dir>` 本次覆盖。
- [ ] 实现链接成功、已存在正确链接、错误 symlink、真实文件和真实目录的用户输出。
- [ ] 实现 manifest 写入和更新，记录 source、link、kind、provider backend、created/updated 时间。
- [ ] 实现 `aglink status` 和 `aglink status --json`，状态只来自 manifest 与实际文件系统检查。
- [ ] 实现 `aglink unlink <name>`、`unlink --all`，只删除 manifest 管理的 symlink。
- [ ] 用 mock provider、临时 manifest、临时数据库集成测试覆盖 link/status/unlink 核心流程。

验收：链接命令只处理 symlink，不覆盖真实文件目录；manifest 能完整支撑 status 和 unlink。

## 阶段 6：Group、Clean 与 Doctor

条件文档：`dev-docs/modules/04-global-store-registry.md`、`dev-docs/modules/06-cli-command-surface.md`。

目标：补齐批量管理、清理和诊断能力。

- [ ] 实现 group create/list/show/rename/delete。
- [ ] 实现 group add/remove item，并验证不存在 item、重复 item 和删除 group 不删除 source。
- [ ] 实现 group link/unlink，内部复用统一 link/unlink 能力。
- [ ] 实现 `clean`、`clean --broken`、`clean --missing-source`，只处理 manifest 管理的 symlink。
- [ ] 实现 `doctor` 检查数据库、migration、manifest、Framework Adapter、symlink backend 和 Windows Broker 可用性。
- [ ] 增加 `--verbose` 诊断输出，包含 backend、系统错误码和 Broker 诊断信息。
- [ ] 用临时数据库和临时项目测试 group/link/clean/doctor 的主要路径。

验收：批量操作与单项操作表现一致；clean 不会删除未记录在 manifest 中的文件或链接。

## 阶段 7：CLI 输出、Dry Run 与安全收口

条件文档：`dev-docs/modules/06-cli-command-surface.md`、`dev-docs/modules/07-code-architecture-verification.md`。

目标：统一用户体验，并补齐会修改链接状态命令的安全选项。

- [ ] 审查所有顶层命令是否符合 `dev-docs/modules/06-cli-command-surface.md`。
- [ ] 为除 MVP 明确例外外的链接状态修改命令补齐 `--dry-run`。
- [ ] 统一默认、`--quiet`、`--verbose` 输出格式。
- [ ] 审查 command 层，确保没有直接 SQL、manifest 读写或平台 symlink API 调用。
- [ ] 添加架构边界测试或静态检查，防止 `commands` 绕过 core。
- [ ] 补齐 README 或开发说明中的真实构建、测试、运行命令；不要复制过长设计内容到根 `AGENTS.md`。

验收：命令表面稳定，架构边界可验证，用户输出可预测。

## 阶段 8：平台专项与发布准备

目标：从 MVP 可用推进到可发布状态。

- [ ] 在 Windows 环境验证 Broker 默认路径，不允许直接创建后自动 fallback。
- [ ] 在 Linux/macOS 或等价 CI 环境验证标准库 symlink provider。
- [ ] 验证数据库默认路径、portable 模式和环境变量覆盖在目标平台上的行为。
- [ ] 建立发布前检查清单：构建、测试、格式化、doctor、自测临时项目。
- [ ] 准备版本号、变更记录和安装说明。

验收：主要平台路径均经过实际或明确标注的专项验证；发布说明不夸大未验证能力。

## 每次 AI 会话推荐开场

```text
Read AGENTS.md, then open dev-docs/ai-development-todo.md.
Pick the first unchecked task in the current phase, read only the conditional module docs named for that phase, implement it, run discovered tests, and update the checkbox only after verification.
```
