# Brief 02: floor `bus` (IPC, mensagens, rotinas, cota, CLI)

Worktree: diretorio atual, branch `layer/bus`, a partir de `develop` depois do scaffold.
Leia: docs/ARCHITECTURE.md secoes 6, 7, 8, 9, 12, 13 e docs/CLI.md inteiro.

## Dependencia com o floor core
O floor `core` implementa `db`, `model`, `pty`, `sessions` em paralelo. Para nao esperar, programe contra traits que voce define em `bus.rs`:
```rust
pub trait AgentDirectory: Send + Sync { fn find(&self, ws: &str, name: &str) -> Option<AgentRef>; fn list(&self, ws: &str) -> Vec<AgentRef>; fn state(&self, id: &str) -> AgentState; }
pub trait Deliverer: Send + Sync { fn deliver(&self, agent_id: &str, text: &str) -> Result<(), BusError>; }
pub trait MessageStore: Send + Sync { /* insert, mark, next_queued_for, find_reply */ }
```
Nos testes use implementacoes em memoria. Os tipos `AgentState`, `MessageKind`, `MessageState` ficam em `model.rs`: se ainda nao existirem no seu branch, crie-os exatamente com os nomes da secao 4 (o merge vai unificar).

## Escopo
1. `bus.rs`: ask/notify/broadcast/reply/inbox conforme secao 6, envelope de texto exato, fila FIFO por destino, entrega quando o destino fica ocioso, timeout padrao 30 min, permissao por workspace.
2. `ipc.rs`: servidor TCP em `127.0.0.1:0`, grava `socket.json`, JSON por linha, autenticacao por token com `subtle`, fail closed, todos os metodos da secao 7 roteados para traits (os de floors/agents chamam um trait `Control` que o app implementa). Limite de 1 MiB por linha.
3. `routines.rs`: parser da gramatica da secao 8 com testes, calculo de `next_run_at`, tick, execucao de comando sem shell (`shell-words`), politica de execucao perdida.
4. `quota.rs`: watcher de padroes por preset, estados, parse de reset, retomada com "continue exactly where you stopped".
5. `crates/nexhub-cli`: implementar todos os comandos de docs/CLI.md sobre o socket, `--json`, codigos de saida, `@arquivo` e `-` para stdin, `doctor` e `init` (o `init` copia os presets built-in).

## Testes obrigatorios
- ask/reply ida e volta com duas entregas falsas; fila enquanto busy; timeout; broadcast nao entrega ao remetente; cross-workspace recusado.
- IPC: token errado -> `unauthorized` e conexao fechada; sem arquivo de token -> recusa tudo; linha maior que 1 MiB -> erro; bind so em loopback (verifique o endereco local do listener).
- routines: cada forma da gramatica, `daily` atravessando meia-noite, comando com aspas dividido certo, nada passa por shell.
- quota: padrao do agy dispara `waiting_quota`; reset em minutos agenda a retomada.
- CLI: teste de integracao com servidor IPC falso em processo (`assert_cmd`), codigos de saida 2 (sem app), 3, 4, 5.

## Verificacao
`cargo fmt --all --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `python scripts/check_no_emoji.py`.

## Git e PR
Igual ao brief 01, branch `layer/bus`.

## Nao fazer
Nao editar `pty.rs`, `sessions.rs`, `db.rs`, `roles.rs`, `floors.rs`, `src-tauri`, `ui/`.
