# Diário do projeto (temporário)

Registro de requisitos, observações e decisões do GLUE 5. **Não incluir dados pessoais, identificadores ou informações de conta.** Este arquivo é temporário e deve ser removido ao final do trabalho, quando solicitado.

## 2026-10-27 — Diagnóstico Android e regressões

### Observações reportadas
- Ao tentar iniciar o jogo, o APK apresenta `wasm trap: unreachable instruction executed`.
- A tela de controles não mostra as novidades esperadas.
- Em Settings → Game, desapareceram as opções de importar save e copiar o relatório de diagnóstico.
- Não há uma opção acessível no app para obter `game.log`; a falha impede depurar pelo próprio GLUE 5.
- O APK em questão foi compilado pelo usuário; não presumir que veio do workflow do repositório.

### Requisitos e sugestões
- Desenvolver uma ferramenta para Windows que use ADB por Wi-Fi para depurar especificamente `local.glue5` e gere um arquivo de diagnóstico fácil de revisar e enviar.
- Capturar logs do processo GLUE 5 durante uma reprodução iniciada pelo usuário; não coletar o logcat inteiro do dispositivo por padrão.
- Evitar exportar serial do aparelho, IP, código de pareamento, contas ou outros identificadores. Informar que logs podem conter caminhos e permitir revisão antes do envio.
- Preservar as opções já existentes. HUD, touchscreen e importação de save/jogo só contam como concluídos se as funções anteriores continuarem operando.
- Manter um diário das ordens, sugestões, observações e decisões relevantes ao projeto, sem dados pessoais; apagar este diário no encerramento do projeto, mediante solicitação.

### Acompanhamento técnico
- No código, o layout touchscreen `Modern` passou a ser o padrão em dispositivos móveis; ações secundárias ficam no botão `More`. `Classic` mantém as ações mais visíveis. Tornar essa diferença clara e verificar se ela explica controles percebidos como ausentes.
- O botão de diagnóstico Android estava condicionado a `started.is_none()` e foi movido para continuar visível depois que o jogo inicia. Para cobrir o encerramento por trap fatal, o código atual também monta o relatório em `stopped()` e solicita sua gravação/cópia antes de sair; isso ainda precisa de build e teste em aparelho.
- A integração GitHub não permitiu disparar o workflow Android (`403 Resource not accessible by integration`). O usuário vai iniciar o workflow manualmente; não afirmar que um APK foi gerado nesta sessão.
- O trap ainda não foi reproduzido nem explicado. O relatório interno conserva o detalhe de trap disponível no runtime, mas não substitui a captura Shizuku filtrada para o PID do GLUE 5, se faltar contexto.
- Criado um protótipo Windows em `tools/glue5-debug/`: pareamento/conexão ADB por Wi-Fi, captura do processo `local.glue5` e exportação de ZIP. Sem PID, o protótipo aborta a captura com uma mensagem clara e não usa logcat amplo; ainda não foi executado em Windows nem validado contra um aparelho. `game.log` é opcional porque ADB pode não ter acesso.

## 2026-10-27 — Captura automática e seletor de jogo

