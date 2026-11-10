# Referencias e diferencial do NexHub

Projetos parecidos (open source e privados), o que o NexHub faz de diferente e o que da para aprender com eles. Pesquisa de 2026-11-08; confira cada projeto antes de usar como referencia, porque mudam rapido.

## Parecidos

### Open source
- **TermCanvas**: canvas infinito com terminais (shell, lazygit, tmux) e agentes de IA lado a lado.
- **OpenCove**: canvas infinito para Claude Code, Codex, terminais, tarefas e notas (TypeScript/Electron).
- **ccanvas**: app Tauri com varios workspaces e "agent flows" (setas de logica entre agentes).
- **Agent Bridge**: canvas estilo Miro para agentes e terminais, inclusive entre maquinas; MIT.
- **OrkestrAI**: desktop (macOS/Windows/Linux) onde se solta agentes no canvas, conecta e eles colaboram.
- **Emdash**: desktop para rodar varios agentes de codigo em paralelo, cada um isolado no seu git worktree.
- **Open Swarm**: orquestrador local para coordenar varios agentes em paralelo.

### Privados / fechados
- **Maestri**: canvas de orquestracao de agentes; "puxe uma linha entre dois terminais e eles colaboram". E o produto que usamos antes e cuja licenca expirou, o que motivou o NexHub.

## Nosso diferencial
- **Floors = git worktree de verdade**, criados e removidos pela ferramenta, com branch `layer/*`. A isolacao por worktree (como o Emdash) e nativa e ligada ao fluxo de PR.
- **Barramento de mensagens entre agentes com CLI** (`nexhub ask/notify/broadcast`): o proprio agente chama pela linha de comando, nao so uma seta no canvas.
- **Roles e presets como YAML versionado** no repo (.nexhub/), nao config escondida. Da para revisar e compartilhar.
- **Supervisor de cota**: detecta "esperando reset de cota" e retoma sozinho, pensado para rodar barato por muito tempo.
- **Rust + Tauri**: um app leve e multiplataforma com terminais reais (portable-pty/ConPTY), SQLite local, sem nuvem.
- **Integra com o nosso ecossistema**: faccoder (pipelines), Nextor (avisos falados), NexDeck (celular), e o instalador que traz os projetos (docs/INSTALLER.md).
- **Open source de verdade (MIT)**, ao contrario do Maestri fechado.

## O que melhorar aprendendo com eles
- **Entre maquinas** (Agent Bridge): hoje o NexHub e local; avaliar coordenar agentes em outra maquina por VPN.
- **Agent flows visuais** (ccanvas/OrkestrAI): alem do barramento por CLI, desenhar o fluxo (condicoes, setas) no canvas.
- **Daemon sempre pronto** (AgentGrid): agentes "quentes" prontos para receber tarefa sem subir do zero.
- **Suporte a mais CLIs** desde o inicio (Codex, outros), como o OpenCove, para nao ficar preso a agy/Claude.
- **Templates de time** prontos (como varios deles), para o primeiro uso ja vir com um time util.

## Sources
- https://alternativeto.net/software/streamdeck/ (panorama)
- https://www.sourcepulse.org/projects/28826990 (TermCanvas)
- https://awesome.ecosyste.ms/projects/github.com%2FDeadWaveWave%2Fopencove (OpenCove)
- https://github.com/DevoidSloth/ccanvas (ccanvas)
- https://www.producthunt.com/products/agent-bridge-2 (Agent Bridge)
- https://www.producthunt.com/products/orkestrai (OrkestrAI)
- https://www.themaestri.app/en (Maestri)
