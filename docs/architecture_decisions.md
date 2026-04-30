# Agent Linker - 架构设计与决策记录

> 本文档用于集中汇总开发前讨论（`discussion_checklist.md`）中已确认的正式结论。
> 它是指导项目代码架构、命令行设计和底层逻辑的官方准则。

---

## 1. 核心术语与基础规范

### 1.1 核心术语
* **Symlink（符号链接）**：操作系统级别的符号链接（非快捷方式，非硬链接）。
* **Agent Skill（技能）**：一个按用途命名的目录，包含 AI Agent 可使用的提示词/脚本/文档等资源。
* **Linkable Item（可链接项）**：Skill 和 Resource（资源）的统称，是本工具管理的基本操作单元。

### 1.2 Symlink 的方向性准则
* **真实存储**：本项目所创建和管理的 `AGENTS.md` 文件及 `.agents/skills/` 目录属于**真实存在的文件/目录**。
* **兼容性链接**：为适配不同的 Agent 框架（如 Claude），自动生成的 `CLAUDE.md` 及 `.claude/skills/` 均为**指向上述真实文件/目录的 Symlink**。

---

## 2. 第一部分：项目级决策

### 2.1 项目与命令名称 (Q1)
* **项目代码工程名**：`agent_linker`
* **CLI 可执行文件（命令）名**：`aglink`
  * **设计考量**："Agent Link" 的组合词，简短且极具专属性，不会与系统中现有的常规链接工具混淆。
  * **使用示例**：`aglink init`, `aglink skill add` 等。

### 2.2 跨平台 Symlink 策略与链接生命周期 (Q2)

#### 2.2.1 总体结论
* 本项目内部**不以外部 `ln` 命令作为跨平台 symlink 的主实现**。
* Linux / macOS 使用 Rust 标准库直接创建 symlink：
  * `std::os::unix::fs::symlink`
* Windows 使用现有 `WinSymlinksBroker` 的 IPC 能力作为默认实现：
  * `aglink` → Broker client → Named Pipe → `WinSymlinksBroker` → `CreateSymbolicLinkW`
* Windows 下已有的 `ln.exe` 作为独立命令、人工调试工具或兼容入口保留，但不作为 `agent-linker` 核心逻辑的默认调用路径。
* Windows Rust 标准库的 `std::os::windows::fs::symlink_file` / `symlink_dir` 仅作为可选 fallback，用于管理员权限、Developer Mode 或测试环境；不能作为 Windows 默认路径。

#### 2.2.2 设计理由
* 调用外部 `ln` 只能统一命令入口，不能统一行为语义。
* 外部命令会引入额外的不稳定因素：
  * GNU `ln` 与 BSD `ln` 的细节差异。
  * `PATH` 查找、版本匹配、子进程启动失败。
  * shell quoting、非 UTF-8 输出、stderr 文本解析。
  * Windows 下 `ln.exe`、Broker 服务、Named Pipe 三者状态需要额外诊断。
* 核心库应依赖结构化 API / IPC，而不是依赖命令行文本协议。
* 真正的跨平台一致性应由项目自己的抽象层和错误模型保证。

#### 2.2.3 Core 抽象
Symlink 创建、删除、读取和验证属于 Core 能力，所有 Stage 统一通过同一套抽象访问。

建议核心接口形态：

```rust
trait SymlinkProvider {
    fn create_symlink(&self, source: &Path, link: &Path, kind: LinkKind) -> Result<()>;
    fn remove_symlink(&self, link: &Path) -> Result<()>;
    fn read_link(&self, link: &Path) -> Result<PathBuf>;
    fn link_status(&self, source: &Path, link: &Path, kind: LinkKind) -> Result<LinkStatus>;
}
```

核心枚举：

```rust
enum LinkKind {
    File,
    Directory,
}

enum LinkStatus {
    Missing,
    CorrectSymlink,
    WrongSymlinkTarget,
    BrokenSymlink,
    ExistingRealFile,
    ExistingRealDirectory,
    UnsupportedFileType,
}
```

平台实现：

```text
UnixSymlinkProvider
  -> std::os::unix::fs::symlink

WindowsBrokerSymlinkProvider
  -> Named Pipe client -> WinSymlinksBroker

WindowsStdSymlinkProvider
  -> std::os::windows::fs::symlink_file / symlink_dir
  -> optional fallback only

ExternalLnSymlinkProvider
  -> optional debug / compatibility backend only
```