- O usuário propôs Shizuku/ADB Wi-Fi no próprio aparelho. Primeiro foi escolhida uma captura interna do trap; naquele momento, pediu uma captura ampla via Shizuku. Essa abrangência foi corrigida depois: o requisito vigente é filtrar pelo PID do GLUE 5, sem logs de outros apps. A captura interna continua como relatório curto.
- Após reinstalar e limpar os dados, `Find files` não abriu/selecionou um jogo. Corrigir erros silenciosos, não cair no seletor desktop no Android, limpar resultados antigos e deixar explícito por que a seleção falhou.
- O usuário vai executar **Build → Run workflow → Android** manualmente após as mudanças; não alterar pipeline/assinatura e não alegar que o build em aparelho passou até receber o resultado.
- Implementação local em andamento: o hook de trap conserva código/linha/frames nativos; `stopped()` gera o relatório automaticamente; Android salva `glue5-support-report.txt` e tenta copiar para clipboard antes de sair. `Find files` agora deve retornar sem fallback desktop no Android e registra resultados/cancelamentos. Ainda sem build de verificação.
- Manter como prioridade recuperar inicialização do jogo e opções existentes antes de considerar HUD/touch/importações concluídos.
- Decisão inicial de diagnóstico (posteriormente substituída): usar Shizuku para uma captura ampla na recuperação da sessão. A correção vigente, após o arquivo de cerca de 35 MB, é capturar somente o PID do processo GLUE 5. Permanecem o consentimento explícito, a autorização Shizuku e a ação manual de recuperação.
- Implementação inicial (escopo amplo, agora substituída): Shizuku/UserService com autorização explícita e confirmação após falha. A execução de `logcat -d -b all -v threadtime` sem filtro foi removida; o código vigente envia o PID do GLUE 5 para snapshot e captura live. O GLUE 5 não cria nem lê códigos de pareamento; o usuário inicia/pareia o serviço no próprio Shizuku.

### Retrospectiva — regressão de Find files
- Sim: alterei um fluxo Android que já existia. Em `e6c5c3c`, `Action::FindGame` deixou de chamar diretamente `find_game(None)` (revarredura dos caminhos conhecidos) e passou a abrir um picker de pasta; para pastas fora do diretório do app, esse caminho exige o acesso especial `MANAGE_EXTERNAL_STORAGE`. Uma reinstalação limpa revoga essa autorização e a aplicação precisa guiá-la de novo.
- Em `b852d82`, também removi o fallback para a revarredura antiga quando a chamada/picker Android falha. Isso deixou o novo caminho menos tolerante e pode explicar o botão não encontrar o jogo quando a permissão não foi concedida ou a Activity/picker não abriu.
- Não posso afirmar qual desses casos ocorreu no aparelho sem o logcat: o root cause em runtime continua desconhecido. O erro foi não preservar e testar o comportamento que já funcionava antes de substituir o fluxo.
- Reintroduzi no working tree a revarredura dos caminhos conhecidos quando o picker falha ou retorna recusa; o picker segue como caminho principal. Ainda não compilado/testado em dispositivo.
- A Build run 40452436632 falhou em `:compileReleaseAidl`: `ILogcatCaptureService.aidl` atribuía transação só a `destroy()` e não a `captureLogcat()`. Corrigi o contrato atribuindo `= 1` a `captureLogcat()` (commit `4b12ae0`); falta rerodar o Build para confirmar a compilação.

### Navegação do repositório por agentes de código
- O usuário esclareceu que a ideia de remover ou reduzir alvos iOS/macOS/Windows/Linux é para facilitar a busca e a análise do repositório por LLMs, não para mudar o produto suportado. A alternativa discutida foi melhorar o mapa de arquitetura/instruções do repositório e indicar código legado, preservando o servidor Windows e evitando apagar código compartilhado sem necessidade. Ainda sem decisão de remover plataformas.

## 2026-10-27 — Material 3 Expressive e captura manual de logcat

### Escopo autorizado
- Antes da futura reformulação visual inspirada em GTA V, melhorar os menus existentes com hierarquia, agrupamento, cor/forma tonal e ícones em linguagem Material You 3 Expressive. Esta etapa é transitória e não redesenha o produto como GTA V.
- Adicionar captura manual contínua via Shizuku, com ações distintas para iniciar e finalizar; conservar a captura dos logs após falha de sessão.
- Correção posterior do usuário: não coletar o logcat completo do aparelho. Filtrar para as entradas do processo GLUE 5; uma exportação do logcat geral ficou com cerca de 35 MB. Não incluir logs de outros apps.

