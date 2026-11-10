# NexHub installer and project bundle (planejado, pos-MVP)

O NexHub e distribuido como um app instalavel (ja previsto: NSIS/MSI no Windows, .dmg no Mac, AppImage/.deb no Linux; ver docs/ARCHITECTURE.md e o floor packaging). Esta nota descreve a camada de bundle, que vem depois do MVP.

## Objetivo
Ao baixar e instalar o NexHub, a pessoa nao precisa clonar nada na mao. No primeiro uso, o assistente oferece trazer os projetos companheiros e liga-los ao NexHub.

## Primeiro uso: trazer projetos
O assistente de primeiro uso (ver docs/UI.md) ganha um passo "Projetos":
- Lista os projetos oficiais que o NexHub sabe instalar: faccoder, Nextor, askdash, Support Engine. Cada um com uma linha do que faz e o tamanho aproximado.
- A pessoa marca os que quer. Para cada marcado, o NexHub:
  1. confere git e as ferramentas necessarias (Python para faccoder/Nextor etc.), usando a logica do `nexhub doctor`;
  2. clona o repo publico de ThiagoVenturim em uma pasta escolhida (ou baixa um release quando houver);
  3. roda o setup do projeto (venv e instalacao dos pacotes, seguindo o getting-started de cada um) com barra de progresso;
  4. registra o projeto como workspace no NexHub, pronto para abrir floors.
- Nada e instalado sem a pessoa marcar. Tudo com log visivel. Se um passo falhar, mostra o erro e como resolver, sem travar os outros.

## Opcao offline / sem git
- Quando houver releases, oferecer baixar o pacote do projeto em vez de clonar, para quem nao tem git.
- Um modo "so o NexHub" para quem nao quer nenhum projeto junto.

## Seguranca
- So instala repos da conta oficial ThiagoVenturim (lista fixa no codigo); nunca uma URL arbitraria vinda de fora.
- Nenhum segredo embutido no instalador. Tokens que cada projeto precise ficam no .env local do projeto, criado pelo setup, nunca no NexHub.
- A pessoa aprova cada clone/instalacao; o NexHub mostra o destino no disco antes.

## Dependencia
Depende do control plane do NexHub (workspaces, doctor) e das integracoes NexLink (para o NexHub conversar com os projetos depois de instalados). Vira issues quando o repo NexHub for publicado.