#### 2.2.4 幂等性与冲突策略
所有链接创建操作必须设计为幂等。

* `link` 路径不存在：创建 symlink。
* `link` 已存在且是指向同一 `source` 的 symlink：视为成功，不重复创建。
* `link` 已存在但指向其他目标：默认报错；允许显式 `--force` 删除该 symlink 后重建。
* `link` 已存在且是真实文件或真实目录：默认报错；`--force` 不应删除真实文件/目录。后续如确需支持，应设计比 `--force` 更明确的危险操作开关。
* `link` 是断链：按其记录的目标判断。若目标与期望一致但 `source` 不存在，报告 `source` 缺失；若目标不一致，按错误 symlink 处理。
* `source` 不存在：不创建链接，报告源路径不存在。
* `link` 的父目录不存在：默认报错；是否自动创建父目录由调用方场景决定，例如 `init` 可以创建基础目录，普通 `link resource` 不应隐式创建过深目录。

#### 2.2.5 Windows Broker 策略
* Windows 默认依赖 `WinSymlinksBroker` 服务完成 symlink 创建。
* Broker client 应在 Core 内封装，不通过 `ln.exe` 间接调用。
* Broker 未安装、未运行、协议版本不兼容、Named Pipe 连接失败时，应返回结构化错误。
* 用户错误信息应明确说明：
  * 当前使用的是 Windows Broker 后端。
  * Broker 当前不可用的具体原因。
  * 可采取的修复动作，例如安装服务、启动服务、检查权限或切换 fallback 后端。
* fallback 后端必须显式配置或由命令参数启用，避免在 Windows 上静默退回到权限不稳定的实现。

#### 2.2.6 Link Manifest
目标项目中需要维护由 `agent-linker` 管理的 link manifest，用于状态检查、幂等判断、清理和审计。

* manifest 只记录 `agent-linker` 创建或接管管理的链接。
* 固定路径：`.agents/links.toml`。
* manifest 是项目链接状态清单，不是项目级配置文件。
* manifest 至少记录：
  * link path
  * source path
  * link kind: file / directory
  * linkable item type: init / skill / resource
  * created by command
  * created at
  * provider backend

#### 2.2.7 错误模型与用户反馈
Core 需要定义统一的 symlink 错误类型，不能把平台原始错误直接作为用户主信息。

建议错误分类：

```text
SourceNotFound
LinkParentNotFound
LinkAlreadyExists
WrongSymlinkTarget
ExistingRealFile
ExistingRealDirectory
PermissionDenied
BrokerUnavailable
BrokerProtocolError
UnsupportedPlatform
UnsupportedLinkKind
Io
```

CLI 层负责把结构化错误转换成可执行的用户提示。

* 默认输出应简洁，说明失败路径和原因。
* `--verbose` 输出后端、系统错误码、Broker 连接细节等诊断信息。
* `--quiet` 只输出必要错误。
* 成功时输出操作摘要，例如已创建、已存在且跳过、已更新。
* 除已明确的 MVP 例外外，会修改链接状态的命令应支持 `--dry-run`，用于预览创建、跳过、覆盖和失败项。
* `aglink init` 在 MVP 阶段暂不提供 `--dry-run`、`--force`、`--verbose`。

#### 2.2.8 对后续 Stage 的影响
* `init`、Skill 链接、Resource 链接都必须复用同一个 symlink Core。
* Stage 1 的 `CLAUDE.md`、`.claude/skills` 兼容链接走同一套 `SymlinkProvider`。
* Stage 2 / Stage 3 的 Skill 与 Resource 链接行为在底层统一为 Linkable Item → Symlink 操作。
* Stage 2 / Stage 3 的命令形态和数据模型仍可继续单独讨论，但不得绕过 Core symlink 抽象。

### 2.3 配置体系与 Agent 框架适配策略 (Q-A)

#### 2.3.1 总体结论
* 统一以 `AGENTS.md` 和 `.agents/` 作为真实源。
* Agent 框架特定文件和目录只作为兼容性投射，不作为真实内容源。
* 项目预留多框架适配能力，通过 Framework Adapter 管理框架映射。
* 全局配置、Registry、框架适配、Skill / Resource 等结构化数据统一存储在 SQLite 数据库中。
* MVP 阶段不引入项目级配置文件，例如不创建 `.agents/config.toml`。
* 项目内保留 `.agents/links.toml` 作为 link manifest，用于记录该项目实际由 `aglink` 创建或接管管理的 symlink。