### Plano e critérios de validação
1. Nas páginas Settings atuais, acrescentar cabeçalho/tabs expressivos, ícones vetoriais sem dependência de fontes específicas e grupos sem alterar o significado dos controles; garantir layout em telefone, tablet, desktop e safe area.
2. Acrescentar uma seção Android de diagnóstico que declare o filtro pelo PID do GLUE 5 e botões Iniciar/Finalizar. No serviço Shizuku, usar `logcat --pid=<pid>` tanto na captura contínua como no snapshot da sessão anterior; persistir o PID da sessão em execução e não fazer fallback para o logcat global. Gravar incrementalmente e oferecer compartilhar/copiar após o stop.
3. Validar `cargo fmt --check` e testes da UI/crates relevantes; compilar o AIDL/Java/Android. No aparelho, confirmar fluxo sem Shizuku, autorização negada/concedida, start→stop com arquivo compartilhável e a captura automática após falha. Não marcar teste em aparelho como concluído sem resultado do usuário.
4. Não mexer no pipeline/assinatura. O usuário executará o workflow Android manualmente depois que o código estiver pronto.

### Riscos a observar
- A captura filtrada ainda pode conter dados do próprio GLUE 5 e crescer durante a reprodução; informar o escopo, não compartilhar automaticamente e deixar claro quando a captura terminou.
- A implementação deve fechar o serviço/pipe mesmo em cancelamento, falha de binder ou saída inesperada da Activity, sem interromper a captura por falha de sessão.

### Implementação local desta etapa
- Settings agora recebe tokens de superfície/acento Material 3 Expressive, tabs segmentadas, títulos de página, cabeçalhos de grupo e ícones vetoriais próprios (sem fonte de emoji adicional). Ações recebem tratamento tonal distinto dos controles de ajuste; as funções dos controles existentes não foram substituídas.
- No Android, a aba Logs declara que a captura contém somente entradas do processo GLUE 5. Start/stop e recuperação após falha usam `logcat --pid=<pid>`; o PID da sessão anterior é persistido para o snapshot. O serviço grava incrementalmente e não recorre ao logcat global se não houver PID.
- AIDL recebeu os PIDs em `startLogcat(int)` e `captureLogcat(int)`; versão do UserService foi incrementada para 3. A autorização continua explícita via Shizuku; o GLUE 5 não inicia o serviço nem manipula códigos de pareamento.
- **Validação pendente:** este sandbox não tem `cargo`, `rustc`, `rustfmt`, `java`, `javac`, Gradle nem wrapper Gradle; portanto não rodei formato, testes Rust, compilação AIDL/Android ou aparelho. `git diff --check` passou sem avisos. Não declarar compilação ou teste Shizuku concluídos; após revisar o patch, o usuário executará o workflow e o teste em dispositivo.
- O workflow Android seguinte compilou até `glue5` e falhou com `E0603`: os tokens `theme::expressive::*` estavam declarados `pub(super)` dentro do submódulo `theme::expressive`, o que não os expunha ao módulo irmão `pages`. Corrigido o escopo para `pub(in crate::ui)`, mantendo os tokens privados ao módulo de UI. Falta rerodar o workflow para validar a correção.

### Correção do escopo da captura após exportação de 35 MB
- O usuário esclareceu que não precisa do logcat geral do aparelho; quer somente as linhas do GLUE 5. A captura anterior gerou um arquivo de cerca de 35 MB.
- Alterar a captura contínua e a snapshot pós-falha para `logcat --pid=<pid>` do processo `local.glue5`; persistir o PID durante a sessão e reaproveitá-lo para a recuperação. Não usar o logcat global como fallback se o PID estiver ausente.
- AIDL agora recebe o PID nos métodos `startLogcat(int)`/`captureLogcat(int)`; versão do UserService passa a 3. Atualizar avisos de privacidade e labels para dizer que os demais apps são excluídos, mas que logs do próprio GLUE 5 podem conter caminhos/dados de diagnóstico.
- **Validação pendente:** checar build AIDL/Rust/Android e no aparelho confirmar que live e snapshot contêm apenas o PID do GLUE 5 e produzem arquivo significativamente menor.

### Build Android falhou por lifetime no helper JNI
- O workflow do usuário reportou `E0521` em `call_logcat_activity`: `anyhow::Context::context` exige contexto com vida `'static`, mas a função recebia `&str` arbitrário. Os três chamadores passam literais; o parâmetro foi alterado para `&'static str`.
- Ainda falta o usuário rerodar o workflow para confirmar a correção. Não mexer no workflow nem na assinatura Android.

