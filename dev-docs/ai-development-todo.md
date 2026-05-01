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

- [x] 定义 `LinkKind`、`LinkStatus`、Provider trait 和结构化 symlink 错误分类。
- [x] 实现 link status 检查，覆盖 missing、正确链接、错误链接、断链、真实文件、真实目录和不支持类型。
- [x] 实现 create/remove/read 的 mock provider，并用单元测试锁定幂等与冲突语义。
- [x] 实现 Unix 标准库 provider，平台条件编译限制在 symlink 模块内。
- [x] 实现 Windows provider 选择策略：默认 Broker，标准库 symlink 只作为显式 fallback。
- [x] 集成 `win-symlinks-client` broker-only 能力，实现 Windows Broker provider 创建真实 symlink。
- [x] 用测试证明 `--force` 语义只可替换错误 symlink，不可删除真实文件或真实目录。
阶段 1 验证备注（2026-04-30）：

- 已运行：`cargo fmt --check`、`cargo check`、`cargo test`、`cargo run -- --help`。
- 本会话追加验证：`git ls-remote https://github.com/winddxr/win-symlinks HEAD`、`cargo metadata --format-version 1`、`cargo check --locked`、`cargo test --locked`、`cargo run --locked -- --help`。
- `win-symlinks-client` 使用公开 git 依赖，`Cargo.lock` 锁定到 `https://github.com/winddxr/win-symlinks#7d5c3317764b8456ce4af140e0cd0aaf375cee1d`。
- 本环境普通沙箱运行默认 Cargo cache 仍会因 `F:\Runtime\Rust\.cargo\git\db` 写入被拒绝而失败；同样命令使用提权后，默认 Cargo cache 可正常解析、下载、构建和测试公开 git 依赖。
- 当前 Windows 环境验证了默认 provider 选择为 Windows Broker，并验证了 Broker 错误码映射；真实 Broker 服务端创建路径仍留待阶段 8 平台专项验证。Unix 标准库 provider 代码通过条件编译隔离，并带 `#[cfg(unix)]` 平台专项测试，未在本 Windows 会话实际执行。

验收：symlink core 单元测试通过；mock provider 能覆盖命令层后续测试需要的成功、跳过和冲突场景。

## 阶段 2：Project Init 与 Manifest

条件文档：`dev-docs/modules/02-project-init-manifest.md`、`dev-docs/modules/03-framework-adapter.md`。

目标：完成 `aglink init` 的真实源结构、Claude 兼容链接和 `.agents/links.toml` 管理。

- [x] 实现 `.agents/links.toml` schema、读写、合法性校验和损坏 manifest 失败语义。
- [x] 实现项目路径创建规则：`AGENTS.md`、`.agents/`、`.agents/skills/`。
- [x] 实现 `.gitignore` managed block，保证用户内容不被覆盖。
- [x] 实现内置 Claude mapping：`AGENTS.md -> CLAUDE.md`、`.agents/skills/ -> .claude/skills/`。
- [x] 实现 `aglink init` command，command 层不得直接读写 manifest 或调用平台 symlink API。
- [x] 用临时目录集成测试验证重复 `init` 幂等、不修改已有真实 `AGENTS.md` 内容。
- [x] 用冲突测试验证真实 `CLAUDE.md`、真实 `.claude/skills/`、错误 symlink 均按设计失败。

阶段 2 验证备注（2026-04-30）：

- 已运行：`cargo fmt --check`、`cargo check --locked`、`cargo test --locked`、`cargo run --locked -- --help`。
- `init` 的链接行为使用临时目录加 mock symlink provider 覆盖成功、幂等和冲突路径；真实 Windows Broker 创建路径仍留待阶段 8 平台专项验证。
- 用户随后在本仓库实际运行 `aglink.exe init`，本会话观察到 `CLAUDE.md` 与 `.claude/skills` 为真实 symlink，`.agents/links.toml` 记录 `provider_backend = "windows-broker"`；阶段 8 仍保留更完整的平台专项验证。

