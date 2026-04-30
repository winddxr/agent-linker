# Framework Adapter

## 设计目标

通过“真实源到框架约定路径”的映射模型，让不同 Agent 框架复用同一套真实内容源。

## 职责边界

- 管理框架定义与 mapping。
- 决定哪些框架默认启用。
- 为 `init` 和链接状态管理提供映射数据。
- 支持内置框架和用户自定义框架。

## 架构约束

- `AGENTS.md` 与 `.agents/` 是唯一真实内容源。
- 框架特定文件和目录不作为真实内容源。
- 框架适配数据存储在全局 SQLite。
- 新框架通过数据扩展，不通过运行时插件扩展。

## 接口契约

框架定义至少包含：

- `id`
- `name`
- `display_name`
- `built_in`
- `enabled_by_default`
- `created_at`
- `updated_at`

框架 mapping 至少包含：

- `id`
- `framework_id`
- `source_path`
- `link_path`
- `link_kind`
- `required`

内置 Claude mapping：

- `AGENTS.md` -> `CLAUDE.md`
- `.agents/skills/` -> `.claude/skills/`

## 关键决策

- MVP 内置 `claude` 并默认启用。
- `aglink init --framework` 可覆盖初始化时启用的框架集合。
- 后续框架沿用相同 mapping 模型。
- mapping 创建 symlink 时必须走 Symlink Core。

## 验收口径

- `init` 能根据启用框架生成对应兼容链接。
- 框架链接状态能写入并回读 manifest。
- 禁用或启用框架只影响框架适配行为，不改变真实源结构。
- 用户自定义 mapping 不需要新增代码路径即可被识别。