#### 2.3.2 Framework Adapter
框架适配采用“真实源 → 框架约定路径”的映射模型。

内置 Claude 映射：

```text
AGENTS.md        -> CLAUDE.md
.agents/skills/  -> .claude/skills/
```

后续框架通过相同模型扩展。框架定义应支持内置项和用户自定义项。

建议数据结构：

```text
frameworks
  id
  name
  display_name
  built_in
  enabled_by_default
  created_at
  updated_at

framework_mappings
  id
  framework_id
  source_path
  link_path
  link_kind: file / directory
  required
```

#### 2.3.3 全局 SQLite 数据库
全局 SQLite 是本项目的主要配置与 Registry 存储。它负责保存：

* Framework Adapter 与映射规则。
* Skill / Resource / Linkable Item Registry。
* Group 定义。
* 来源路径、来源类型、版本、commit、描述等元数据。
* 用户级偏好设置。

默认数据库路径遵循平台用户数据目录：

```text
Windows:
  %APPDATA%\agent-linker\agent-linker.db

macOS:
  ~/Library/Application Support/agent-linker/agent-linker.db

Linux:
  $XDG_DATA_HOME/agent-linker/agent-linker.db
  fallback: ~/.local/share/agent-linker/agent-linker.db
```

同时支持 portable 模式：

* 当可执行文件同目录存在 `agent-linker.db` 时，可使用该数据库作为 portable 数据库。
* 允许通过 `AGLINK_DB` 显式指定数据库路径。
* 允许通过 `AGLINK_HOME` 显式指定 agent-linker 的数据根目录。
* portable 模式必须可诊断，例如 `aglink db path` 应能显示当前实际使用的数据库路径和解析原因。

Windows 不默认把数据库写到 exe 同目录，因为 exe 可能位于 `Program Files`、包管理器目录或其他普通用户不可写位置。

#### 2.3.4 项目级配置文件
MVP 阶段不创建项目级配置文件。

不采用 `.agents/config.toml` 的原因：

* 当前项目状态可由全局 SQLite 和项目 link manifest 共同表达。
* 过早引入项目配置会增加配置合并、覆盖优先级、提交策略和迁移成本。
* 团队共享的“期望配置”不是 MVP 必需能力，可在后续需求明确后单独设计。

项目内只保留 `.agents/links.toml` 作为状态清单。它不表达用户偏好或 Registry 信息。

#### 2.3.5 Link Manifest
`.agents/links.toml` 是必须的。它用于支持：

* `status`：判断链接是否存在、是否断链、是否仍指向正确源。
* `unlink`：只移除由 `aglink` 管理的链接。
* `clean`：安全清理 manifest 中记录的 symlink。
* 审计：判断某个项目当前启用了哪些 framework / skill / resource 链接。
* 恢复：当全局数据库不可用或已迁移时，仍能识别项目内已管理链接。

manifest 至少记录：

```text
id
scope: framework / skill / resource / init
framework name, when scope = framework
item id, when scope = skill/resource and known
item name
source path
link path
link kind: file / directory
provider backend
created by command
created at
updated at
```

manifest 只允许用于记录 symlink 状态，不应成为项目级配置文件的替代品。

#### 2.3.6 CLI 边界
配置、数据库和框架适配都通过同一个 `aglink` 可执行文件管理，不拆分独立配置命令。

建议命令边界：

```text
aglink config ...
  管理基础用户配置和显示配置解析结果

aglink db ...
  管理数据库路径、迁移、备份、诊断

aglink framework ...
  管理框架适配、启用/禁用框架、查看映射
```

示例命令：

```text
aglink config path
aglink config get <key>
aglink config set <key> <value>

aglink db path
aglink db migrate
aglink db backup

aglink framework list
aglink framework enable claude
aglink framework disable claude
aglink framework mapping add <framework> <source> <link>
```

`aglink init` 可提供框架快捷参数：

```text
aglink init --framework claude
aglink init --framework claude,cursor
aglink init --framework all
```

### 2.4 Init 命令的目标结构与幂等行为 (Q-B)

