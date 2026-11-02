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

## 2026-10-30 — Celular continua abrindo e fechando (terceiro relato)

- O usuário buildou a `main` com as duas correções anteriores e confirmou: o celular continua com o bug, em carro e a pé. A hipótese de pulsos duplicados do botão Phone, sozinha, não explica o aparelho.
- Nova leitura: quando o celular abre, `Touch::set_context` só soltava o dedo do botão Phone. Os demais dedos de botão continuavam no mapa e o `gamepad()` continua emitindo botões `Press` enquanto o dedo está baixo, mesmo se o botão sumiu do layout. Melee/Reload é `EAST` — justamente o `CELLPHONE_CANCEL`. Um botão segurado de antes de abrir (ou um botão criado pelos prompts) podia, portanto, guardar o celular logo em seguida; simetricamente, a tecla Back do celular segurada ao fechar continuava emitindo em jogo.
- Terceira correção local: na abertura do celular, soltar todos os dedos de botão que o celular não mantém (`with_phone` ou Start/Select ficam), incluindo botões de prompt e o leque More; limpar `pressed`, `pressed_inputs` e clique pendente. No fechamento, soltar as teclas `Key::Phone`. O trace de diagnóstico do toque passou a registrar `phone out: true/false` junto das transições do pad, para o próximo relatório separar causa nossa de causa interna do jogo.
- Teste novo: `opening_phone_releases_other_held_buttons_and_closing_releases_its_keys` (segurar Melee, abrir o celular e exigir `EAST = 0`; segurar o Back do celular, fechar e exigir `EAST = 0`).
- **Validação pendente:** sem toolchain Rust neste ambiente, não rodei `cargo test` nem build Android. Se ainda ciclar, o próximo passo é o usuário reproduzir e copiar o **Import report** (Settings › Diagnostics) logo em seguida: com `phone out` e as saídas do pad no mesmo trace, dá para saber se o GLUE emitiu `East`/`Up` de novo ou se o jogo ciclou sozinho (aí a investigação vai para `LAST_DEVICE`/estado do `CPhoneMgr` no runtime).

## 2026-10-30 — Celular cicla com um único toque, parado (quarto relato)

- O usuário corrigiu a premissa: **um único toque no botão de celular, totalmente parado, sem tocar em mais nada** — e o celular abre e fecha repetidamente, a pé ou em veículo, até clicar em Voltar. Logo, botão segurado/segundo dedo **não** é a explicação; a limpeza anterior fica como higiene, não como causa. O ciclo é autônomo depois de um pulso só.
- O mecanismo autônomo encontrado está no runtime: `control_update` lê os bytes brutos do `CPhoneMgr` a cada frame e publica `PHONE_OUT` direto. Durante a própria abertura (`PHONE_GOING` ligado) — e possivelmente quando o jogo refaz o celular ao trocar o dispositivo de entrada — esse valor cai por instantes. Como é justamente o celular aberto que faz `control_update` gravar `LAST_DEVICE = teclado/mouse`, cada queda devolve o dispositivo para o pad, e a volta para teclado/mouse pode fazer o jogo refazer o celular de novo: um vaivém que se sustenta sozinho, sem nenhum dedo, e que o Voltar encerra porque aí o celular é guardado de vez. O layout touch (`context.phone`) oscilava junto.
- Quarta correção: histerese no `PHONE_OUT` do runtime (`PhoneOut` em `crates/glue5-runtime/src/input.rs`). Abrir é acreditado após 60 ms estáveis; fechar só após 600 ms contínuos de leitura "guardado". Quedas curtas durante a refação do celular não alternam mais o estado publicado, nem o dispositivo, nem o layout, nem a ampliação. Teste novo `phone_out_dips_while_the_phone_remakes_itself_do_not_flip_it_back` cobre queda curta (ignora) e fechamento sustentado (acredita).
- **Validação pendente:** sem toolchain Rust neste ambiente, não rodei `cargo test` nem build Android. O usuário precisa buildar a branch `arena/ed8f0e3a-glue5`. Se ainda ciclar com essa build, aí o ciclo é interno do jogo mesmo com `PHONE_OUT` estável — e o Import report dessa build (que já registra `phone out`) mostra isso: celular ciclando sem nenhuma transição nossa repetida.

## 2026-10-31 — Celular ainda cicla: relatório separa o toque do runtime (quinto relato)