### Diagnóstico interativo de congelamentos
- O usuário relata congelamento da imagem em alguns casos, com áudio ainda tocando ou controles ainda ativos; em outros casos não há resposta. O GLUE 5 permanece utilizável para abrir Settings.
- Desenvolver captura manual por PID do GLUE 5, botões iniciar/parar e marcadores de evento selecionáveis (imagem congelada, áudio continua, controles respondem/não respondem), mantendo Shizuku, consentimento, filtro PID e compartilhamento explícito. O marcador precisa entrar no log do mesmo processo para correlacionar por timestamp.
- Investigar a branch `origin/main` antes de sincronizar: priorizar correções comprovadas de GPU/renderização, sem trazer automaticamente as grandes mudanças de mods/online nem sobrescrever edições locais.
- Implementação local: nova aba DEBUG usa o start/stop Shizuku filtrado pelo PID do GLUE 5 e oferece marcadores fixos `SCREEN_FROZEN`, `AUDIO_CONTINUES`, `CONTROLS_RESPOND` e `CONTROLS_UNRESPONSIVE`, gravados pelo mesmo processo com tempo monotônico. Sem coleta ou envio automático.
- Identidade de build: manter o SemVer em Cargo e acrescentar `VERSION_CODENAME` neutro (`otter`) à exibição e ao Android `versionName`; não usar sufixos SemVer que possam significar pré-release. O usuário esclareceu que manterá `0.1.0` até pedir uma mudança, e que o codinome distingue builds: `otter` identificava a build daquela etapa; na próxima mudança de código que justificasse uma nova build, escolher outra palavra neutra. Rebuilds da mesma fonte preservam a palavra; o codinome não comunica estágio/estabilidade.
- Atualização de 2026-10-29: para a build com a reformulação significativa do HUD/shell e os ajustes de cenas/blur, o codename mudou de `otter` para `heron`; o SemVer continua `0.1.0`.
- Atualização de 2026-10-29: a próxima build Android direcionada ao Galaxy S21 Plus usa o codename `willow`; SemVer continua `0.1.0` e `VERSION_CODE` é `2`.
- Notificações ntfy: adicionar marcos aproximados de 50% (início da fase longa de compilação/testes) e 85% (trabalho principal concluído, workflow quase finalizado) aos workflows longos de Build, GPU e import tests. São marcos por etapa, não percentuais medidos de tempo; dependem do secret existente `NTFY_TOPIC`.
- Comparação até 2026-10-27: `origin/main` tem 211 commits desde a base comum e alterações amplas (189 arquivos; destaque para mods/menus, online, GPU e Android). Não fazer merge integral nesta fase. A branch original descreve um congelamento de tela com áudio/jogo ainda ativos no Mali/MediaTek, muito parecido com o relato do usuário.
- Portados seletivamente, sem mudar a branch de origem: o heartbeat de GPU e a saída com causa quando submissões param (`1acd494`, `1637c56`, `22937c7`); supressão de falsos positivos em transições de background/foreground; tratamento de device-lost; e drop de recursos em vez de `destroy()` agressivo (`bbd6241`, validado upstream no Pixel 8a Mali-G715: 20/20 sem device-lost, ante ~1/6). Isso deve diagnosticar/sair do freeze de GPU e inclui uma correção plausível, mas ainda não prova que seja o mesmo root cause no aparelho do usuário.
- **Validação pendente:** o sandbox não tem Cargo/Rust/Java/Gradle, então não compilei o fix JNI, heartbeat GPU, UI de debug, versão Android ou YAML/workflows; o usuário executará workflow Android manualmente. Testar marcadores e stop/share no aparelho, em especial nos cenários “imagem parou/áudio segue” e “imagem parou/controles seguem”. Os recursos Adreno e as demais mudanças de mods/online não foram sincronizados.

## 2026-10-27 — Próxima etapa: controles, HUD e UI