#### 2.4.1 总体结论
* `aglink init` 只作用于当前工作目录。
* 不支持 `aglink init <path>`。
* `init` 必须幂等，重复执行不得覆盖现有真实文件或真实目录。
* `AGENTS.md` 不存在时创建空文件。
* `init` 自动追加更新 `.gitignore`，但不得覆盖用户已有内容。
* `init` 在 MVP 阶段暂不提供 `--dry-run`、`--force`、`--verbose`。

#### 2.4.2 初始化创建内容
`aglink init` 至少创建或确认以下结构：

```text
AGENTS.md
.agents/
.agents/skills/
.agents/links.toml
```

并根据启用的 Framework Adapter 创建框架兼容 symlink，例如：

```text
CLAUDE.md        -> AGENTS.md
.claude/skills/  -> .agents/skills/
```

默认启用哪些 framework 由全局 SQLite 中的 `enabled_by_default` 决定。MVP 内置 `claude` 并默认启用。

#### 2.4.3 文件与目录冲突规则
`AGENTS.md`：

* 不存在：创建空文件。
* 已存在且是真实文件：保留。
* 已存在且是目录：报错。
* 已存在且是 symlink：报错，因为 `AGENTS.md` 必须是真实源文件。

`.agents/`：

* 不存在：创建目录。
* 已存在且是目录：保留。
* 已存在但不是目录：报错。

`.agents/skills/`：

* 不存在：创建真实目录。
* 已存在且是目录：保留。
* 已存在但不是目录：报错。

框架兼容 symlink，例如 `CLAUDE.md` 和 `.claude/skills/`：

* link 路径不存在：创建 symlink。
* link 已是正确 symlink：视为成功。
* link 已是错误 symlink：报错，提示用户先 `unlink` / `clean` 或手动处理。
* link 是真实文件或真实目录：报错，不覆盖。

#### 2.4.4 `.gitignore` 策略
`init` 自动维护 `.gitignore` 的 aglink managed block。

* 若 `.gitignore` 不存在，创建文件。
* 若 `.gitignore` 已存在，追加或更新 aglink managed block。
* 不覆盖用户手写内容。
* managed block 的具体条目来自全局 SQLite 配置。

MVP 内置默认 ignore patterns：

```gitignore
# BEGIN aglink managed
.claude/
.agents/skills/
.agents/links.toml
# END aglink managed
```

`.claude/` 在 MVP 阶段整体忽略。后续如需要更细粒度提交 Claude 自有配置，可通过全局配置调整 ignore patterns。

#### 2.4.5 Manifest 规则
`init` 必须写入 `.agents/links.toml`。

* framework symlink 必须记录到 manifest。
* manifest 已存在且格式合法：合并或更新 aglink 管理的记录。
* manifest 已存在但格式损坏：报错，不覆盖。
* manifest 不存在：创建。

`init` 不创建 `.agents/config.toml` 或其他项目级配置文件。

### 2.5 Linkable Item、Registry 与来源管理模型 (Q-C)

#### 2.5.1 总体结论
* Skill 和 Resource 统一建模为 **Linkable Item（可链接项）**。
* Registry 统一存储所有 Linkable Item。
* Registry 存储在全局 SQLite 数据库中。
* MVP 采用分散管理模式：`aglink` 只记录源路径，不复制、不移动、不接管源内容。
* Linkable Item 的 `source_path` 永远存绝对路径。
* 当前项目内的 `link_path` 使用相对项目根目录的路径。
* 同名不同源默认报错，提示用户改名或指定 alias。

#### 2.5.2 Skill 与 Resource 的差异
Skill 和 Resource 共用 Registry，但业务约束不同。

Skill：

* 业务类型为 `skill`。
* 必须是目录。
* 默认链接到 `.agents/skills/<link_name>`。

Resource：

* 业务类型为 `resource`。
* 可以是文件或目录。
* 目标目录由注册或链接时指定。
* 最终链接名默认维持源文件或源目录原名称。

#### 2.5.3 路径规则
Registry 中：

```text
source_path = absolute path
```

项目 manifest 中：

```text
source_path = absolute path
link_path = project-relative path
```

symlink 本身在 MVP 阶段使用绝对 source path 作为目标。这样与 Registry 的 source path 规则一致，也避免受当前工作目录影响。

#### 2.5.4 Linkable Item 字段
Registry 中的 Linkable Item 至少包含：

```text
id
name
alias / link_name
type: skill / resource
kind: file / directory
source_path
source_type: local / git / external
source_ownership: external / managed
default_target_dir
description
repo_url
repo_commit
created_at
updated_at
```

