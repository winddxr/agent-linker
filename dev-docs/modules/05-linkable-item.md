# Linkable Item

## 设计目标

将 Skill 和 Resource 统一建模为 Linkable Item，使注册、分组、链接、状态检查和清理共享一致的数据模型。

## 职责边界

- 定义 Linkable Item 领域模型。
- 处理 Skill 与 Resource 的业务约束。
- 检测 source 的 file / directory kind。
- 计算默认链接路径和链接名。
- 管理来源记录，不接管源内容。

## 架构约束

- Registry 存储在全局 SQLite。
- MVP 采用分散管理模式。
- `source_path` 永远记录绝对路径。
- 项目 manifest 中的 `link_path` 使用相对项目根目录的路径。
- MVP 阶段 symlink 目标使用绝对 source path。

## 接口契约

Linkable Item 至少包含：

- `id`
- `name`
- `alias / link_name`
- `type`
- `kind`
- `source_path`
- `source_type`
- `source_ownership`
- `default_target_dir`
- `description`
- `repo_url`
- `repo_commit`
- `created_at`
- `updated_at`

Skill 约束：

- 类型为 `skill`。
- source 必须是目录。
- source 必须包含非空 `SKILL.md`。
- 默认链接到 `.agents/skills/<link_name>`。

Resource 约束：

- 类型为 `resource`。
- source 可以是文件或目录。
- 目标目录由注册或链接时指定。
- 默认链接名保持源文件或源目录原名称。

## 关键决策

- 同名不同源默认报错，提示用户改名或指定 alias。
- 注册时自动检测 kind，用户不手动指定。
- 链接时实际 source kind 与 Registry 记录不一致时必须报错。
- `source_ownership = external` 是 MVP 唯一实现模式。
- `managed`、`repo_url`、`repo_commit` 等字段只作为未来集中管理或自动更新能力预留。
- 删除 Registry 项不自动删除已链接项目中的 symlink。

## 验收口径

- Skill 注册会校验目录和非空 `SKILL.md`。
- Resource 注册能识别文件和目录。
- Linkable Item 能稳定计算项目相对 link path。
- 已链接项目的清理由 `unlink` 或 `clean` 基于 manifest 完成。
- `aglink` 不复制、不移动、不删除 source 内容。

