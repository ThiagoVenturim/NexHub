# Brief 01: floor `core` (PTY, sessoes, banco, roles)

Worktree: o seu diretorio atual, branch `layer/core`, criada a partir de `develop` depois do scaffold.
Leia: docs/ARCHITECTURE.md secoes 3, 4, 5, 10, 12, 13 e docs/ROLES.md inteiro.

## Escopo (so `crates/nexhub-core`, modulos `config`, `db`, `model`, `pty`, `sessions`, `roles`, `floors`)
1. `config.rs`: `Paths::resolve()` honrando `NEXHUB_HOME`; cria o diretorio; `Token::load_or_create()` (32 bytes aleatorios via `getrandom`, gravados em hex). Permissao do arquivo: em Unix `0o600`; no Windows basta criar dentro do perfil do usuario (`%APPDATA%`), que ja e privado, e documentar isso no comentario do codigo.
2. `db.rs`: `Db::open(path)` com WAL e foreign keys, migracao 1 exatamente como na secao 4, `user_version`. Funcoes tipadas CRUD para cada tabela (`insert_workspace`, `list_agents(workspace)`, `set_agent_state`, `insert_message`, `mark_message`, `next_queued_for(agent)`, `upsert_note`, etc.). Nada de SQL montado com format!: sempre parametros.
3. `model.rs`: structs serde para todas as tabelas, enums para `AgentState`, `MessageKind`, `MessageState`, `FloorStatus` com `as_str`/`FromStr`.
4. `pty.rs` e `sessions.rs` conforme secao 5, incluindo ring buffer de 256 KiB, broadcast de saida, bracketed paste, `IdleDetector` com implementacao por tempo.
5. `roles.rs`: carregar e validar presets e roles (ordem de busca, `include_str!` dos built-ins, variaveis `${...}`, todos os erros listados em ROLES.md com teste para cada um). Funcao `build_spawn_spec(preset, role, ctx) -> SpawnSpec`.
6. `floors.rs` conforme secao 10, com `run_git` unico e validacao de nome e caminho.

## Testes obrigatorios
- migracao em banco novo e reabrir sem refazer; check constraints rejeitam estado invalido.
- spawn do `echo_agent`, escrever "oi", ler "echo: oi"; resize nao falha; kill encerra; ring buffer devolve saida apos o fim.
- idle detector: busy -> idle apos `idle_ms` sem saida (use tempo curto no teste).
- roles: cada erro de validacao; resolucao de variaveis; built-ins carregam.
- floors: em um repo git temporario (git init + commit), criar floor, listar, recusar remover sujo, remover limpo, branch continua existindo; nome invalido (`../x`, `A B`) rejeitado.

## Verificacao
`cargo fmt --all --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `python scripts/check_no_emoji.py`.

## Git e PR
Commits pequenos em `layer/core`. Push da branch e PR para `develop` somente se o repo tiver remote `origin`; se nao tiver, termine com status `pr_ready`, `pr` vazio e `branch: layer/core`, e o Conductor faz o merge local.

## Nao fazer
Nao mexer em `bus.rs`, `ipc.rs`, `routines.rs`, `quota.rs`, `nexhub-cli`, `src-tauri` ou `ui/` (outros floors). Se precisar de um tipo novo em `model.rs` para eles, adicione e descreva no summary.
