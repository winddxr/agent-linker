# Agent Linker (aglink)

**Agent Linker** (`aglink`) 是一个基于 Rust 开发的 CLI 工具，专为 AI Agent 的指令、技能 (Skills) 和资源 (Resources) 提供基于受控 Symlink (符号链接) 的管理方案。

通过构建中心化的 Agent 资源仓库，并将它们安全、高效地链接到各个独立的工程目录中，Agent Linker 帮助开发者轻松实现跨项目的上下文同步与框架兼容。

## 🌟 Advantages (优势)

- **Safety First (安全第一)**: 严格区分真实文件与符号链接。程序永远不会覆盖或删除用户的真实文件与目录。即便在使用 `--force` 时，也仅限于替换错误的符号链接。
- **Cross-platform Consistency (跨平台一致性)**: 在 Linux / macOS 上使用原生 Symlink；在 Windows 上提供专属 Broker 服务机制，彻底告别频繁的 UAC 提权弹窗。
- **Centralized Management (中心化管理)**: 全局使用 SQLite 存储 Registry、Config、Framework 和 Group 数据，支持按分组管理技能与资源，一处修改，处处生效。
- **Framework Adapter (框架适配)**: 自动将统一管理的真实源映射投射为各种 AI Agent 框架（如 Claude 等）所要求的特定 Format (格式) 和路径。
- **High Performance (高性能)**: 基于 Rust 构建，操作幂等，执行极速。

## 🎯 Significance (意义)

随着 AI Agent 技术的普及，开发者通常需要在多个项目之间共享相同的提示词 (Prompts)、脚本技能或背景知识文档。传统做法往往依赖复制粘贴，导致版本碎片化，更新极其痛苦。

**Agent Linker** 的诞生正是为了解决这一痛点：
1. **Definition (定义) 统一**: 统一提供全局的 Skill 与 Resource 抽象标准，构建专属的 AI 知识库。
2. **零冗余**: 利用操作系统的符号链接特性，实现多工程复用而不在磁盘上产生副本。
3. **消除 Windows 权限痛点**: 长期以来，Windows 下创建符号链接需要管理员权限，破坏了自动化的流畅体验。本项目结合专用的本地服务进程，为 Windows 开发者带来了丝滑的链接体验。

## 📦 Installation & Configuration (安装配置)

### 1. 基础安装 (所有系统)

确保你的系统已安装 [Rust 与 Cargo](https://rustup.rs/) 环境。在本项目根目录执行：

```bash
cargo install --path .
```
*(注：后续发布到 crates.io 后可通过 `cargo install aglink` 安装)*

### 2. 各平台环境配置

#### 🍎 macOS / 🐧 Linux
系统原生支持无特权的符号链接操作，无需额外配置，开箱即用。

#### 🪟 Windows (特别说明)
在 Windows 系统下，创建符号链接默认需要管理员权限。为了保证安全与自动化流程的顺畅，**Windows 用户必须安装配套的 Broker 服务**。

1. 访问并下载 Windows 符号链接代理服务：
   👉 [win-symlinks (GitHub)](https://github.com/winddxr/win-symlinks)
2. 按照该仓库的说明安装并启动后台服务。
3. Agent Linker 会自动通过安全的本地 IPC 与该 Broker 服务通信，无缝完成符号链接的创建。

## 🚀 Quick Start (快速开始)

初始化项目并创建 `.agents/` 管理目录与配置文件：

```bash
aglink init
```

> **注意**: 完整命令使用方法与用途分类，请查阅详细的 [CLI Command Reference (命令行指南)](CLI_REFERENCE.md)。同时，也可通过 `aglink --help` 随时获取内建帮助。

## 📖 Architecture & Contract (架构与契约)

项目的设计理念与模块拆分严格遵循了以下原则。对于开发与贡献者，核心的 Acceptance Criteria (验收标准) 请参阅 `dev-docs/architecture.md`。

### Key Terms (关键术语)
- **Symlink**: 操作系统级符号链接，非快捷方式，非硬链接。
- **Agent Skill**: 按用途命名的目录，包含 AI Agent 可使用的提示词、脚本或文档等资源。
- **Linkable Item**: Skill 与 Resource 的统一抽象，是工具管理的基本链接单元。

### 核心模块
- **Symlink Core**: 封装操作系统差异，保障创建与销毁的幂等性。
- **Global Store & Registry**: 全局 SQLite 状态存储。
- **CLI Command Surface**: 用户交互边界，输出结构化诊断信息。
