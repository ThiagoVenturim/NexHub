# NexHub

O NexHub e um aplicativo desktop para coordenar times de agentes autonomos de programacao em um canvas infinito, fornecendo worktrees isolados do git (floors) e um barramento local de mensagens para comunicacao entre agentes.

## Status

Status atual: pre-alpha.

## Primeiros Passos

### Pre-requisitos

- Rust (toolchain estavel com `rustfmt` e `clippy`)
- Node.js (versao 20+)
- Tauri CLI v2 (`cargo install tauri-cli --version "^2" --locked`)

### Execucao Local

Instale as dependencias da interface:

```bash
npm --prefix ui install
```

Inicie o aplicativo em modo de desenvolvimento:

```bash
cargo tauri dev
```

## Documentacao

A documentacao detalhada esta disponivel no diretorio `docs/`:

- [Arquitetura](docs/ARCHITECTURE.md)
- [Referencia da CLI](docs/CLI.md)
- [Papeis e Presets](docs/ROLES.md)
- [Especificacao de UI](docs/UI.md)

## Origem

O NexHub aproveita padroes e conceitos arquiteturais refinados em plataformas anteriores desenvolvidas no ApoIA e lochome.

## Licenca

MIT
