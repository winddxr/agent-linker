# CLI Command Surface

## 设计目标

提供“类型注册 + 统一链接”的命令体系，让全局 Registry 管理和当前项目链接状态管理保持分离。

## 职责边界

- CLI 层负责参数解析、调用 command 和格式化输出。
- command 层负责业务编排。
- 底层存储、manifest 和 symlink 操作必须通过 core 能力完成。

## 架构约束

- 不提供 `skill link` 或 `resource link` 作为主路径。
- 链接行为统一走 `aglink link`。
- 会修改链接状态的命令除已声明 MVP 例外外，应支持 `--dry-run`。
- `test` 不作为正式顶层命令。

## 顶层命令

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

## 命令契约

全局输出选项：

```text
aglink --quiet <command>
aglink --verbose <command>
```

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

Skill 命令：

```text
aglink skill add <path> [--name <name>] [--alias <link-name>]
aglink skill list
aglink skill show <name>
aglink skill rename <old> <new>
aglink skill remove <name>
aglink skill refresh <name>
```

Resource 命令：

```text
aglink resource add <path> --target-dir <project-relative-dir> [--name <name>] [--alias <link-name>]
aglink resource list
aglink resource show <name>
aglink resource rename <old> <new>
aglink resource remove <name>
aglink resource refresh <name>
```

Group 命令：

```text
aglink group create <name>
aglink group list
aglink group show <name>
aglink group rename <old> <new>
aglink group delete <name>
aglink group add <group> <item>...
aglink group remove <group> <item>...
aglink group link <group>
aglink group link <group> --dry-run
aglink group unlink <group>
aglink group unlink <group> --dry-run
```

链接命令：

```text
aglink link <name>
aglink link <name> --as <link-name>
aglink link <name> --target-dir <dir>
aglink link <name> --force
aglink link <name> --dry-run
aglink link --group <group>
aglink link --group <group> --dry-run
```

取消链接命令：

```text
aglink unlink <name>
aglink unlink --group <group>
aglink unlink --all
aglink unlink <name> --dry-run
aglink unlink --group <group> --dry-run
aglink unlink --all --dry-run
```

状态、清理、诊断：

```text
aglink status
aglink status --json
aglink clean
aglink clean --broken
aglink clean --missing-source
aglink clean --dry-run
aglink doctor
```

## 关键决策

- `link` 根据 Registry 中的 Linkable Item 类型决定链接目标。
- `--as` 只覆盖本次链接名，不修改 Registry alias。
- `--target-dir` 只用于 Resource 的本次链接目标覆盖。
- `--force` 只允许替换错误 symlink，不允许删除真实文件或真实目录。
- `--dry-run` 覆盖 `link`、`unlink`、`clean`、`group link` 和 `group unlink`；`init` 在 MVP 中暂不支持 dry run。
- `status --json` 的 `links[].status` 使用稳定 snake_case 机器值：`missing`、`correct_symlink`、`wrong_symlink_target`、`broken_symlink`、`existing_real_file`、`existing_real_directory`、`unsupported_file_type`。
- `unlink` 和 `clean` 只处理 manifest 中由 `aglink` 管理的 symlink。
- `doctor` 检查数据库、migration、manifest、Framework Adapter、Windows Broker 和 symlink 后端可用性。
- 临时验证场景由 `aglink link <name>` 和后续可能的 `aglink link --local <path>` 承担，`--local` 暂不进入正式命令集。

## 验收口径

- 全局 Registry 命令不直接改变当前项目链接状态。
- 当前项目链接命令必须写入或读取 `.agents/links.toml`。
- 用户输出默认简洁，`--verbose` 提供诊断信息，`--quiet` 只输出必要错误。
- 成功操作输出创建、跳过或更新等摘要。