MVP 只实现 `source_ownership = external`。`managed` 作为后续集中管理或自动 clone/update 能力的预留值。

#### 2.5.5 文件和目录检测
注册时必须自动检测 source 是文件还是目录，并记录为 `kind`。

* 用户不需要手动指定 `kind`。
* `kind` 是跨平台 symlink 执行所需元数据，不是额外业务负担。
* Windows 后端创建 symlink 时需要区分 file / directory。
* Unix 后端虽然不需要该区分，但 Core 抽象需要统一 `LinkKind`。
* 链接时如果实际 source kind 与 Registry 记录不一致，报错并提示重新注册或刷新。

#### 2.5.6 来源管理策略
MVP 采用分散管理。

* `aglink` 记录现有 Skill / Resource 的绝对路径。
* `aglink` 不复制源目录。
* `aglink` 不负责更新 GitHub 仓库。
* `aglink` 不删除源内容。

分散管理对 GitHub 仓库更友好，因为第三方 Skill 常位于仓库二级或三级子目录，直接记录绝对路径可以避免同步和仓库结构改写问题。

集中管理与混合管理只作为未来扩展方向，通过 `source_ownership`、`source_type`、`repo_url`、`repo_commit` 等字段预留。

#### 2.5.7 Registry 删除与项目链接
删除 Registry 项不自动删除已链接项目中的 symlink。

项目实际链接状态以 `.agents/links.toml` 为准。清理项目链接必须通过后续 CLI 命令体系中的 `unlink` / `clean` 能力完成。

### 2.6 工程模块边界、数据层与扩展策略 (Q-E)

#### 2.6.1 总体结论
* MVP 采用单 crate + internal modules，不采用 Rust workspace + 多 crate。
* CLI 层只负责参数解析、调用 command、格式化输出。
* command 层负责业务编排。
* core 层负责领域模型、路径、错误、symlink、manifest、SQLite、registry、framework、linkable item。
* command 层不直接写 SQL。
* command 层不直接读写 `.agents/links.toml`。
* command 层不直接调用平台 symlink API。
* 当前阶段不实现运行时插件系统，只保证代码架构可扩展。

#### 2.6.2 推荐目录结构
MVP 推荐单 crate 结构：

```text
agent-linker/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── cli.rs
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── init.rs
│   │   ├── config.rs
│   │   ├── db.rs
│   │   ├── framework.rs
│   │   ├── registry.rs
│   │   ├── link.rs
│   │   ├── group.rs
│   │   ├── status.rs
│   │   └── clean.rs
│   └── core/
│       ├── mod.rs
│       ├── error.rs
│       ├── paths.rs
│       ├── symlink.rs
│       ├── manifest.rs
│       ├── db.rs
│       ├── migrations.rs
│       ├── registry.rs
│       ├── framework.rs
│       └── linkable.rs
└── tests/
```

后续只有在 core API 稳定、复用需求明确或编译/测试边界确实需要时，再拆分为 workspace。

#### 2.6.3 Core 模块职责
`core::error`：

* 定义项目统一错误类型。
* 包装 IO、SQLite、TOML、symlink provider、Broker 等底层错误。

`core::paths`：

* 解析当前项目根目录。
* 解析全局数据目录、数据库路径、portable 模式、环境变量。
* 处理项目相对路径与绝对路径转换。

`core::symlink`：

* 定义 `SymlinkProvider`、`LinkKind`、`LinkStatus`。
* 提供 Unix、Windows Broker、Windows std、External ln 可选实现。
* 提供 mock provider 供测试使用。

`core::manifest`：

* 读写 `.agents/links.toml`。
* 校验 manifest schema。
* 提供 append / update / remove / status 查询 API。

`core::db` 与 `core::migrations`：

* 管理 SQLite 连接。
* 管理 schema migration。
* 提供数据库路径诊断。

`core::registry`：

* 管理 Linkable Item Registry 的持久化访问。
* 对 command 层暴露结构化 API，不暴露 SQL。

`core::framework`：

* 管理 Framework Adapter 与 mapping。
* 提供默认 framework 初始化能力。

`core::linkable`：

* 定义 Linkable Item 领域模型。
* 处理 Skill / Resource 的业务约束、alias、kind 检测和默认 link path 规则。

