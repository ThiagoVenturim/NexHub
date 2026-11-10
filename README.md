# NexHub

NexHub is a desktop application for coordinating teams of autonomous coding agents on an infinite canvas, providing isolated git worktrees (floors) and a local message bus for inter-agent communication.

## Status

Current status: pre-alpha.

## Getting Started

### Prerequisites

- Rust (stable toolchain with `rustfmt` and `clippy`)
- Node.js (version 20+)
- Tauri CLI v2 (`cargo install tauri-cli --version "^2" --locked`)

### Running Locally

Install frontend dependencies:

```bash
npm --prefix ui install
```

Launch the application in development mode:

```bash
cargo tauri dev
```

## Documentation

Detailed documentation is available in the `docs/` directory:

- [Architecture](docs/ARCHITECTURE.md)
- [CLI Reference](docs/CLI.md)
- [Roles and Presets](docs/ROLES.md)
- [UI Specification](docs/UI.md)

## Origin


## License

MIT