### Novos requisitos do usuário
- Editor de controles de toque na tela, com suporte a carregar presets XML; por enquanto a prioridade é permitir que o próprio usuário edite os controles.
- A interação com o celular do GTA deve ser por toques diretos: tocar em cada app, iniciar chamada e encerrá-la; não depender de gestos. Remover os direcionais.
- Os controles atuais cobrem o minimapa/HUD sob o polegar esquerdo ao usar o joystick. Avaliar mover o minimapa para um canto superior ou permitir posicionamento/configuração.
- Separar o HUD do GTA da renderização 3D para que escala/responsividade do HUD possa ser tratada independentemente. O HUD atual está pequeno e pouco legível apesar do jogo usar a resolução total do aparelho.
- Reformular a UI do GLUE 5 com linguagem visual coerente com GTA V, reduzindo a ruptura visual na transição; a separação do HUD deve ser considerada parte da solução, não uma mudança isolada.
- Há um piscar azul de milissegundos perto do minimapa em alguns momentos. A relação com mudanças futuras do HUD ou dos controles é apenas hipótese; não atribuir causa antes de isolar.
- Não fazer alterações sem rumo: inspecionar o caminho atual, elaborar plano, registrar riscos, estabelecer teste e anotar progressos/regressões.
- Este pedido autoriza explicitamente voltar a trabalhar em controles touchscreen e na aparência/background/UI, escopos que anteriormente estavam pausados.

### Plano de investigação, sem implementação ainda
1. Build Android run 40452436632, SHA `7be19d1`, terminou em falha no step `Build`; setup, dependências e assinatura passaram. A annotation só informa exit code 1, e a coleta dos logs via API falhou com EOF; a causa permanece desconhecida. O aviso de Node.js 20 é não fatal. Obter as linhas do erro antes de corrigir; esta build não inclui as mudanças desta próxima etapa.
2. Registrar baseline de controles e renderização no aparelho. Separar os bugs de toque, escala do HUD e piscar azul; variar um subsistema por vez.
3. Mapear o modelo de controles existente e definir XML de preset com validação, versão, limites de posição/tamanho e fallback seguro; só então desenhar o editor mínimo.
4. Usar o censo GPU e as medições existentes para decidir a separação do HUD. Gate: comprovar que alvos, leituras do back buffer, `SV_Position`, profundidade, clears e limites de pass são seguros; sem redirect especulativo nem descarte previsto.
5. Fazer protótipo isolado de HUD responsivo e layout sem oclusão do polegar; comparar capturas/métricas antes e depois e testar a cintilação azul com toggles A/B.
6. Reformular a UI do GLUE 5 depois de estabilizar a base técnica, com estilo próprio inspirado na hierarquia visual do GTA V e transições consistentes; validar em tela de celular e sem jogo instalado.
7. Para cada etapa, registrar commit, build, aparelho/ação de teste, resultado e qualquer regressão antes de avançar.

## Requisitos técnicos registrados anteriormente

- Estender E0 e o censo GPU existente; não reconstruir a infraestrutura já compilada no CI.
- `pass swaps` mede trocas de conjunto de render targets; passes que escrevem no back buffer precisam de uma métrica separada.
- `reads SV_Pos: 3` conta programas/labels distintos no censo, não leituras por frame; o gate associado ainda não passou.
- A fórmula `10,4 MB × (1 + 2·(P−1))` é apenas estimativa teórica; para `P=4`, cerca de `73 MB`, não uma medição.
- Nunca ignorar `CLEAR_RT` do back buffer na fase HUD; se aparecer, cancelar o redirecionamento naquele frame.
- Não usar `StoreOp::Discard` baseado em previsão.
- Validar a forma WGSL via IR do naga, não por busca textual. O seed contém descritores, não milhares de fontes WGSL.
- Não presumir que o pass de HUD termina em `SCENE_DONE`; medir/comprovar antes de redirecionar.
- Em relatório de bugs, preferir dados copiáveis a screenshots. Não incluir conteúdo de saves nem coordenadas cruas de touch.
- Preservar alterações locais preexistentes e não alterar pipeline ou assinatura Android sem pedido explícito.

