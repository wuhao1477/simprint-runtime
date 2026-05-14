<div align="center">
  <h1>Simprint Runtime</h1>
  <p>Standalone runtime process for Simprint browser orchestration, IPC, and runtime-side browser-kernel coordination.</p>
  <p>
    <img alt="License AGPLv3" src="https://img.shields.io/badge/license-AGPLv3-67e8f9?style=flat-square&labelColor=0f172a" />
    <img alt="Language Rust 2024" src="https://img.shields.io/badge/language-Rust%202024-f97316?style=flat-square&labelColor=0f172a" />
    <img alt="Platform Windows" src="https://img.shields.io/badge/platform-Windows-60a5fa?style=flat-square&labelColor=0f172a" />
    <img alt="IPC stdio" src="https://img.shields.io/badge/ipc-stdio-f59e0b?style=flat-square&labelColor=0f172a" />
  </p>
  <p>
    <strong>English</strong> | <a href="./README.zh-CN.md">简体中文</a>
  </p>
</div>

---

## Introduction

Simprint Runtime is the standalone runtime process used by Simprint to host browser-runtime orchestration outside the main client repository surface. It runs as a separate binary over stdio and exposes an internal IPC protocol for the Simprint client.

It is intended to isolate runtime-side browser control, browser-kernel communication, and environment lifecycle management into a narrower process boundary that can evolve independently from the frontend and higher-level client UI.

## Why Simprint Runtime?

Some runtime concerns are better kept outside the open client shell:

- Browser orchestration and runtime lifecycle logic need a dedicated process boundary.
- IPC contracts between the client and runtime need a stable transport surface.
- Browser event forwarding, environment control, and sync coordination benefit from a focused runtime service layer.
- Windows-targeted runtime packaging should be releasable independently from the rest of the client stack.

Simprint Runtime is being shaped around those constraints: a smaller Rust binary, a clear IPC-facing scope, and a narrower runtime surface that can move independently as the surrounding Simprint architecture evolves.

## Features

- **Runtime host lifecycle**: Manages runtime startup, shutdown, module initialization, and internal orchestration.
- **Client/runtime IPC**: Exposes a stdio-based internal IPC protocol for the Simprint client.
- **Browser eventbus transport**: Forwards runtime/browser eventbus messages and runtime-side browser communication.
- **Environment control**: Handles environment start/stop, batch operations, CDP endpoint tracking, proxy refresh, and window bounds updates.
- **Sync coordination**: Manages sync roles and forwards sync-related events.
- **Auth channel support**: Handles browser auth request/response flow propagation on the runtime side.
- **Windows release pipeline**: Builds a Windows runtime binary and publishes release metadata through GitHub Actions.

## Status

Simprint Runtime is part of the broader Simprint open-source restructuring effort. Its current role is to hold runtime-side orchestration logic that should not remain directly embedded in the main client codebase.

Some internal module boundaries and IPC contracts may continue to evolve as the surrounding client and browser-kernel architecture is further separated and stabilized. The current repository should be treated as an actively reorganizing runtime layer rather than a fully settled public SDK surface.

## Contributing

This repository is still in an open-source refactoring phase, but issues and pull requests are welcome.

High-value contribution areas include:

- IPC protocol clarity and regression coverage
- Runtime lifecycle correctness and failure handling
- Windows packaging and release automation improvements
- Documentation around runtime responsibilities and client integration

## License

This project is licensed under the GNU Affero General Public License v3.0 (AGPLv3).

If you want to use Simprint Runtime in a way that does not comply with the AGPLv3 obligations, including distributing modified versions or providing modified versions as a closed-source service, please contact us for a commercial license.
