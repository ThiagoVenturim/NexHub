# Brief 04: floor `packaging` (app Tauri, ligacao backend, instalador, docs)

Worktree: diretorio atual, branch `layer/packaging`, a partir de `develop` depois do scaffold.
Leia: docs/ARCHITECTURE.md secoes 2, 7, 11, 12 e docs/UI.md (assistente de primeiro uso).

## Escopo (so `src-tauri/`, `.github/`, `docs/` de uso, README)
1. `src-tauri/src/commands.rs`: um `#[tauri::command]` por comando da secao 11 mais `doctor_check` e `workspace_add`, chamando `nexhub-core`. Onde o modulo do core ainda for stub (os floors core e bus rodam em paralelo), chame a funcao publica prevista na ARCHITECTURE.md e deixe `todo!()` atras de um teste ignorado; o objetivo e a costura compilar e ficar pronta para o merge.
2. `src-tauri/src/events.rs`: emissores da secao 11 com coalescencia de 16 ms para `pty-output`.
3. `src-tauri/src/main.rs`: iniciar o servidor IPC do core em uma task tokio no `setup`, implementar o trait `Control` (agents/floors) delegando ao `SessionManager`, encerrar agentes no fechamento da janela (perguntar antes se houver agente busy).
4. Capabilities Tauri 2 (`src-tauri/capabilities/default.json`) liberando so os comandos do NexHub e o evento listen; CSP do scaffold mantido.
5. `doctor_check`: procura `git`, `agy`, `claude`, `gh`, `node` no PATH e nos caminhos comuns do Windows (`%LOCALAPPDATA%\agy\bin\agy.exe`, `C:\Program Files\GitHub CLI\gh.exe`), devolve `{tool, found, path, version, hint}`.
6. Instalador: `cargo tauri build` gerando NSIS e MSI; o NSIS coloca `nexhub.exe` (CLI) no PATH do usuario. Workflow `.github/workflows/release.yml` disparado por tag `v*` que gera os instaladores para Windows, `.dmg` macOS e `.AppImage`/`.deb` Linux como artefatos (sem publicar release; o Thiago aprova).
7. Docs de uso: `docs/GETTING_STARTED.md` (instalar, primeiro uso, criar time, mandar mensagem entre agentes) e `docs/INTEGRATIONS.md` (agy, Claude Code, como os agentes usam `nexhub ask`; secao "planejado para v0.2": MCP, `agy acp`, faccoder, Nextor, NexDeck). Escrita direta, sem cara de IA (skill escrita-humana).

## Testes obrigatorios
- `doctor_check` com PATH falso (diretorio temporario com executaveis vazios) encontra e nao encontra.
- os comandos validam parametros (nome de floor invalido -> erro `invalid_params`).
- capabilities: teste que le o JSON e confere que so comandos do NexHub estao liberados.

## Verificacao
`cargo fmt --all --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `cargo tauri build --debug`, `python scripts/check_no_emoji.py`.

## Git e PR
Igual ao brief 01, branch `layer/packaging`.

## Nao fazer
Nao editar `crates/nexhub-core` alem de expor `pub` algo que falte (descreva no summary). Nao editar `ui/`. Nao publicar release, nao criar repo.