## 2026-10-30 — Celular do jogo abre e fecha repetidamente (novo relato)

- O usuário relatou de novo que o celular do GTA abre e fecha rapidamente: clica no botão e o ciclo começa; joga por touch, a pé e em veículo. A correção de 2026-10-29 (pulso único enquanto o dedo segura) não resolveu no aparelho.
- Nova leitura do código: enquanto o celular está subindo, o botão Phone continua visível, pois o contexto só muda quando o motor registra o celular aberto; um segundo registro do mesmo toque nesse intervalo emitia outro `UP` e guardava o celular pela metade. Com o celular aberto sem posição conhecida, toques no lugar dele também podiam cair em botões do layout.
- Implementação local: o pulso do botão Phone vale até o celular ser visto aberto (ou 1,5 s se não abrir, para permitir nova tentativa); registros repetidos são ignorados, sem haptics. `Touch::phone_area()` concentra o retângulo conhecido ou presumido do celular, usado pelas teclas e pelo roteamento do toque. Testes novos cobrem pulso único até ver o celular aberto, nova tentativa após a espera e toque sem botão com o celular aberto sem posição.
- **Validação pendente:** sem toolchain Rust neste ambiente, não rodei `cargo test` nem build Android; o usuário precisa gerar a Build e testar no aparelho. Se persistir, pedir o relatório de diagnóstico do app: o trace touch registra as transições do pad (`Up`) sem coordenadas e distingue repetição de entrada de causa interna do jogo.

## 2026-10-30 — Áreas de toque de debug ensinadas pelo usuário

- O usuário perguntou se cada elemento tocável do GTA tem ID para podermos referenciá-lo. Investigação: a UI do jogo é Scaleform; o GLUE 5 conhece filmes por nome, alguns métodos e estados, mas não recebe uma lista de elementos internos com IDs. O botão de encerrar chamada, por exemplo, vive dentro do filme do celular; só o jogo sabe o que há em cada coordenada.
- Escolhas do usuário: cada área tem ação própria (clique no centro ou botão de controle), contexto próprio de exibição, e editar e testar são modos separados; implementar agora.
- Implementação local (primeira versão): `DebugTouchArea` persistida em `glue5.json` (nome, retângulo em frações da tela, ação `Click`/`PadButton`, botão `Back` por padrão, contexto `Always`/`Gameplay`/`Phone`/`Pointer`). Em Settings › Diagnostics: interruptor de visibilidade, botão **Edit debug touch areas** (editor sobre o jogo: desenhar, mover e redimensionar por cantos, sem disparar ações; áreas editadas voltam para as configurações e são salvas quando nenhum dedo está editando) e **Add**, com nome/posição/ação/botão/contexto/excluir por área. No modo ativo, a área intercepta o toque antes dos controles: botão pulsa no toque; clique acontece ao levantar, no centro da área. Toques de debug também entram em `occupied()`, para overlays não ficarem sob elas.
- Testes novos em `touch/tests.rs`: pulso único do botão e clique no centro, área de celular fora do gameplay, desenhar/mover/redimensionar sem agir, toque pequeno descartado e cópia das configurações sem sobrescrever o editor.
- **Validação pendente:** sem toolchain Rust neste ambiente, não rodei `cargo test`, `cargo fmt` nem build Android; `git diff --check` é a única checagem automática possível aqui. Validar no aparelho: criar uma área sobre o botão de encerrar chamada com contexto **Phone**, testá-la em chamada e confirmar que o editor não dispara ações.

## 2026-10-31 — Assets do Drive verificados e o pop-up estilo GTA V (primeira fatia de paridade visual)

