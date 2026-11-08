# Brief 00: scaffold do NexHub

Repo: C:\Users\thiag\projetos-oss\nexhub (branch `develop`, ainda sem commits e sem remote).
Leia antes: docs/ARCHITECTURE.md (secoes 2, 12, 13), docs/ROLES.md, docs/UI.md, docs/CLI.md.

## Objetivo
Deixar o esqueleto compilando e testando, para que 4 floors trabalhem em paralelo depois sem conflito. Nao implemente features: so estrutura, stubs e CI.

## Entregas
1. `Cargo.toml` raiz como workspace com membros `crates/nexhub-core`, `crates/nexhub-cli`, `src-tauri`. `edition = "2021"`, `resolver = "2"`. Perfil release com `lto = "thin"`.
2. `crates/nexhub-core`: `lib.rs` declarando os modulos da secao 2 (`db, model, pty, sessions, floors, roles, bus, routines, quota, ipc, config`), cada um com um comentario de uma linha e um teste trivial. Dependencias ja declaradas: `rusqlite` (feature `bundled`), `portable-pty`, `tokio` (rt-multi-thread, macros, sync, net, time, io-util), `serde`, `serde_json`, `serde_yaml`, `ulid`, `directories`, `subtle`, `shell-words`, `cron`, `regex`, `thiserror`, `tracing`, `base64`. Dev: `tempfile`. `src/bin/echo_agent.rs` (le linhas do stdin e escreve `echo: <linha>`).
3. `crates/nexhub-cli`: binario `nexhub` com `clap` derive e TODOS os subcomandos de docs/CLI.md definidos; cada um imprime `not implemented yet` e sai com codigo 10. `nexhub --version` funciona.
4. `src-tauri`: app Tauri 2 minimo (`tauri = "2"`, `tauri-build`) que abre uma janela "NexHub" carregando `ui/`. `tauri.conf.json` com identifier `io.github.thiagoventurim.nexhub`, CSP restrito (`default-src 'self'; style-src 'self' 'unsafe-inline'`), sem URLs remotas, bundle targets `nsis` e `msi` no Windows. Icone placeholder gerado (formas simples, sem marca de terceiros).
5. `ui/`: Vite + TypeScript (strict), `@tauri-apps/api` v2, `@xterm/xterm`, `@xterm/addon-fit`, `vitest`. `src/main.ts` mostra o layout vazio de docs/UI.md (barra superior, sidebar, canvas com grade de pontos, log de mensagens recolhido) usando os tokens de cor de docs/UI.md em `src/styles/tokens.css`. Um teste vitest qualquer passando. Scripts: `dev`, `build`, `test`.
6. `presets/agy.yaml`, `presets/agy-auto.yaml`, `presets/claude.yaml`, `presets/shell.yaml` e `roles/floor-builder.yaml`, `roles/reviewer.yaml`, `roles/conductor.yaml` conforme docs/ROLES.md.
7. `.github/workflows/ci.yml` com a matriz da secao 13 (windows, ubuntu, macos). No ubuntu instale as libs do Tauri (`libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`).
8. `.gitignore` (target/, node_modules/, ui/dist/, .env, *.db, .nexhub-floors/), `README.md` em ingles curto (o que e, status "pre-alpha", como rodar `npm --prefix ui install` + `cargo tauri dev`, link para docs/) e `README.pt-BR.md` equivalente. `CONTRIBUTING.md` curto com as regras (sem emoji, sem coautoria de IA, hooks).
9. `rust-toolchain.toml` com `channel = "stable"` e componentes `rustfmt`, `clippy`.

## Verificacao (todos precisam passar antes de commitar o ultimo commit)
- `cargo fmt --all --check`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`
- `npm --prefix ui ci` (gere o package-lock.json) e `npm --prefix ui test`
- `cargo build -p nexhub-cli` e `target/debug/nexhub --version`
- `python scripts/check_no_emoji.py`
- `cargo tauri build --debug` (instale o CLI com `cargo install tauri-cli --version "^2" --locked` se faltar). Se falhar por falta de ferramenta do sistema, registre em `blockers` e continue com o resto.

## Git
- `git config core.hooksPath .githooks` (ja configurado, confira).
- Commits pequenos em `develop` mesmo (este repo ainda nao tem remote; nao crie remote, nao faca push). Mensagens conventional commits em ingles: `chore: cargo workspace`, `feat(ui): empty layout`, etc.

## Nao fazer
- Nao implementar PTY, bus, IPC, banco: so stubs.
- Nao criar repo no GitHub, nao fazer push.
- Nao copiar visual, nome ou icones de outro produto.

Status final esperado: `done` com o resumo e a saida dos comandos de verificacao em `tests`.