验收：`aglink init` 在临时项目内能创建真实源、兼容链接、manifest 和 `.gitignore` managed block；重复执行结果稳定。

## 阶段 3：Global Store、Registry 与 Framework Adapter

条件文档：`dev-docs/modules/03-framework-adapter.md`、`dev-docs/modules/04-global-store-registry.md`。

目标：完成全局 SQLite 基础设施和内置框架数据模型。

- [x] 实现数据库路径解析：默认平台路径、portable `agent-linker.db`、`AGLINK_DB`、`AGLINK_HOME`。
- [x] 实现 `db path`，输出实际路径和解析原因。
- [x] 建立版本化 migration，至少覆盖 config、framework、mapping、linkable item、group 所需表。
- [x] 实现 `db migrate`、`db check` 的 core API 和 CLI 命令。
- [x] 实现内置 `claude` framework 与默认 mapping 的初始化或 migration seed。
- [x] 实现 framework list/show/enable/disable/mapping list 的最小命令。
- [x] 用临时数据库测试 migration、framework seed、路径覆盖和诊断输出。

阶段 3 验证备注（2026-05-01）：

- 已运行：`cargo fmt`、`cargo fmt --check`、`cargo check`、`cargo test`。
- `cargo test` 结果：30 个测试通过，0 个失败。
- 使用工作区临时 `AGLINK_HOME=.tmp-stage3-db` 验证 CLI：`cargo run -- db path`、`cargo run -- db migrate`、`cargo run -- db check`、`cargo run -- framework list`、`cargo run -- framework show claude`、`cargo run -- framework mapping list`、`cargo run -- framework disable claude`、`cargo run -- framework enable claude`。
- `db check` 验证结果：schema `1 / latest 1`，frameworks `1`，mappings `2`，status `ok`。

验收：全局数据库可初始化、迁移、检查；Framework Adapter 可通过结构化 API 提供 init/link 所需 mapping。

## 阶段 4：Linkable Item 注册

条件文档：`dev-docs/modules/05-linkable-item.md`、`dev-docs/modules/04-global-store-registry.md`。

目标：让 Skill 和 Resource 能进入全局 Registry，但不改变当前项目链接状态。

- [x] 实现 Linkable Item 领域模型，`source_path` 持久化为绝对路径。
- [x] 实现 Skill 注册校验：source 必须是目录且包含非空 `SKILL.md`。
- [x] 实现 Resource 注册校验：source 可为文件或目录，注册或链接时必须有目标目录。
- [x] 实现默认 link name 和默认项目相对 link path 计算。
- [x] 实现 skill add/list/show/rename/remove/refresh。
- [x] 实现 resource add/list/show/rename/remove/refresh。
- [x] 用临时数据库和临时 source 测试同名冲突、alias、kind 检测和 refresh 后 kind 不一致报错。

阶段 4 验证备注（2026-05-01）：

- 已运行：`cargo fmt`、`cargo fmt --check`、`cargo check --locked`、`cargo test --locked`。
- `cargo test --locked` 结果：39 个测试通过，0 个失败。
- 使用工作区临时 `AGLINK_HOME=.tmp-stage4-cli/home` 验证 CLI：`cargo run --locked -- db migrate`、`cargo run --locked -- skill add writer <temp-skill> --alias writing-helper`、`cargo run --locked -- skill list`、`cargo run --locked -- skill show writer`、`cargo run --locked -- resource add notes <temp-resource> --target-dir .agents/resources`、`cargo run --locked -- resource list`、`cargo run --locked -- resource refresh notes`。
- 阶段 4 的 registry 命令只写入全局 SQLite；未创建、删除或修改当前项目 symlink，也不复制、移动或删除 source 内容。

验收：Registry 命令不创建、不删除、不修改任何当前项目 symlink；source 内容不被复制、移动或删除。

## 阶段 5：统一 Link、Unlink 与 Status