- O usuário testou a `main` pós-PR #9 (`92805a8`) e o ciclo continua. Quarta tentativa também não resolveu no aparelho — sem nova hipótese às cegas nesta rodada.
- O **Import report** dessa build chegou (SM-G996B, build `92805a8`) e inocenta a camada de toque na amostra: depois do pulso único `Up`, o trace só mostra `buttons=[none]` e `phone out: true` **uma vez** — nenhum `Up`/`East` repetido sai do GLUE. O que faltava era enxergar o lado do runtime: bytes crus do `CPhoneMgr` e o dispositivo gravado, que o relatório não tinha. (Limitação achada de quebra: o trace do pad gravava sticks em centésimos, então o arrasto enchia o buffer de 48 linhas e expulsava os eventos de botão.)
- Esta rodada entrega **instrumentação + um teste A/B**, não uma correção prometida:
  - O runtime passa a registrar transições de `created/camera/going` crus, do `raw_out`, do estado publicado e do dispositivo (`pad` × `keyboard/mouse`), com carimbo `+ms`; o relatório ganha a seção "Recent phone/input-device transitions (from the game)".
  - O trace do toque ganha carimbo `+ms` e sticks/gatilhos em quartos de passo, para o arrasto não expulsar os botões do buffer.
  - Nova chave **Settings › Diagnostics › "Phone touchscreen input"** (padrão ligada, `Settings::phone_touchscreen`): desligada, o runtime não troca `LAST_DEVICE` para teclado/mouse por causa do celular — o pad (D-pad/botões ao lado do aparelho) comanda o celular. Se o ciclo parar com ela desligada, o gatilho é a troca de dispositivo; se continuar, é interno do jogo mesmo com o pad no comando.
- **Validação pendente:** sem toolchain Rust neste ambiente, não rodei `cargo test` nem build Android. Protocolo para o usuário: buildar, reproduzir com a chave ligada e copiar o Import report; depois desligar a chave, reproduzir de novo e mandar o segundo relatório (e dizer se o ciclo parou). Os dois relatórios juntos apontam o culpado sem chute.

## 2026-10-31 — Os dois relatórios chegaram: o estado do celular não cicla (sexto relato)

- Protocolo A/B executado pelo usuário na build `ba0becc`, chave ligada e desligada, a pé e em carro: **o ciclo visual acontece nos dois modos**. E os traces novos contam a história que faltava:
  - Nos dois relatórios, os bytes crus do `CPhoneMgr` (`created/going`) abrem **uma vez**, ficam estáveis por todo o episódio (até ~57 s) e fecham **uma vez**. Nenhum flicker. O aparelho na mão do personagem **não** sobe e desce fisicamente.
  - Com a chave desligada, `device=pad` o episódio inteiro e o ciclo visual persistiu: **a troca de dispositivo está inocente** (a histerese do `PHONE_OUT` também — o publicado acompanha o cru com os atrasos de 60/600 ms certinhos).
  - No relatório da chave desligada, o trace do toque mostra o usuário navegando pelas teclas do celular (D-pad) por ~6 s durante o episódio — tentando usar um celular que não para quieto.