- O usuário definiu o rumo: deixar o GLUE 5 com a maior cara de GTA V possível, seguindo os padrões visuais do V (citando como lacuna os pop-ups, inexistentes no GLUE), e colocou no Google Drive (pasta `glue5/assets`) extrações de RPF de hoje: 44 filmes `.gfx` (HUD, Minimap, `pause_menu_pages_*`, celular), `.ytd`, `.ysc` crus, `frontend.xml`/`pausemenu.xml`, dois `scaleform_frontend.rpf` e um `cellphone.zip`. A `chalet.woff` da pasta-pai é o `$Font2` (Chalet-LondonNineteenSixty **Regular**, 320 glifos) exportado pelo FFDec; os filmes em si não embutem fonte, importam `gfxfontlib.swf`/`font_lib_efigs.swf` (`$Font2`, `$Font2_cond`, `$Font5`, `$FixedWidthNumbers`). Registro completo e parecer item a item em `docs/gta5-visual-parity.md`, incluindo o que pedir das ferramentas de RPF/descompilação de YSC do usuário e o limite legal da fonte (uso só em tempo de execução, dos arquivos do jogador; nunca embarcar/commit).
- Implementação local: novo `crates/glue5/src/ui/popup.rs` — aviso modal no vocabulário do shell (página escurecida, bloco quadrado, escolhas como blocos com foco branco invertido e contador `1/2`, rodapé `A SELECT / B BACK`, seleção inicial na opção segura), com as teclas retiradas do input do frame para a página de baixo não receber setas/Enter. Ligado a **Restore graphics** e **Clear the local copy** (Settings) e ao **Delete** da cópia em Servers, aposentando o padrão "Sure?" de duplo toque; testes cobrem o filtro de teclas, o wrap da seleção e as duas respostas do aviso.
- **Validação pendente:** sem toolchain Rust neste ambiente (sem `cargo`/`rustc`/`mise`, e `static.rust-lang.org` inalcançável), não rodei `cargo test`/`cargo fmt`/build; checks feitos: revisão linha a linha contra as APIs já usadas no shell, `git diff` e análise dos `.gfx`/WOFF/XML por scripts Python. Validar: compilar e, no aparelho, acionar os três pontos e confirmar que setas/toque/B/ESC respondem só no pop-up, sem a página de baixo reagir.

## 2026-11-01 — Verificação completa do Drive e inventário do lote `PACOTES ARENA`