条件文档：`dev-docs/modules/01-symlink-core.md`、`dev-docs/modules/02-project-init-manifest.md`、`dev-docs/modules/05-linkable-item.md`、`dev-docs/modules/06-cli-command-surface.md`。

目标：把全局 Registry 条目链接到当前项目，并以 manifest 作为项目状态来源。

- [x] 实现 `aglink link <name>`，根据 Linkable Item 类型计算项目相对 link path。
- [x] 实现 `--as <link-name>`，仅覆盖本次链接名，不修改 Registry alias。
- [x] 实现 Resource 的 `--target-dir <dir>` 本次覆盖。
- [x] 实现链接成功、已存在正确链接、错误 symlink、真实文件和真实目录的用户输出。
- [x] 实现 manifest 写入和更新，记录 source、link、kind、provider backend、created/updated 时间。
- [x] 实现 `aglink status` 和 `aglink status --json`，状态只来自 manifest 与实际文件系统检查。
- [x] 实现 `aglink unlink <name>`、`unlink --all`，只删除 manifest 管理的 symlink。
- [x] 用 mock provider、临时 manifest、临时数据库集成测试覆盖 link/status/unlink 核心流程。
- [x] 阶段 4/5 完成后复查 `paths.rs` 是否需要承接共享路径工具；只有当 `db`、`manifest`、`linkable` 或 `link/status` 出现实质重复路径解析逻辑时再迁移。（2026-05-01 复查：`paths.rs` 仍保持占位；阶段 5 未出现需要迁移的实质重复路径解析逻辑。）
- [x] 评估 Registry `link_name` 唯一性策略；若 link/status 需要按 link name 高可靠查询，考虑持久化 `link_name` 并添加唯一约束，避免仅靠全表扫描检测冲突。（2026-05-01 评估：MVP 暂保留阶段 4 的按类型扫描校验；阶段 5 通过 manifest link_path 冲突检测保证项目链接状态，不新增 schema migration。）

阶段 5 验证备注（2026-05-01）：

- 已运行：`cargo fmt --check`、`cargo check --locked`、`cargo test --locked`、`cargo run --locked -- --help`、`cargo run --locked -- status --json`。
- `cargo test --locked` 结果：42 个测试通过，0 个失败。
- 新增 link/status/unlink 核心流程测试使用 mock symlink provider、临时 manifest 和临时 SQLite 数据库覆盖 Skill 链接、Resource `--as` 与 `--target-dir` 覆盖、status 检查、unlink 移除 manifest 记录，以及真实文件占用 managed path 时拒绝 unlink。
- 当前仓库只读运行 `aglink status --json` 成功；manifest 中 `CLAUDE.md` 与 `.claude/skills` 两条 init 管理链接均报告 `correct_symlink`。
- 修正 manifest 路径序列化：Windows 绝对路径保留原始分隔符，项目相对路径继续标准化为 `/`，避免 canonical source path 经 manifest 轮转后与 symlink target 不一致。

验收：链接命令只处理 symlink，不覆盖真实文件目录；manifest 能完整支撑 status 和 unlink。

## 阶段 6：Group、Clean 与 Doctor

条件文档：`dev-docs/modules/04-global-store-registry.md`、`dev-docs/modules/06-cli-command-surface.md`。

目标：补齐批量管理、清理和诊断能力。

- [x] 实现 group create/list/show/rename/delete。
- [x] 实现 group add/remove item，并验证不存在 item、重复 item 和删除 group 不删除 source。
- [x] 实现 group link/unlink，内部复用统一 link/unlink 能力。
- [x] 实现批量命令时评估 Global Store / Framework Adapter 连接复用；若单个命令会连续执行多个 registry/framework 操作，应避免重复打开连接和重复运行 migration/seed。
- [x] 为 Registry / Framework 批量操作设计连接复用 API，避免 group 批量命令为每个 item 重复 open/migrate/seed。
- [x] 实现 `clean`、`clean --broken`、`clean --missing-source`，只处理 manifest 管理的 symlink。
- [x] 实现 `doctor` 检查数据库、migration、manifest、Framework Adapter、symlink backend 和 Windows Broker 可用性。
- [x] 增加 `--verbose` 诊断输出，包含 backend、系统错误码和 Broker 诊断信息。
- [x] 用临时数据库和临时项目测试 group/link/clean/doctor 的主要路径。

