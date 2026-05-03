# Agent Linker CLI Reference (命令行指南)

`aglink` 提供了“类型注册 + 统一链接”的命令体系，将全局 Registry 管理与当前项目的链接状态管理优雅分离。

## 🌐 Global Options (全局选项)

所有命令均支持以下全局标志，用于控制输出的详细程度：

- `aglink --quiet <command>`
  **用途**: 静默模式。
  **说明**: 仅输出必要的错误信息，隐藏进度和摘要。
- `aglink --verbose <command>`
  **用途**: 详细模式。
  **说明**: 输出包含底层诊断、后端交互及系统错误码等详细日志。

## 🏁 Initialization & Status (初始化与状态)

管理当前项目的链接清单与健康状况。

- `aglink init`
  **用途**: 初始化当前项目的链接环境。
  **说明**: 在当前目录下创建 `.agents/` 管理目录及 `.agents/links.toml` 链接状态清单，并自动管理 `.gitignore`，为当前工程建立 Contract (契约)。MVP 阶段不支持 `--dry-run`。

- `aglink status` / `aglink status --json`
  **用途**: 查看当前项目的链接状态。
  **说明**: 检查并输出项目中所有被管理的 Symlink (符号链接) 状态。`--json` 格式提供供机器读取的稳态输出（如 `missing`、`correct_symlink`、`broken_symlink` 等稳定键值）。

- `aglink clean [--broken] [--missing-source] [--dry-run]`
  **用途**: 清理失效的链接。
  **说明**: 仅处理 `.agents/links.toml` 清单中由 `aglink` 管理且状态异常的链接。`--dry-run` 允许在不实际删除的情况下预览操作。

- `aglink doctor`
  **用途**: 全局环境健康诊断。
  **说明**: 检查 SQLite 数据库、Migration、清单文件、Framework Adapter、Symlink (符号链接) 后端（如 Windows Broker）的可用性与配置正确性。

## 🔗 Link Management (链接管理)

核心的链接操作命令。所有的链接行为统一通过 `link` 与 `unlink` 完成。

- `aglink link <name> [--as <link-name>] [--target-dir <dir>] [--force] [--dry-run]`
  **用途**: 为当前项目创建 Linkable Item 的 Symlink (符号链接)。
  **说明**: 根据全局 Registry 中的类型（Skill 或 Resource），将目标链接到当前项目中。
  - `--as`: 临时覆盖本次链接的名称，不影响 Registry 中的别名。
  - `--target-dir`: 覆盖 Resource 本次的链接目标目录。
  - `--force`: 仅允许替换指向错误的符号链接，**绝不允许**删除真实文件或目录。

- `aglink link --group <group> [--dry-run]`
  **用途**: 批量创建分组链接。
  **说明**: 将指定 Group (分组) 下的所有成员一次性链接到当前项目。

- `aglink unlink <name> [--dry-run]`
  **用途**: 移除指定的链接。
  **说明**: 安全移除当前项目中受管理的符号链接。

- `aglink unlink --group <group> [--dry-run]` / `aglink unlink --all [--dry-run]`
  **用途**: 批量移除链接。
  **说明**: 移除特定分组内的所有链接，或通过 `--all` 清理当前项目中的所有管理的链接。

## 🛠️ Linkable Item: Skill (技能管理)

管理全局可用的 Agent 技能。

- `aglink skill add <path> [--name <name>] [--alias <link-name>]`
  **用途**: 注册新的 Skill。
  **说明**: 将指定路径注册为全局 Skill。可选指定注册名称与默认别名。

- `aglink skill list` / `aglink skill show <name>`
  **用途**: 查询 Skill 信息。
  **说明**: 列出所有已注册的 Skill 或展示指定 Skill 的详细信息。

- `aglink skill rename <old> <new>`
  **用途**: 重命名 Skill。
  **说明**: 修改 Registry 中已注册 Skill 的标识名称。

- `aglink skill remove <name>`
  **用途**: 移除 Skill 注册。
  **说明**: 从全局 Registry 中注销该 Skill，但不会删除原始真实文件。

- `aglink skill refresh <name>`
  **用途**: 刷新 Skill 状态。
  **说明**: 重新检查并更新 Skill 源路径的有效性。

## 📄 Linkable Item: Resource (资源管理)

管理全局可用的背景知识或文档等资源。

- `aglink resource add <path> --target-dir <project-relative-dir> [--name <name>] [--alias <link-name>]`
  **用途**: 注册新的 Resource。
  **说明**: 将指定路径注册为全局 Resource。必须提供 `--target-dir` 作为默认在目标项目中映射的相对路径。

- `aglink resource list` / `aglink resource show <name>`
  **用途**: 查询 Resource 信息。
  **说明**: 浏览资源列表或查看单个资源的详情与映射规则。

- `aglink resource rename <old> <new>` / `aglink resource remove <name>` / `aglink resource refresh <name>`
  **用途**: 维护 Resource。
  **说明**: 对已注册的资源进行重命名、移除注册或刷新源状态操作。

## 🗂️ Group (分组管理)

将多个 Skill 与 Resource 组合为逻辑集合，方便批量操作。

- `aglink group create <name>` / `aglink group delete <name>` / `aglink group rename <old> <new>`
  **用途**: 维护分组生命周期。
  **说明**: 创建新分组、删除现有分组或进行重命名。

- `aglink group add <group> <item>...` / `aglink group remove <group> <item>...`
  **用途**: 管理分组内成员。
  **说明**: 向分组中添加或移除特定的 Skill 与 Resource。

- `aglink group list` / `aglink group show <name>`
  **用途**: 查询分组。
  **说明**: 查看所有分组的列表或特定分组的成员明细。

- `aglink group link <group> [--dry-run]` / `aglink group unlink <group> [--dry-run]`
  **用途**: 分组链接操作。
  **说明**: 提供与 `aglink link --group` 等价的能力，通过 Group 视角批量创建或移除项目的链接。

## ⚙️ Config & Database (配置与存储管理)

管理 CLI 行为与底层 SQLite 数据库。

- `aglink config path` / `aglink config list`
  **用途**: 查看配置信息。
  **说明**: 显示全局配置文件的路径及当前所有配置项。

- `aglink config get <key>` / `aglink config set <key> <value>` / `aglink config unset <key>`
  **用途**: 修改配置项。
  **说明**: 读取、更新或移除特定的全局配置键值。

- `aglink db path` / `aglink db migrate` / `aglink db backup [path]` / `aglink db check`
  **用途**: 管理 SQLite 数据库。
  **说明**: 获取数据库路径、手动执行数据表结构迁移、备份当前数据库以及检查数据库完整性。

## 🤖 Framework (框架适配管理)

管理目标 AI 代理框架（如 Claude 等）的特殊路径映射。

- `aglink framework list` / `aglink framework show <name>`
  **用途**: 查询框架支持。
  **说明**: 列出所有可用的 AI 框架及其默认的映射规则。

- `aglink framework enable <name>` / `aglink framework disable <name>`
  **用途**: 启停框架适配。
  **说明**: 在当前项目中启用或禁用特定框架的自动化映射投射。

- `aglink framework mapping list <name>` / `aglink framework mapping add <name> <source> <link> --kind file|dir` / `aglink framework mapping remove <name> <link>`
  **用途**: 自定义框架映射。
  **说明**: 查看、增加或移除特定框架的源到链接的映射规则，满足框架要求的特定 Format (格式) 及路径要求。