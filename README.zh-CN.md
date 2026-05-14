<div align="center">
  <h1>Simprint Runtime</h1>
  <p>面向 Simprint 浏览器编排、IPC 通信与运行时侧 browser-kernel 协调的独立运行时进程。</p>
  <p>
    <img alt="License AGPLv3" src="https://img.shields.io/badge/license-AGPLv3-67e8f9?style=flat-square&labelColor=0f172a" />
    <img alt="Language Rust 2024" src="https://img.shields.io/badge/language-Rust%202024-f97316?style=flat-square&labelColor=0f172a" />
    <img alt="Platform Windows" src="https://img.shields.io/badge/platform-Windows-60a5fa?style=flat-square&labelColor=0f172a" />
    <img alt="IPC stdio" src="https://img.shields.io/badge/ipc-stdio-f59e0b?style=flat-square&labelColor=0f172a" />
  </p>
  <p>
    <a href="./README.md">English</a> | <strong>简体中文</strong>
  </p>
</div>

---

## Introduction

Simprint Runtime 是 Simprint 使用的独立运行时进程，用于承载主客户端仓库之外的浏览器运行时编排能力。它以单独二进制形式通过 stdio 运行，并向 Simprint 客户端暴露内部 IPC 协议。

它的目标是把运行时侧浏览器控制、browser-kernel 通信和环境生命周期管理隔离到更清晰的进程边界中，使这部分能力可以独立于前端和上层客户端 UI 持续演进。

## Why Simprint Runtime?

有些运行时能力更适合放在开放客户端壳层之外：

- 浏览器编排和运行时生命周期逻辑需要独立的进程边界。
- 客户端与运行时之间的 IPC 协议需要稳定的传输面。
- 浏览器事件转发、环境控制和同步协调更适合放在聚焦的运行时服务层中。
- 面向 Windows 的运行时打包能力需要能独立于客户端其他部分单独发布。

Simprint Runtime 正是在这些约束下组织起来的：一个更小的 Rust 二进制、一个清晰的 IPC 作用面，以及一个能够随着 Simprint 整体架构演进而独立调整的运行时层。

## Features

- **运行时宿主生命周期**：负责运行时启动、关闭、模块初始化和内部编排。
- **客户端/运行时 IPC**：通过基于 stdio 的内部 IPC 协议与 Simprint 客户端通信。
- **浏览器 eventbus 传输**：转发 runtime/browser eventbus 消息以及运行时侧浏览器通信。
- **环境控制**：处理环境启动/停止、批量操作、CDP endpoint 跟踪、代理刷新和窗口边界更新。
- **同步协调**：管理 sync 角色并转发 sync 相关事件。
- **认证通道支持**：承载运行时侧浏览器认证请求/响应流程的传递。
- **Windows 发布流水线**：通过 GitHub Actions 构建 Windows 运行时二进制并发布对应的版本元数据。

## Status

Simprint Runtime 是 Simprint 整体开源重构工作的一部分。它当前的角色是承载那些不应继续直接嵌入主客户端代码库中的运行时编排逻辑。

随着周边客户端与 browser-kernel 架构进一步拆分和稳定，仓库内部的一些模块边界和 IPC 协议仍可能继续演化。当前这个仓库应被视为一个仍在持续重组中的运行时层，而不是完全稳定下来的公共 SDK 表面。

## Contributing

这个仓库目前仍处于开源重构阶段，但已经欢迎通过 Issue 和 Pull Request 参与改进。

当前更有价值的贡献方向包括：

- IPC 协议清晰度和回归覆盖
- 运行时生命周期正确性与失败处理
- Windows 打包与发布自动化改进
- 运行时职责边界和客户端集成文档完善

如果你准备快速建立上下文，建议先看这些入口：

- `src/main.rs`
- `src/lib.rs`
- `src/app`
- `src/services`
- `src/infrastructure`
- `scripts/prepare-version.mjs`
- `scripts/generate-latest-json.mjs`
- `Cargo.toml`

## License

本项目采用 GNU Affero General Public License v3.0 (AGPLv3) 进行许可。

如果你希望在不履行 AGPLv3 义务的前提下使用 Simprint Runtime，包括分发修改版本或以闭源服务形式提供修改版本，请联系获取商业许可。