- O usuário pediu a verificação de que **tudo** está no Google Drive e a documentação detalhada do que foi encontrado, por ser importante para trabalho futuro. Ele subiu `glue5/PACOTES ARENA/01-11 02:30`: 20 ZIPs + 4 arquivos de índice (893 MB), resposta ao guia de extração base/OpenIV e ao prompt de descompilação YSC de 2026-10-31.
- Verificação feita aqui: SHA-256 dos 20 ZIPs confere com o índice oficial, `testzip` íntegro, 4.520 membros extraídos, 3 arquivos divididos remontados e validados por offset/tamanho (`SCRIPT.rpf`, `paths.xml`, `script_rel.rpf`). Inventário próprio: 4.523 arquivos com hash individual. Material proprietário ficou fora do repositório, como sempre.
- Achados que mudam o trabalho: mapa alias→fonte do `fontmap.xml` confirmado e 50 TTF do FFDec analisados glifo a glifo (`$Font2` = Chalet-LondonNineteenSixty 400, 320 glifos, pt-BR completo; `$Font2_cond` = ChaletComprime-CologneSixty); paleta medida (branco = RGB 240, fundo de bloco = preto 73%); geometria real do `popup_warning.gfx` e dos botões instrucionais conferida no ActionScript (o nosso `ui/popup.rs` é aproximação, os deltas estão registrados); o mistério das cópias `(1)` do lote 1 resolvido por hash: `(1)` = build base, sem sufixo = build update.
- Pendência mais valiosa: os 22 `.ysc.full` estão descriptografados e validados, mas a descompilação em massa falhou (`NO_OUTPUT_EXIT_`, logs vazios) — só existem 3 `.c` (celular ×2, `ingamehud`). Rodar o descompilador nesses 22 é o próximo desbloqueio.
- Registro durável: novo `docs/gta5-reference-inventory.md` + CSVs de metadados em `docs/gta5-referencia/`; `docs/gta5-visual-parity.md` atualizado (pedidos marcados como atendidos, deltas medidos para o shell).
- **Lição operacional:** o workspace do sandbox não persiste entre sessões — os downloads do lote 1 precisaram ser rebaixados. A fonte da verdade do material de referência é o Google Drive + o PC do usuário; os IDs do Drive estão no inventário.
- **Validação pendente (antiga):** o código do pop-up segue sem compilação/teste neste ambiente (sem toolchain Rust); continua valendo o pedido de `cargo test` + teste no aparelho.
- Adendo (2026-11-01, tarde): baixados também os dois últimos itens do Drive — `pausemenu.m4a` (AAC 21min28s, gravação via FFmpeg, não é arquivo do jogo) e `RESIDENT.rpf` (RPF7 de TOC criptografada, não aberto). Com isso a pasta `glue5/` do Drive está 100% baixada/inventariada; nada novo chegou desde as 02:30. Confirmado na prática que o sandbox apaga tudo fora do repositório entre turnos — o acervo bruto vive no Drive/PC e se rebaixa pela receita do inventário (§10), com hashes em `docs/gta5-referencia/`.
- Adendo (2026-11-01, tarde 2): o usuário confirmou que o `Download.7z` da raiz do Drive **não é do projeto** — escopo do projeto é a pasta `glue5/` (o `RESIDENT.rpf` ficou na raiz; recomendado movê-lo para dentro de `glue5/`). Correções aplicadas no inventário. Preparado `docs/prompt-chatgpt-2026-11-01-ysc-full.md` (rodada 2 para o PC do usuário: descompilar os 22 `.ysc.full` com protocolo anti-falha silenciosa, confirmar a proveniência do `script_rel` contra o `SCRIPT.rpf` com hashes de referência, ficha de estilo das lacunas, FFDec opcional; entrega `ENTREGAS-GLUE5-YSC-02` via `PACOTES ARENA`). O prompt foi colado na conversa para envio imediato e salvo no repositório (o guia da rodada 1 se perdeu com o reset do sandbox).
- Adendo (2026-11-01, tarde 3): usuário conectou o **Linear**; workspace conferido (time `Glue5`/`GLU`, projeto `GLUE 5 — GTA V para Android` id `290ca018-…`). Backlog semeado a partir dos docs: 15 issues GLU-5…GLU-19 (frente PC/referência GLU-5…GLU-9+GLU-11; código GLU-10, GLU-12…GLU-19), labels criadas `pc-usuario`, `paridade-visual`, `referencia-gta5`, `validacao`; GLU-7 (descompilar os 22) em *In Progress* bloqueando GLU-8 (ficha de estilo 02) e GLU-9 (verificar entrega); GLU-12 (compilar/testar pop-up) em *Todo* como urgente. Issues-modelo GLU-1…GLU-4 do Linear arquivadas e texto do projeto preenchido com estado/regras/documentos.
- Adendo (2026-11-01, final da tarde): rodada 2 recebida em `glue5/PACOTES ARENA/01-11 14:55` (ZIP 4,0 MB) e **verificada**: SHA do índice confere, 1.631 arquivos, 22/22 `.c` com hashes de entrada/saída revalidados (os 3 regenerados byte-idênticos) e 24/24 filmes com ActionScript (817 `.as`, entradas idênticas à base da rodada 1). Descobertas: `SCRIPT.rpf` é só áudio (521 `.awc`); contêiner dos scripts é `script_rel.rpf` (`x64/levels/gta5/script/`); bytes brutos de `.ysc` variam entre cópias mas os corpos de código são idênticos → proveniência bruta = mistura/indeterminado, conteúdo validado. `RELATORIO-ESTILO-HUD-02` fecha as lacunas (legendas, área/rua, dinheiro, estrelas, mensagens grandes) e traz os SoundId literais (`NAV_UP_DOWN` etc.). Registro completo: inventário §13. Linear: GLU-5/6/7/8/9 concluídas. Próximo gargalo volta a ser código nosso: compilar/testar o pop-up (GLU-12).