#### 2.6.4 Command 层职责
command 层只做编排，不直接处理底层存储或平台细节。

示例：

* `commands::init`：
  * 调用 `core::paths` 定位当前项目。
  * 调用 `core::framework` 读取启用框架。
  * 调用 `core::symlink` 创建兼容链接。
  * 调用 `core::manifest` 写入链接状态。
* `commands::registry`：
  * 调用 `core::linkable` 校验 source。
  * 调用 `core::registry` 写入 SQLite。
* `commands::status`：
  * 调用 `core::manifest` 读取项目链接。
  * 调用 `core::symlink` 检查实际状态。

#### 2.6.5 数据访问层
SQLite 数据访问必须集中在 core 内部。

* command 层不直接写 SQL。
* 数据库 schema 必须版本化 migration。
* migration 应在数据库初始化或显式 `aglink db migrate` 时运行。
* repository / DAO 风格 API 放在 `core::registry`、`core::framework` 等模块中。
* SQLite 只保存全局配置、Registry、Framework Adapter、Group 等机器级数据。
* 项目链接状态只保存在 `.agents/links.toml`。

#### 2.6.6 Manifest 与 SQLite 的边界
SQLite 与 manifest 是两个不同来源。

```text
SQLite:
  全局 registry/config/framework/group/linkable item

.agents/links.toml:
  当前项目实际由 aglink 管理的 symlink 状态
```

两者不能互相替代：

* SQLite 不记录每个项目的实际链接状态。
* manifest 不记录全局 Registry 或用户偏好。

#### 2.6.7 模块依赖规则
* `cli` 可以依赖 `commands`。
* `commands` 可以依赖 `core`。
* `core` 不依赖 `commands` 或 `cli`。
* `commands` 之间避免直接互相依赖；共享逻辑下沉到 `core`。
* 平台条件编译应限制在 `core::symlink` 或极少数平台路径解析代码中。

#### 2.6.8 扩展策略
当前阶段的可扩展性只指代码架构可扩展。

明确不做：

* 不实现运行时插件系统。
* 不开放第三方动态加载命令。
* 不开放自定义 Linkable Item 类型。

允许扩展：

* 新 Agent 框架通过 SQLite 中的 Framework Adapter 数据扩展。
* 新 mapping 通过 `framework_mappings` 扩展。
* 新 source ownership 模式通过已预留字段扩展。
* 后续可在不破坏 command 层的情况下替换或拆分 core 模块。

#### 2.6.9 测试边界
测试策略必须从一开始支持平台差异和状态存储差异。

* symlink provider 使用 mock provider 做单元测试。
* SQLite 使用临时数据库测试。
* manifest 使用临时目录测试。
* `init`、`link`、`status`、`clean` 使用集成测试覆盖核心流程。
* Windows Broker 后端使用独立集成测试，不阻塞 Unix 单元测试。
* 路径解析需要覆盖 Windows、macOS、Linux 的默认数据目录和 portable 模式。

### 2.7 CLI 命令体系：注册、分组、链接与清理 (Q-D)

#### 2.7.1 总体结论
CLI 采用“类型注册 + 统一链接”的命令体系。

```text
skill / resource / group / framework
  管理全局 Registry、分组和框架配置

link / unlink / status / clean
  管理当前项目链接状态

config / db / doctor
  管理配置解析、数据库和环境诊断
```

顶层命令：

```text
aglink init

aglink config ...
aglink db ...
aglink framework ...

aglink skill ...
aglink resource ...
aglink group ...

aglink link ...
aglink unlink ...
aglink status
aglink clean
aglink doctor
```

不提供 `skill link` / `resource link` 作为主路径。链接行为统一走 `aglink link`，由 Registry 中的 Linkable Item 类型决定具体目标。

#### 2.7.2 配置、数据库与框架命令
配置命令：

```text
aglink config path
aglink config list
aglink config get <key>
aglink config set <key> <value>
aglink config unset <key>
```

数据库命令：

```text
aglink db path
aglink db migrate
aglink db backup [path]
aglink db check
```

框架命令：

```text
aglink framework list
aglink framework show <name>
aglink framework enable <name>
aglink framework disable <name>
aglink framework mapping list <name>
aglink framework mapping add <name> <source> <link> --kind file|dir
aglink framework mapping remove <name> <link>
```

#### 2.7.3 Skill 命令
Skill 是目录，并且必须包含非空 `SKILL.md`。

