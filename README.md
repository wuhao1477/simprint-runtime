# simprint-runtime

`simprint-runtime` is the standalone runtime process for Simprint.

It is responsible for the runtime-side browser orchestration and browser-kernel communication that should not remain in the open `simprint` client codebase.

Current responsibilities:

- runtime host lifecycle and IPC protocol
- browser eventbus transport and browser message forwarding
- environment start/stop, batch start/stop, CDP endpoint tracking, proxy refresh, window bounds update
- sync role management and sync event forwarding
- browser auth request/response channel

The binary runs over stdio and exposes an internal IPC protocol for the Simprint client.

## Layout

- `src/app`: runtime host, lifecycle, module orchestration, IPC dispatch
- `src/infrastructure/ipc`: client/runtime IPC protocol
- `src/infrastructure/eventbus`: runtime/browser eventbus protocol
- `src/services/environment`: browser environment runtime services
- `src/services/sync`: sync control and forwarding
- `src/services/auth`: runtime-side auth state and auth broadcast

## Development

```bash
cargo check
cargo test
```
