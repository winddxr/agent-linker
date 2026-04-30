# Agent Linker — 开发前讨论清单

> 本清单基于 [原始需求.md](file:///f:/00_Workspace/AI_Agents/agent_linker/docs/%E5%8E%9F%E5%A7%8B%E9%9C%82%E6%B1%82.md) 逐条分析，按开发流程顺序排列。  
> 每个议题需逐一讨论并达成结论后，方可进入对应阶段的开发。  
> 状态标记：⬜ 待讨论 · 🔵 讨论中 · ✅ 已确定

---

## 术语规范（贯穿全项目） ✅

原始需求中存在以下术语不规范或有歧义的表述，现已逐一澄清并统一：

| # | 原始需求中的表述 | 问题 | 澄清与规范 |
|---|---|---|---|
| T1 | "symbolic links""符号连接"混用 | 同一概念使用了英文和非标准中文翻译，且"符号连接"不是标准术语 | **符号链接（symlink）** |
| T2 | "anget skill"（第16行） | 拼写错误 | **Agent Skill（技能）** |
| T3 | "堵路开发"（第8行） | 疑似"独立开发"的笔误 | **独立开发** |
| T4 | "craet-documents"（第29行） | 拼写错误 | **create-documents** |
| T5 | "写在分组类的skill"（第29行） | 笔误 | **指：按分组名加载或卸载分组内的skill** |
| T6 | "一个test命令"（第30行） | 语义澄清 | **指：在skill开发项目内临时试验该skill的命令** |
| T7 | "创建 AGENTS.md 的symbolic links 到CLAUDE.md"（第12行） | symlink 方向澄清 | **`AGENTS.md` 是真实文件，`CLAUDE.md` 是指向它的链接** |
| T8 | "创建 .agents/skills/ 到 .claude/skills/"（第13行） | symlink 方向澄清 | **`.agents/skills/` 是真实目录，`.claude/skills/` 是指向它的链接** |
| T9 | "公共内容"（第4行） | 指代澄清 | **指：我们需要开发的这个项目的代码内公共函数等内容** |
| T10 | "基本目录"（第11行） | 具体实现细节 | **留在 Q-B 详细讨论** |

> [!IMPORTANT]
> 以下为已确认的核心术语定义表，全项目统一使用：

| 术语 | 定义 |
|---|---|
| **Symlink（符号链接）** | 操作系统级别的符号链接（symbolic link），非快捷方式、非硬链接 |
| **Skill（技能）** | 一个按用途命名的目录，包含 AI Agent 可使用的提示词/脚本/文档等资源 |
| **Skill Source（技能源）** | Skill 的原始存储位置（可能是本地自研目录、GitHub 克隆仓库等） |
| **Skill Registry（技能注册表）** | 记录所有已知 Skill 及其源路径的索引/数据库 |
| **Skill Group（技能组）** | 将多个 Skill 按业务场景归类的逻辑分组（如 `design`、`coding`） |
| **Target Project（目标项目）** | 需要链入 Skill / 资源的 AI Agent 项目目录 |
| **Resource（资源）** | 工具脚本、公共文档等非 Skill 类可链接内容 |
| **Linkable Item（可链接项）** | Skill 和 Resource 的统称，表示任何可通过 symlink 链入目标项目的内容 |
| **Init（初始化）** | 在目标项目中创建基础目录结构和初始 symlink 的操作 |

---

## 第一部分：项目级决策

### Q1. 项目名称与 CLI 命令名 ✅

**问题描述：** 项目目前命名为 `agent_linker`，最终编译出的 CLI 可执行文件应叫什么名字？全称可能过长，需要设计一个合理的简短别名方便日常使用。

**讨论结论：**
- 项目级名称仍为 `agent_linker`。
- **CLI 可执行文件/命令名正式定为：`aglink`**。
  - 理由：具有极好的辨识度（Agent Link），且简短易拼写，能有效避免与其他通用终端工具重名。

---

### Q2. 跨平台符号链接策略 ✅

**问题描述：** 需求提到跨平台支持 Linux / macOS / Windows。Windows 下已有 `win-ln` 项目提供无需管理员权限的 `ln` 命令。需求中还提出了一个问题："macOS 是否有和 Linux 行为一致的符号链接？"

**背景事实：**
- macOS 的 `ln -s` 命令与 Linux 行为一致，无需额外处理
- Windows 下使用已有的 `win-ln` 项目

**合并讨论范围：**
- Q2：跨平台 symlink 后端选择
- Q13：Core 中 symlink 抽象的边界
- Q16：symlink 状态追踪与幂等性
- Q17：symlink 错误模型与用户反馈

**讨论结论：**
1. 本项目内部**不以外部 `ln` 命令作为跨平台 symlink 的主实现**。
2. Linux / macOS 使用 Rust 标准库 `std::os::unix::fs::symlink`。
3. Windows 默认通过 `WinSymlinksBroker` 的 IPC client 创建 symlink：`aglink` → Broker client → Named Pipe → `WinSymlinksBroker` → `CreateSymbolicLinkW`。
4. Windows 下已有的 `ln.exe` 仅作为独立命令、人工调试工具或兼容入口保留，不作为 `agent-linker` 核心逻辑的默认调用路径。
5. Windows Rust 标准库 `std::os::windows::fs::symlink_file` / `symlink_dir` 仅作为可选 fallback，用于管理员权限、Developer Mode 或测试环境。
6. Symlink 创建、删除、读取和验证属于 Core 能力，统一通过 `SymlinkProvider` 抽象访问。
7. 所有链接创建操作必须幂等：
   - 链接不存在则创建。
   - 已存在且指向正确源路径则视为成功。
   - 已存在但指向错误目标则默认报错，可通过显式 `--force` 重建该 symlink。
   - 已存在真实文件/目录时默认报错，`--force` 不删除真实文件/目录。
8. 目标项目需要维护 link manifest，初始建议路径为 `.agents/links.toml`；若后续 Q4 对配置格式和存储位置做出统一调整，可随 Q4 调整。
9. Core 定义统一的 symlink 错误类型，CLI 层负责转换为用户可执行的错误提示。
10. 除已明确的 MVP 例外外，会修改链接状态的命令应支持 `--dry-run`，并支持 `--verbose` / `--quiet` 控制输出；`aglink init` 在 MVP 阶段暂不提供 `--dry-run`、`--force`、`--verbose`。

> 详细正式决策见 [architecture_decisions.md](architecture_decisions.md) 的 **2.2 跨平台 Symlink 策略与链接生命周期 (Q2)**。

---

## 第二部分：已完成的基础决策补充

### Q6. Symlink 方向的明确定义 ✅

**问题描述：** 需求中写的是"创建 `AGENTS.md` 的 symbolic links 到 `CLAUDE.md`"。Symlink 有源（source/target）和链接名（link name）两个概念，方向至关重要。原始需求的表述存在歧义（见 T7、T8）。

**讨论结论：**
- `AGENTS.md` 和 `.agents/skills/` 是**真实文件/目录**。
- `CLAUDE.md` 和 `.claude/skills/` 是指向它们的 **symlink**。

> [!IMPORTANT]
> 这一方向确立后，真实内容归属于我们定义的 `AGENTS.md` 规范，Agent 框架特定的文件（如 `CLAUDE.md`）仅作为兼容性链接存在。这要求 `.gitignore` 中通常应该忽略这些软链接（除非有特定的 Agent 框架提交要求）。

---

## 第三部分：待讨论整合议题

> 以下议题由原 Q3-Q15 重新整合而来。Q16/Q17 的 symlink 状态、幂等、错误反馈内容已并入 Q2 并完成讨论。

### Q-A. 配置体系与 Agent 框架适配策略 ✅

**合并原议题：** Q3 + Q4 + Q5 的配置相关部分

**问题描述：** 项目需要同时确定 Agent 框架兼容方式与配置体系。框架映射、全局 Registry、项目级配置、manifest 路径等内容互相影响，应作为同一个决策包讨论。

**讨论结论：**
1. 统一以 `AGENTS.md` 和 `.agents/` 作为真实源；Agent 框架特定文件和目录只作为兼容性投射。
2. 预留多框架适配能力，通过 Framework Adapter 管理框架映射。
3. 内置 Claude 映射：
   - `AGENTS.md` → `CLAUDE.md`
   - `.agents/skills/` → `.claude/skills/`
4. 全局配置、Registry、框架适配、Skill / Resource 等结构化数据统一存储在 SQLite 数据库中。
5. 默认数据库路径遵循平台用户数据目录：
   - Windows: `%APPDATA%\agent-linker\agent-linker.db`
   - macOS: `~/Library/Application Support/agent-linker/agent-linker.db`
   - Linux: `$XDG_DATA_HOME/agent-linker/agent-linker.db`，fallback 为 `~/.local/share/agent-linker/agent-linker.db`
6. 支持 portable 模式：
   - 可执行文件同目录存在 `agent-linker.db` 时，可使用该数据库作为 portable 数据库。
   - 允许通过 `AGLINK_DB` 显式指定数据库路径。
   - 允许通过 `AGLINK_HOME` 显式指定数据根目录。
7. Windows 不默认把数据库写到 exe 同目录，因为安装目录可能不可写。
8. MVP 阶段不创建项目级配置文件，例如不创建 `.agents/config.toml`。
9. 项目内保留 `.agents/links.toml` 作为 link manifest；它是项目链接状态清单，不是项目配置文件。
10. `.agents/links.toml` 是必须的，用于支持 `status`、`unlink`、`clean`、审计和全局数据库不可用时的项目链接识别。
11. 配置、数据库和框架适配都通过同一个 `aglink` 可执行文件管理，不拆分独立配置命令。
12. CLI 边界：
    - `aglink config ...` 管理基础用户配置和显示配置解析结果。
    - `aglink db ...` 管理数据库路径、迁移、备份、诊断。
    - `aglink framework ...` 管理框架适配、启用/禁用框架、查看映射。
13. `aglink init` 可提供框架快捷参数，例如 `--framework claude`、`--framework claude,cursor`、`--framework all`。

> 详细正式决策见 [architecture_decisions.md](architecture_decisions.md) 的 **2.3 配置体系与 Agent 框架适配策略 (Q-A)**。

---

### Q-B. Init 命令的目标结构与幂等行为 ✅

**合并原议题：** Q5 + Q3 的框架映射结果 + Q2/Q6 的 symlink 前提

**问题描述：** `init` 是项目进入可管理状态的入口，应明确它创建哪些真实文件、目录、兼容 symlink、配置文件，以及重复执行时的行为。

**讨论结论：**
1. `aglink init` 只作用于当前目录，不支持 `aglink init <path>`。
2. `init` 必须幂等，重复执行不得覆盖现有真实文件或真实目录。
3. `AGENTS.md` 不存在时创建空文件；已存在真实文件则保留；若是目录或 symlink 则报错。
4. `init` 至少创建或确认：
   - `AGENTS.md`
   - `.agents/`
   - `.agents/skills/`
   - `.agents/links.toml`
   - 根据启用的 Framework Adapter 创建框架兼容 symlink
5. 默认启用哪些 framework 由全局 SQLite 中的 `enabled_by_default` 决定；MVP 内置 `claude` 并默认启用。
6. `.agents/` 和 `.agents/skills/` 已存在且为目录时保留；若不是目录则报错。
7. 框架兼容 symlink 不存在时创建，已是正确 symlink 时视为成功，错误 symlink 或真实文件/目录冲突时默认报错。
8. `init` 自动追加或更新 `.gitignore` 的 aglink managed block，不覆盖用户已有内容。
9. `.gitignore` managed block 的具体条目来自全局 SQLite 配置。
10. MVP 内置默认 ignore patterns：
    - `.claude/`
    - `.agents/skills/`
    - `.agents/links.toml`
11. `init` 必须写入 `.agents/links.toml`；manifest 已存在且格式损坏时，报错且不覆盖。
12. `init` 不创建 `.agents/config.toml` 或其他项目级配置文件。
13. `init` 在 MVP 阶段暂不提供 `--dry-run`、`--force`、`--verbose`。

> 详细正式决策见 [architecture_decisions.md](architecture_decisions.md) 的 **2.4 Init 命令的目标结构与幂等行为 (Q-B)**。

---

### Q-C. Linkable Item、Registry 与来源管理模型 ✅

**合并原议题：** Q7 + Q11 + Q12

**问题描述：** Skill 与 Resource 都是可链接内容，但它们的来源、默认目标位置、元数据和管理方式不同。需要先确定统一数据模型，再设计命令。

**讨论结论：**
1. Skill 和 Resource 统一建模为 **Linkable Item（可链接项）**。
2. Registry 统一存储所有 Linkable Item，并存储在全局 SQLite 数据库中。
3. MVP 采用分散管理模式：`aglink` 只记录源路径，不复制、不移动、不接管源内容。
4. Linkable Item 的 `source_path` 永远存绝对路径。
5. 当前项目内的 `link_path` 使用相对项目根目录的路径。
6. symlink 本身在 MVP 阶段使用绝对 source path 作为目标。
7. 同名不同源默认报错，提示用户改名或指定 alias。
8. Skill 必须是目录，默认链接到 `.agents/skills/<link_name>`。
9. Resource 可以是文件或目录，目标目录由注册或链接时指定，最终链接名默认维持源文件或源目录原名称。
10. 注册时必须自动检测 source 是文件还是目录，并记录为 `kind = file | directory`；用户不需要手动指定。
11. `kind` 是跨平台 symlink 执行所需元数据。Windows 后端需要区分 file / directory，Unix 后端也通过 Core 抽象统一处理。
12. 链接时如果实际 source kind 与 Registry 记录不一致，报错并提示重新注册或刷新。
13. Registry 字段预留 `source_ownership = external / managed`，MVP 只实现 `external`。
14. 删除 Registry 项不自动删除已链接项目中的 symlink；项目实际链接状态以 `.agents/links.toml` 为准。

> 详细正式决策见 [architecture_decisions.md](architecture_decisions.md) 的 **2.5 Linkable Item、Registry 与来源管理模型 (Q-C)**。

---

### Q-D. CLI 命令体系：注册、分组、链接与清理 ✅

**合并原议题：** Q8 + Q9 + Q10 + Q11 的命令部分 + Q12 的命令部分 + Q16 的命令命名部分

**问题描述：** 在确定 Linkable Item 模型后，需要设计整个项目的完整 CLI 命令体系，同时兼顾语义清晰度和日常使用便利性。

**讨论结论：**
1. CLI 采用“类型注册 + 统一链接”的命令体系：
   - `skill` / `resource` / `group` / `framework` 管理全局 Registry、分组和框架配置。
   - `link` / `unlink` / `status` / `clean` 管理当前项目链接状态。
   - `config` / `db` / `doctor` 管理配置解析、数据库和环境诊断。
2. 顶层命令包括：
   - `aglink init`
   - `aglink config ...`
   - `aglink db ...`
   - `aglink framework ...`
   - `aglink skill ...`
   - `aglink resource ...`
   - `aglink group ...`
   - `aglink link ...`
   - `aglink unlink ...`
   - `aglink status`
   - `aglink clean`
   - `aglink doctor`
3. 不提供 `skill link` / `resource link` 作为主路径；链接行为统一走 `aglink link`。
4. Skill 是目录，并且必须包含非空 `SKILL.md`。
5. Skill 命令：
   - `aglink skill add <path> [--name <name>] [--alias <link-name>]`
   - `aglink skill list`
   - `aglink skill show <name>`
   - `aglink skill rename <old> <new>`
   - `aglink skill remove <name>`
   - `aglink skill refresh <name>`
6. Resource 命令：
   - `aglink resource add <path> --target-dir <project-relative-dir> [--name <name>] [--alias <link-name>]`
   - `aglink resource list`
   - `aglink resource show <name>`
   - `aglink resource rename <old> <new>`
   - `aglink resource remove <name>`
   - `aglink resource refresh <name>`
7. Resource 可以是文件或目录；最终链接路径为 `<target-dir>/<alias-or-source-basename>`。
8. Group 定义存储在全局 SQLite。
9. Group 不支持嵌套，不提供预定义模板；一个 Linkable Item 可以属于多个 Group。
10. Group 命令：
    - `aglink group create <name>`
    - `aglink group list`
    - `aglink group show <name>`
    - `aglink group rename <old> <new>`
    - `aglink group delete <name>`
    - `aglink group add <group> <item>...`
    - `aglink group remove <group> <item>...`
    - `aglink group link <group>`
    - `aglink group unlink <group>`
11. 统一链接命令：
    - `aglink link <name>`
    - `aglink link <name> --as <link-name>`
    - `aglink link <name> --target-dir <dir>`
    - `aglink link --group <group>`
12. 取消链接命令：
    - `aglink unlink <name>`
    - `aglink unlink --group <group>`
    - `aglink unlink --all`
13. `unlink` 只删除 manifest 中由 `aglink` 管理的 symlink，不删除真实源文件或真实源目录。
14. 状态与清理命令：
    - `aglink status`
    - `aglink status --json`
    - `aglink clean`
    - `aglink clean --broken`
    - `aglink clean --missing-source`
15. `aglink doctor` 用于检查数据库、manifest、framework、Windows Broker、权限和 symlink 后端可用性。
16. 原需求中的 `test` 命令不作为正式顶层命令保留。

> 详细正式决策见 [architecture_decisions.md](architecture_decisions.md) 的 **2.7 CLI 命令体系：注册、分组、链接与清理 (Q-D)**。

---

### Q-E. 工程模块边界、数据层与扩展策略 ✅

**合并原议题：** Q13 + Q14 + Q15，并吸收 Q-A / Q-B / Q-C 已确定架构前提

**问题描述：** Q-A / Q-B / Q-C 已经确定全局 SQLite、项目 manifest、Framework Adapter、Linkable Item、分散管理和 init 行为。Q-E 不再泛泛讨论 Core 是否存在，而是明确工程模块边界、数据访问层、扩展策略和测试边界。

**讨论结论：**
1. MVP 采用单 crate + internal modules，不采用 Rust workspace + 多 crate。
2. CLI 层只负责参数解析、调用 command、格式化输出。
3. command 层负责业务编排。
4. core 层负责领域模型、路径、错误、symlink、manifest、SQLite、registry、framework、linkable item。
5. command 层不直接写 SQL。
6. command 层不直接读写 `.agents/links.toml`。
7. command 层不直接调用平台 symlink API。
8. SQLite 数据访问集中在 core 内部，数据库 schema 必须版本化 migration。
9. `.agents/links.toml` 读写属于 `core::manifest`。
10. SQLite 与 manifest 边界明确：
    - SQLite：全局 registry / config / framework / group / linkable item。
    - manifest：当前项目实际由 `aglink` 管理的 symlink 状态。
11. `commands` 之间避免直接互相依赖，共享逻辑下沉到 `core`。
12. 当前阶段不实现运行时插件系统，只保证代码架构可扩展。
13. 新 Agent 框架通过 SQLite 中的 Framework Adapter 数据扩展。
14. MVP 不开放自定义 Linkable Item 类型，只支持 `skill` / `resource`。
15. 测试策略：
    - symlink provider 使用 mock provider。
    - SQLite 使用临时数据库。
    - manifest 使用临时目录。
    - `init`、`link`、`status`、`clean` 使用集成测试。
    - Windows Broker 后端使用独立集成测试。

> 详细正式决策见 [architecture_decisions.md](architecture_decisions.md) 的 **2.6 工程模块边界、数据层与扩展策略 (Q-E)**。

---

## 第四部分：原议题映射

| 原议题 | 当前归属 | 状态 |
|---|---|---|
| Q1 项目名称与 CLI 命令名 | Q1 | ✅ 已确定 |
| Q2 跨平台符号链接策略 | Q2 | ✅ 已确定 |
| Q3 目标 Agent 框架兼容性 | Q-A / Q-B | ✅ 已确定 |
| Q4 配置文件格式与存储位置 | Q-A | ✅ 已确定 |
| Q5 `init` 命令的具体行为 | Q-A / Q-B | ✅ 已确定 |
| Q6 Symlink 方向的明确定义 | Q6 | ✅ 已确定 |
| Q7 Skill Source 管理策略 | Q-C | ✅ 已确定 |
| Q8 Skill 发现与注册流程 | Q-D | ✅ 已确定 |
| Q9 Skill Group 设计 | Q-D | ✅ 已确定 |
| Q10 Skill 链接行为 | Q-D | ✅ 已确定 |
| Q11 Resource 定义与管理 | Q-C / Q-D | ✅ 已确定 |
| Q12 Stage 2 与 Stage 3 底层统一 | Q-C / Q-D | ✅ 已确定 |
| Q13 Core 边界 | Q2 / Q-E | ✅ 已确定 |
| Q14 模块化与 Stage 划分 | Q-E | ✅ 已确定 |
| Q15 可扩展性设计 | Q-E | ✅ 已确定 |
| Q16 状态管理与幂等性 | Q2 / Q-D | ✅ 已确定 |
| Q17 错误处理与用户反馈 | Q2 | ✅ 已确定 |

---

## 讨论推进建议

所有当前开发前讨论议题均已完成。后续可进入实现计划拆分与开发阶段。

---

## 讨论记录区

> 此区域用于记录每个议题的讨论结论。讨论完成后将更新对应议题的状态标记。
