# Code Architecture & Verification

## 设计目标

以单 crate 内部模块组织 MVP，使 CLI、command 和 core 的职责边界清晰，并为后续拆分或扩展保留稳定入口。

## 职责边界

- CLI 层：参数解析、调用 command、格式化输出。
- command 层：业务编排。
- core 层：领域模型、路径、错误、symlink、manifest、SQLite、registry、framework 和 linkable item。

## 架构约束

- MVP 不采用 Rust workspace 和多 crate。
- 当前阶段不实现运行时插件系统。
- command 层不直接写 SQL。
- command 层不直接读写 `.agents/links.toml`。
- command 层不直接调用平台 symlink API。
- 平台条件编译限制在 symlink 或极少数路径解析代码中。

## 模块契约

core 模块职责：

- `error`：统一错误类型，包装底层错误。
- `paths`：项目根、全局数据目录、数据库路径、portable 模式、环境变量和路径转换。
- `symlink`：Provider、链接类型、链接状态、平台后端和测试 mock。
- `manifest`：`.agents/links.toml` 读写、schema 校验和状态查询。
- `db` / `migrations`：SQLite 连接、migration 和数据库诊断。
- `registry`：Linkable Item Registry 持久化访问。
- `framework`：Framework Adapter、mapping 和默认框架初始化。
- `linkable`：Linkable Item 模型、业务约束、alias、kind 检测和默认 link path。

依赖规则：

- `cli` 可以依赖 `commands`。
- `commands` 可以依赖 `core`。
- `core` 不依赖 `commands` 或 `cli`。
- `commands` 之间避免直接互相依赖，共享逻辑下沉到 `core`。

## 扩展策略

当前阶段只保证代码架构可扩展。

明确不做：

- 运行时插件系统。
- 第三方动态加载命令。
- 自定义 Linkable Item 类型。

允许扩展：

- 新 Agent 框架通过 Framework Adapter 数据扩展。
- 新 mapping 通过 `framework_mappings` 扩展。
- 新 source ownership 模式通过预留字段扩展。
- 后续可在不破坏 command 层的情况下替换或拆分 core 模块。

## 验收口径

- symlink provider 使用 mock provider 做单元测试。
- SQLite 使用临时数据库测试。
- manifest 使用临时目录测试。
- `init`、`link`、`status`、`clean` 使用集成测试覆盖核心流程。
- Windows Broker 后端使用独立集成测试，不阻塞 Unix 单元测试。
- 路径解析覆盖 Windows、macOS、Linux 默认数据目录和 portable 模式。