```text
aglink skill add <path> [--name <name>] [--alias <link-name>]
aglink skill list
aglink skill show <name>
aglink skill rename <old> <new>
aglink skill remove <name>
aglink skill refresh <name>
```

规则：

* `<path>` 必须是目录。
* `<path>/SKILL.md` 必须存在且非空。
* `source_path` 记录绝对路径。
* 默认 `name` 使用目录名。
* 默认链接名使用 `name`，可通过 `--alias` 设置。
* 同名不同源报错，提示用户改名或指定 `--name`。
* `refresh` 用于重新检测 source 是否存在、kind 是否变化、元数据是否仍有效。

#### 2.7.4 Resource 命令
Resource 可以是文件或目录。

```text
aglink resource add <path> --target-dir <project-relative-dir> [--name <name>] [--alias <link-name>]
aglink resource list
aglink resource show <name>
aglink resource rename <old> <new>
aglink resource remove <name>
aglink resource refresh <name>
```

规则：

* `<path>` 自动检测 file / directory。
* `--target-dir` 是项目内相对路径，例如 `.agents/tools`、`ref-docs`。
* 最终链接路径为 `<target-dir>/<alias-or-source-basename>`。
* 默认链接名维持源文件或源目录原名称。
* `refresh` 用于重新检测 source 是否存在、kind 是否变化、元数据是否仍有效。

#### 2.7.5 Group 命令
Group 定义存储在全局 SQLite。

```text
aglink group create <name>
aglink group list
aglink group show <name>
aglink group rename <old> <new>
aglink group delete <name>

aglink group add <group> <item>...
aglink group remove <group> <item>...
aglink group link <group>
aglink group unlink <group>
```

规则：

* 不支持嵌套 Group。
* 不提供预定义 Group 模板。
* 一个 Linkable Item 可以属于多个 Group。
* Group 不能属于 Group。
* `group link` 链接组内全部 item。
* `group unlink` 取消链接组内全部 item，但只删除 manifest 中由 `aglink` 管理的 symlink。

#### 2.7.6 Link / Unlink 命令
统一链接入口：

```text
aglink link <name>
aglink link <name> --as <link-name>
aglink link <name> --target-dir <dir>
aglink link --group <group>
```

规则：

* `aglink link <name>` 根据 Registry 中的 Linkable Item 类型决定链接目标。
* Skill 默认链接到 `.agents/skills/<link-name>`。
* Resource 默认链接到注册时记录的 `target_dir`。
* `--target-dir` 只用于 Resource 的本次链接目标覆盖。
* `--as` 只用于本次链接名覆盖，不改 Registry 中的 alias。
* 成功链接必须写入 `.agents/links.toml`。

取消链接入口：

```text
aglink unlink <name>
aglink unlink --group <group>
aglink unlink --all
```

规则：

* `unlink` 只删除 manifest 中由 `aglink` 管理的 symlink。
* `unlink` 不删除真实源文件或真实源目录。
* `unlink --all` 只处理当前项目 manifest 中的链接。

#### 2.7.7 Status / Clean / Doctor 命令
状态命令：

```text
aglink status
aglink status --json
```

规则：

* 检查当前项目 `.agents/links.toml` 中记录的链接。
* 报告链接存在、断链、目标错误、源缺失、manifest 记录异常等状态。

清理命令：

```text
aglink clean
aglink clean --broken
aglink clean --missing-source
```

规则：

* `clean` 只清理 manifest 中记录的 symlink。
* `clean` 不删除真实文件或真实目录。
* `--broken` 只清理断链。
* `--missing-source` 只清理源路径不存在的链接记录及对应 symlink。

诊断命令：

```text
aglink doctor
```

检查内容：

* 当前数据库路径和可写性。
* schema migration 状态。
* 当前项目 manifest 状态。
* Framework Adapter 映射是否有效。
* Windows Broker 是否安装、运行、协议兼容。
* symlink 创建权限或后端可用性。

#### 2.7.8 临时测试命令
原需求中的 `test` 命令不作为正式顶层命令保留。

MVP 使用 `aglink link <name>` 和后续可能的 `aglink link --local <path>` 覆盖临时验证场景。是否加入 `--local` 留待实现阶段按实际需要决定，不进入当前正式命令集。

---

> *(后续讨论结论将在此文档中逐步补充...)*