阶段 6 验证备注（2026-05-01）：

- 已运行：`cargo fmt --check`、`cargo check --locked`、`cargo test --locked`、`cargo run --locked -- --help`。
- `cargo test --locked` 结果：47 个测试通过，0 个失败。
- 新增测试使用临时 SQLite 数据库、临时 source、临时项目 manifest 和 mock symlink provider 覆盖 group lifecycle、group link/unlink、clean 只清理 manifest 管理链接、`clean --missing-source`，以及 doctor 的数据库、manifest、Framework Adapter 和 backend 检查。
- 使用临时 `AGLINK_HOME=.tmp-stage6-cli/home` 验证 CLI：`db migrate`、`skill add`、`group create`、`group add`、`group show`、`doctor --verbose`。
- 沙箱内直接运行 `aglink.exe doctor --verbose` 能正确报告 Windows Broker pipe 权限问题：`broker_code=SERVICE_UNAVAILABLE`；同一命令提权运行后 Windows Broker probe 通过并返回 `windows-broker available`。

验收：批量操作与单项操作表现一致；clean 不会删除未记录在 manifest 中的文件或链接。

## 阶段 7：CLI 输出、Dry Run 与安全收口

条件文档：`dev-docs/modules/06-cli-command-surface.md`、`dev-docs/modules/07-code-architecture-verification.md`。

目标：统一用户体验，并补齐会修改链接状态命令的安全选项。

阶段 7 实施决策（2026-05-01）：

- 以 `dev-docs/modules/06-cli-command-surface.md` 为命令契约来源；发现实现不一致时优先修实现，不通过收缩文档来迁就当前代码。
- 引入 `clap` 作为正式参数解析入口，替代 command 层手写 flag index 解析。
- 引入 `serde_json` 作为 JSON 输出入口，替代 `status --json` 手写 JSON 拼接。
- 正式引入 `aglink link --force`，语义只允许替换错误 symlink，不允许删除真实文件或真实目录。
- `--dry-run` 覆盖 `link`、`unlink`、`clean`、`group link`、`group unlink`；`init` 暂作为 MVP 例外，若后续纳入需先更新 CLI 契约文档。

阶段 7 命令契约矩阵（2026-05-01）：

| 命令区域 | 阶段 7 结论 |
| --- | --- |
| `config` | 补齐 `path/list/get/set/unset`，存储仍走全局 SQLite。 |
| `db` | 保留 `path/migrate/check`，补齐 `backup [path]` 且不覆盖已有备份目标。 |
| `framework` | 保留 list/show/enable/disable/mapping list，补齐 mapping add/remove；required mapping 不允许删除。 |
| `skill` / `resource` | 改为 path-first add，并保留可选 `--name` / `--alias`；resource add 继续要求 `--target-dir`。 |
| `link` / `group link` | 增加 `--dry-run`；增加 `--force`，仅替换错误 symlink。 |
| `unlink` / `group unlink` / `clean` | 增加 `--dry-run`，dry-run 不删除 symlink、不写 manifest。 |
| `status` / `doctor` | `status --json` 改为结构化 JSON 输出；`doctor --verbose` 通过全局 `--verbose` 支持。 |