- Nova pista do usuário, decisiva: **com o bug rodando, abrir o menu do GLUE faz o celular aparecer e parar de ciclar**, com o mundo do jogo vivo atrás (carros e NPCs se movendo). Logo não é congelamento: é algo que o GLUE faz por frame com o celular aberto e que cessa quando os controles touch somem (`touch_play` falso).
- O que resta de autônomo por frame com o celular aberto, nos dois modos: (1) a **ampliação do celular** (`update_phone_position` reescreve a posição do `CPhoneMgr` a cada `control_update` para trazê-lo 1,25× mais perto); (2) o pad virtual publicado; (3) `LAST_DEVICE` reescrito. Se a posição reescrita estiver oscilando entre a original e a ampliada, o celular "vai e volta" na tela — a leitura de "abrindo e fechando" — sem tocar nos bytes de estado. É o suspeito nº 1 agora.
- Esta rodada (branch `arena/d32e41ec-glue5`): o trace do runtime ganhou `pos_z` (profundidade do celular em meios passos — oscilação vira linhas próprias no relatório) e nova chave experimental **Settings › Diagnostics › "Bigger phone"** (`Settings::phone_enlarge` → `input::set_phone_enlarge`, padrão ligada): desligada, a posição do jogo fica intocada. Um único teste no aparelho separa: ciclo para sem ampliação → culprada achada e o conserto é na lógica de ampliação; continua → próximo suspeito é o stream do pad, e o `pos_z` do relatório mostra a posição quieta.
- **Validação pendente:** sem toolchain Rust neste ambiente; aguardar CI do PR e o teste do usuário no aparelho. Nada de "corrigido" sem o OK dele.
- [PERF] Adendo (2026-11-01): **back-off térmico no Android** (branch `arena/b8d177aa-glue5`, issue **GLU-26**) — a Thermal API do NDK (`AThermal_getCurrentThermalStatus`, API 30; `AThermal_getThermalHeadroom`, API 31) lida em tempo de execução por `dlopen("libandroid.so")` em `crates/glue5-qos/src/thermal.rs`; um *governor* com histerese vira as leituras em nível 0–3 (sobe após 6 s de calor pedido, desce um nível a cada 45 s; o veredito do sistema é um piso). O nível segura o jogo pelo hook do limitador (`glue5-runtime/src/frame_pace.rs`): 40 fps no nível 1, 30 nos níveis 2 e 3 (tela de 120 Hz), e com *Frame rate and resolution* a escala de render cai a 85% no último nível. O alvo do *performance hint* e a taxa pedida ao display acompanham o limite. Paralelo: **PGO no aparelho** (`GLUE5_GAME_PGO=generate` + `mise run pgo:android`: build instrumentado → benchmark no device → `adb pull` → `llvm-profdata merge` do NDK → `game.profdata` → rebuild com o profile; o app escreve o arquivo à mão com `__llvm_profile_write_file()` porque o processo termina sem os *exit handlers*), **`-mtune=cortex-a78`** no C do jogo para Android (`GLUE5_ANDROID_TUNE`) e o **preset de gráficos "Phone"** (`glue5-runtime/src/game_settings.rs`: texturas menores, sem reflexos, sem sombras suaves, sem grama, sem filtragem anisotrópica — o que corta os bytes que a Mali-G78 lê da memória do aparelho). Rejeitados após inspeção: Arm ASR (C++/GLSL/HLSL, sem WGSL, e é FSR 2.2.2 contra o FSR 2.3 que já temos), Swappy (pacifica swapchains), Turnip/libadrenotools (Adreno), `StoreOp::Discard` (toda attachment é lida depois) e ThinLTO (zero ganho). **Validação pendente:** limiares (0,80/0,90/0,97 do headroom) são palpite, a afinar pelas linhas `thermal:` do log; o split dos passes (`Stats.pass_ends`) ainda não foi medido; sem toolchain Rust neste ambiente, `cargo check`/`cargo test` não rodaram — a primeira compilação é no aparelho.
- [PERF] Adendo (2026-11-02): **publicação e integração** (branch `arena/b8d177aa-glue5`, issue **GLU-26**) — o dono autorizou a publicação: a branch está no remoto, tip `19f751d` (9 commits à frente de `af1158b`, 28 arquivos, +1251/−24), **sem PR e sem merge** no momento desta linha; a integração em `main` vem no PR aberto a partir desta branch. O **CI do repositório rodou na branch e passou** (`GPU tests` e `Release guard`), e foi a **primeira compilação real** deste trabalho: a cadeia `glue5-gpu → glue5-runtime → {glue5-game, glue5-stream → glue5-qos}` compila o `thermal.rs`, o `frame_pace.rs`, o `game_settings.rs` e o build script do `-mtune`, e a suíte de GPU passa. **Continua sem compilar** o crate do app (`app.rs`, `ui.rs`, `ui/pages.rs`, `ui/perf.rs`, `settings.rs`, `imports.rs`, `profile.rs`, `build.rs`) e o lado Java (`LLVM_PROFILE_FILE`) — só o build do dono os cobre. **Achado do CI:** nenhum workflow testa o pacote `glue5-qos`, então os 8 testes unitários do *governor* (`thermal.rs`) compilariam mas não rodariam — precisam de `cargo test -p glue5-qos` na mão até que o `import-tests.yml` ganhe a linha que os roda. **No `main`, o `build.yml` já traz a identidade de build** (`Glue5-<plataforma>-<branch>-<sha7>.apk`, `Glue5-BUILD-<id>.txt`, `concurrency` com `github.ref`, o keystore `Glue5-android.keystore` preservado), trazida pelo PR #13 da revisão geral; o commit `daa1bf9` (ChatGPT/GPT-6, empurrado nesta branch) só antecipava `default: android`, que o `main` já trazia. **Validação pendente:** o build do dono, as linhas `thermal:` do log para afinar os limiares (0,80/0,90/0,97 continuam palpite), o split dos passes (`Stats.pass_ends`) ainda não medido, e o alvo de 30 fps (40 no painel de 120 Hz). **O toque direto no celular do jogo segue sem confirmação (GLU-27).**