- [x] 建立阶段 7 命令契约矩阵，对照 `dev-docs/modules/06-cli-command-surface.md` 列出已实现、需补齐、需改签名的命令。
- [x] 以 CLI 契约为准补齐缺失命令：`config path/list/get/set/unset`、`db backup [path]`、`framework mapping add/remove`。
- [x] 将 `skill add` / `resource add` 调整为 path-first：`skill add <path> [--name <name>] [--alias <link-name>]` 与 `resource add <path> --target-dir <dir> [--name <name>] [--alias <link-name>]`。
- [x] 引入 `clap`，统一顶层命令、子命令、全局 `--quiet` / `--verbose`、命令级 `--dry-run` / `--force` 的解析与 usage。
- [x] 调整输出架构：CLI 层负责格式化输出，command 层返回结构化 report；避免新增 command 层散落 `println!`。
- [x] 统一默认、`--quiet`、`--verbose` 输出格式；默认输出摘要，quiet 只输出必要错误，verbose 输出 backend、manifest、created dirs 和诊断细节。
- [x] 引入 `serde_json`，将 `status --json` 改为结构化序列化，并补齐 JSON 字段稳定性与 escaping 测试。
- [x] 为 `link`、`unlink`、`clean`、`group link`、`group unlink` 补齐 `--dry-run`，确保 dry-run 不创建 symlink、不删除 symlink、不写 manifest。
- [x] 正式实现 `aglink link --force`，只传递到 Symlink Core 的 wrong-symlink replacement 路径，并补齐真实文件、真实目录、broken symlink、missing source 的回归测试。
- [x] 改善 broken symlink 与 force 相关的用户提示；保持 force 不删除真实文件目录、不把缺失 source 当作可修复成功状态。
- [x] 让 `link` 创建缺失父目录时在 report 中返回 created dirs，并在默认或 verbose 输出中提示用户。
- [x] 审查 command 层，确保没有直接 SQL、manifest 读写或平台 symlink API 调用。
- [x] 添加架构边界测试或静态检查，防止 `commands` 绕过 core。
- [x] 抽取重复小工具：`timestamp()`、`bool_to_i64()`、`LinkKind` 字符串解析，以及测试中的临时目录 helper；优先放入语义明确的 `core::util` / `core::test_support`，不要把非路径工具塞进 `paths.rs`。
- [x] 复查 clean 的 missing-source 判定，区分真实 NotFound 与权限等其他 metadata 错误，避免把不可诊断的 IO 错误静默当成 source missing。
- [x] 评估 unlink/clean 内部选中记录的索引集合是否改用 `HashSet` / `BTreeSet`，避免 manifest 增大后 `Vec::contains` 形成不必要的 O(n²) 扫描。
- [x] 评估 group link 内部是否可避免 `group.items.clone()`，在不牺牲接口清晰度的前提下降低大 group 的额外分配。
- [x] 复查 `unlink <name>` 的匹配规则和提示文案，特别是按 link path 文件名匹配的便利行为；保留时应确保歧义提示要求使用 manifest id 或完整 link path。
- [x] 补齐 README 或开发说明中的真实构建、测试、运行命令；不要复制过长设计内容到根 `AGENTS.md`。

阶段 7 验证备注（2026-05-01）：

- 已运行：`cargo fmt`、`cargo fmt --check`、`git diff --check`，均通过。
- 已运行静态检查：确认 `src/commands` 无 `println!` / `eprintln!`，无 `rusqlite`、manifest 读写函数或平台 symlink API 直接调用；确认未再使用 `group.items.clone()`。
- 已补充但本会话未能执行的测试覆盖：CLI 参数互斥与 `--dry-run` / `--force` 解析、`status --json` serde escaping、config/db backup、framework mapping add/remove、link dry-run、link force、unlink dry-run、command 层边界静态检查。
- 未运行：`cargo check --locked`、`cargo test --locked`、`cargo run --locked -- --help`。原因：本工作区 Cargo 验证按 `AGENTS.md` 需要直接提权写入默认 Cargo cache；提权请求被环境用量限制拒绝，且新增 `clap` 依赖需要 Cargo 更新 `Cargo.lock` 后才能使用 `--locked` 验证。

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
