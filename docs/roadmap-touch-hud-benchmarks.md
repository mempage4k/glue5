# Backlog: controles touch, HUD e benchmarks do GLUE 5

Itens registrados a pedido do usuário em 2026-10-28. Este documento é planejamento; não afirma que qualquer item já esteja implementado.

## Controles touchscreen

- Permitir editar os controles touch na própria tela, ajustando o layout para o uso do jogador.
- Considerar carregar e salvar presets XML, conforme o backlog existente.
- No celular do GTA V, permitir interação touchscreen direta: tocar no ícone visível do app para selecioná-lo/abri-lo e tocar nos controles da tela para atender/encerrar chamadas. Priorizar toque no alvo, não deslizar para navegar pela grade; manter gestos apenas onde fizerem sentido.
- No modo touchscreen, aumentar o tamanho visual do celular para facilitar leitura e toque, mantendo o retângulo de interação alinhado ao aparelho renderizado. Não alterar a escala para quem não usa touchscreen.
- Relato do usuário (2026-10-29): o celular do GTA abre e fecha repetidamente; o drive-by dispara sempre reto; o usuário quer poder editar a posição dos controles. Tratar o primeiro como possível repetição do botão de abrir, e o segundo como falta de mira direcional durante o tiro; validar no aparelho antes de considerar resolvidos.

## HUD e renderização

- Melhorar a responsividade do HUD do GTA V para diferentes tamanhos de tela, densidades e orientações, incluindo escala, legibilidade e áreas de toque.
- Direção de longo prazo registrada pelo usuário (2026-10-31): criar uma comunicação ativa e estruturada entre GLUE 5 e GTA V para observar e manipular mecanismos do jogo com mais controle. Não há protocolo geral definido nem implementação nesta etapa.
- Investigar a separação da composição/renderização do HUD e dos menus do GTA em relação ao passe 3D: o objetivo é poder reduzir a resolução da cena 3D e ainda compor HUD/menus na resolução de saída, legíveis e visíveis.
- Antes de qualquer mudança nos passes, mapear alvos, ordem de composição, profundidade, clears e demais dependências; não presumir que redirecionar ou separar o HUD seja seguro ou suportado pelo pipeline atual.

## Benchmarks do GLUE 5

- Planejar opções de benchmark próprias do GLUE 5, distintas do benchmark embutido no GTA V.
- Usar cenários controlados e repetíveis e registrar as condições e os resultados de cada execução.
- Manter ativo o log da execução do GLUE 5 durante todo o benchmark; não ampliar a captura para logs de outros processos.
- Cobrir pelo menos testes de efeitos visuais e de renderização do mapa.

## Progresso técnico: benchmark próprio do GLUE 5 (2026-10-28)

- Adicionado `benchmark::glue5` como base isolada do benchmark embutido no GTA V: cenários tipados para efeitos visuais e renderização do mapa, condições de execução e registros JSONL de início, cada frame e encerramento.
- O gravador faz `flush` após cada registro para preservar os frames já capturados em caso de falha e rejeita tempos não finitos/negativos. Ainda não está ligado a um botão, a cenários executáveis ou à coleta ao vivo; portanto nenhum benchmark foi executado.
- Próxima etapa: conectar o gravador a um executor controlado que leia os tempos/census do GLUE 5, mantenha o log ativo durante todo o run e grave condições/resultados em local próprio.

## Mapeamento inicial do código (2026-10-28)

- O editor de layout ainda não existe. `Settings` guarda apenas perfil Classic/Modern, tamanho, opacidade e preferências de interação; não há posições customizadas nem importação/exportação XML.
- `Touch::buttons()` monta controles a partir do contexto do jogador e dos prompts dinâmicos. `Touch::draw()`, `touch_finger()` e `occupied()` dependem dessa geometria; qualquer editor deve usar a mesma posição resolvida para renderizar, acertar hitboxes e evitar sobreposições.
- As posições padrão são âncoras `At` relativas à safe area e à escala da tela. O app configura o objeto `Touch` durante o jogo; a página de controles atual não oferece uma prévia editável.
- Implementado no roteamento de input, **não como seleção direta**: um tap no visor emite clique de mouse genérico na coordenada tocada; swipes seguem como passos direcionais. Isso não muda a seleção interna do filme por si só. O usuário confirmou que, com Michael pré-selecionado, tocar Trevor ainda abre as mensagens de Michael; não apresentar clique ou swipe como prova de hit-test direto. Atender/encerrar chamadas por alvo tocado também continua sem validação.
- A ampliação experimental de 1,25× aproxima o aparelho e usa sua posição para a área de toque; a investigação posterior identificou corrida entre essa escrita por frame e a posição/sway que o jogo escreve. A mitigação atual é `phone_enlarge=false` por padrão. Ver o status de validação abaixo; não tratar a ampliação como comportamento seguro ou como correção de interação direta.
- Correção inicial (2026-10-29): o botão de abrir o celular passou a emitir um único frame de `UP`, evitando repetir a ação enquanto o dedo continua pressionado; o teste cobre frames subsequentes sem soltar. O botão drive-by agora continua segurando L1+R1 enquanto um arrasto nele move a mira/câmera; testes cobrem Classic e Modern. Ainda não foi compilado nem validado no aparelho.
- Segunda correção (2026-10-30, após novo relato no aparelho): o pulso único não bastava — enquanto o celular ainda está subindo, o botão continua visível e qualquer segundo registro do mesmo toque (evento duplicado, segundo dedo ou retoque) emitia outro `UP`, guardando o celular de novo e reiniciando o ciclo abre/fecha. Agora o pulso vale até o jogo mostrar o celular aberto (ou 1,5 s, se ele não abrir): registros repetidos nesse intervalo não emitem nada. Além disso, com o celular aberto mas a posição ainda desconhecida, toques onde ele fica não caem mais em botões do layout (o Back o guardaria). Testes: `the_phone_button_pulses_once_until_the_phone_is_seen_out` e `with_the_phone_out_unplaced_a_tap_on_its_place_presses_no_button`. Ainda não compilado nem validado no aparelho; se o ciclo persistir, coletar o relatório de diagnóstico do app (ele registra as transições do pad touch: pulsos repetidos de `Up` apareceriam nele).
- Áreas de toque de debug (primeira versão, 2026-10-30): os elementos internos das telas Scaleform do GTA não são expostos ao GLUE 5 com IDs, então o usuário ensina retângulos sobre a tela. Em Settings › Diagnostics, o interruptor mostra/ativa as áreas; **Edit debug touch areas** fecha as páginas e entra no editor sobre o jogo (desenhar em espaço vazio, arrastar para mover, puxar cantos para redimensionar; editar não dispara ação). Cada área persiste em `glue5.json` com nome, retângulo normalizado, contexto de exibição (sempre, gameplay, celular aberto ou telas de ponteiro) e ação ao tocar: clique de mouse no centro da área ou um pulso de botão do pad (Back/Confirm/direcionais). Testes novos cobrem pulso/clique, contexto e edição. Ainda não compilado nem validado no aparelho.
- Terceira correção (2026-10-30, após o usuário confirmar que o ciclo continua a pé e em carro): ao abrir, o celular troca o lado direito do layout, mas os dedos de botões que ele não mantém continuavam valendo — Melee é East, o Cancel do celular, então um botão segurado de antes (ou um botão de prompt) podia guardar o celular logo depois de abrir. Agora a abertura solta todos os dedos de botão que o celular não mantém, limpa pulsos/inputs enfileirados e qualquer clique pendente; ao fechar, as teclas do celular também são soltas. O trace touch do relatório passou a registrar as transições `phone out: true/false` junto das saídas do pad. Testes: `opening_phone_releases_other_held_buttons_and_closing_releases_its_keys`. Ainda não compilado nem validado no aparelho; se persistir, coletar o **Import report** em Settings › Diagnostics logo após reproduzir — o trace agora distingue o GLUE emitindo `East`/`Up` de novo do jogo ciclando por conta própria.
- Quarta correção (2026-10-30): o usuário esclareceu que o ciclo ocorre com **um único toque, totalmente parado** — logo não depende de botão segurado. O ciclo pode nascer no runtime: `PHONE_OUT` era publicado direto dos bytes brutos do `CPhoneMgr`, que caem durante a própria abertura do celular; e celular aberto força `LAST_DEVICE` para teclado/mouse, então cada queda/volta pode fazer o jogo refazer o celular e reiniciar o ciclo sozinho, sem nenhum dedo. Agora há histerese: abrir é acreditado após 60 ms estáveis; fechar só após 600 ms contínuos. Teste: `phone_out_dips_while_the_phone_remakes_itself_do_not_flip_it_back`. Validado no CI (PR #9 mergeado, `main` em `92805a8`), **mas o usuário confirmou no aparelho que o ciclo continua**.
- Diagnóstico e teste A/B (2026-10-31, branch `arena/4b9fd668-glue5`): o Import report da build `92805a8` mostra um único pulso `Up`, nenhum `East` repetido e `phone out: true` uma vez — a camada de toque não alimenta o ciclo. O runtime agora registra no relatório as transições dos bytes crus do celular (`created/camera/going`), do estado publicado e do dispositivo gravado (pad × teclado/mouse), com carimbo de tempo; o trace do toque também ganhou carimbo e sticks em quartos de passo. Nova chave experimental em Settings › Diagnostics, **Phone touchscreen input**: desligada, o celular fica no pad (sem troca para teclado/mouse). Sem toolchain neste ambiente; protocolo de validação no aparelho: reproduzir com a chave ligada → Import report; desligar a chave → reproduzir → segundo relatório.
- Resultado do A/B e próximo suspeito (2026-10-31, branch `arena/d32e41ec-glue5`): os dois relatórios (`ba0becc`) mostram os bytes crus do celular abrindo uma vez e fechando uma vez por episódio, sem flicker — e o ciclo visual ocorreu **nos dois modos**, então a troca de dispositivo não é o gatilho. Abrir o menu do GLUE para o ciclo com o jogo vivo atrás: o que cessa aí é o que o GLUE faz por frame com o celular aberto. Suspeito nº 1: a **ampliação de 1,25×**, que reescreve a posição do celular a cada frame; uma oscilação original↔ampliada pareceria "abrindo e fechando" sem mover os bytes de estado. O trace ganhou `pos_z`, e Diagnostics ganhou a chave experimental **Bigger phone** (desligar mantém a posição do jogo intocada). Validação no aparelho pendente: reproduzir com Bigger phone desligada e mandar o Import report.
- Desfecho do ciclo e revisão do toque (2026-10-31, branch `arena/107599ac-glue5`): o usuário confirmou no aparelho que **desligar Bigger phone para o ciclo** — a ampliação (reescrever a posição do celular por frame, em outra thread, enquanto o jogo escreve a dele) era o gatilho. `phone_enlarge` agora nasce desligada e a chave avisa que é experimental; ampliar sem a briga pede intervir em `SET_MOBILE_PHONE_POSITION` na fronteira do nativo (futuro). No início, o tap que abria Contatos foi atribuído à ausência de delta relativo e o `App` passou a enviar delta junto da posição, o que segue útil para caminhos de mouse. A inspeção posterior dos scripts/GFX não encontrou hit-test por ponteiro na seleção da tela inicial; portanto **o delta não está confirmado como correção de seleção do app tocado**. O caminho atual encaminha posição/clique, enquanto o script ainda lê `GET_CURRENT_SELECTION` da seleção existente.
- Investigação `deep_movie` (2026-10-31): nos scripts descompilados, navegação usa seleção (`SET_INPUT_EVENT(1..4)`; rolagem 180/181 no modo teclado/mouse) e confirmação usa `GET_CURRENT_SELECTION`; os quatro GFX examinados não mostram handlers de clique/coordenada. Isso é evidência contra um hit-test direto já implementado, não prova de que nenhuma camada externa possa despachar um evento. O Import report registra os métodos do filme do celular (`SET_INPUT_EVENT`, `DISPLAY_VIEW`, `GET_CURRENT_SELECTION`) para o próximo teste no aparelho ajudar a decidir a injeção mais direta no limite filme/script.
- Próximo passo: implementar o editor arrastável na tela com IDs semânticos estáveis, posições persistidas e a mesma geometria compartilhada por desenho, hitboxes e áreas ocupadas; depois adicionar importação/exportação de presets XML versionados e validados. Manter fallback para o layout padrão e não alterar os passes gráficos.

## Status do celular e gate de integração (2026-11-01)

Acompanhar evidências e decisão do dono na issue Linear [GLU-27](https://linear.app/glue5/issue/GLU-27/touch-gate-do-celular-ciclo-mitigado-e-selecao-direta-pendente).

### Implementado

- O caminho de tap do telefone encaminha clique genérico de mouse; swipe continua gerando navegação direcional. **Não existe seleção direta demonstrada.**
- O Import report inclui `Recent phone movie methods`, limitado e sem coordenadas cruas: registra `SET_INPUT_EVENT(n)`, `DISPLAY_VIEW(viewID,currentID)` e `GET_CURRENT_SELECTION`, com timestamps compartilhados. O trace não registra o inteiro retornado por `GET_CURRENT_SELECTION`.
- `phone_enlarge` tem padrão `false` e é experimental. Isso evita a ampliação concorrente por padrão, mas não corrige o caminho de seleção.

### Testado em CI

- O PR #12 tem três checks verdes segundo o estado informado. Eles não são teste no aparelho nem fecham o gate de toque direto.

### Validado no aparelho

- O dono confirmou que `Bigger phone` desligado para o ciclo visual, a pé e em carro; ligado, o ciclo volta. Isso valida a mitigação de posição, não o toque direto. O teste foi feito com a opção explicitamente desligada; não há confirmação separada de um primeiro-start com valores padrão.
- O caso de seleção continua falhando: Michael pré-selecionado + toque em Trevor abre as mensagens de Michael. Portanto o toque direto não está validado.

### Teste diagnóstico recebido; A/B incompleto

- Em 2026-11-02, o dono enviou um Import report do build `f4add73`, com `Phone touchscreen input=true` e `Bigger phone=false`, após três tentativas. O dono relata que o toque direto não selecionou o alvo; o relatório cobre somente essa combinação, sem comparação com `Bigger phone=true`.
- No trace, o telefone sai uma vez (`phone out: true` por volta de +47,4 s) e volta uma vez (por volta de +91,2 s); a posição muda uma vez e permanece estável no restante da abertura. Não houve ciclo visual registrado nesta amostra, consistente com a mitigação `Bigger phone=false`; não foi um A/B comparando `true` contra `false`.
- Os gestos registram taps como `mouse_click` e dois passos `Up`; o filme registra `DISPLAY_VIEW(1,4)`, `SET_INPUT_EVENT(1)`, chamadas `GET_CURRENT_SELECTION` após taps, `DISPLAY_VIEW(6,0)` e, ao sair, `DISPLAY_VIEW(1,1)`. O trace não registra o valor retornado por `GET_CURRENT_SELECTION`, as coordenadas nem o item tocado. Isso mostra uma correlação temporal entre taps e consultas de seleção, mas não identifica qual alvo o jogo usou nem prova causalidade ou que `DISPLAY_VIEW` seja a fronteira de injeção.
- O dono esclareceu “nada”: a tentativa abriu o item que já estava pré-selecionado, não o alvo tocado. Isso confirma a falha de seleção direta neste cenário (não foi ausência de resposta) e coincide com o caso Michael pré-selecionado + toque em Trevor já registrado acima. O trace continua sem índice retornado nem alvo do toque, portanto não localiza a fronteira de injeção. Não repetir o mesmo ensaio como validação de correção: não há patch de seleção direta neste build. O próximo dado técnico útil é instrumentar o retorno/target.
- Não registrar neste roadmap os caminhos, nomes de saves, dados do jogador ou blocos de diagnóstico de GPU do relatório. O gate de correção continua: tocar item diferente do pré-selecionado deve abrir o alvo tocado, sem directional stepping nem mouse forwarding como prova, nas quatro variantes.

### Ainda não pronto / decisões do dono

- O editor de layout e o fluxo de presets editáveis não estão prontos nem validados. Preservar posições persistidas e presets; desenho, hit-test e áreas ocupadas devem continuar compartilhando uma fonte de geometria.
- O PR #12 segue aberto. Dividir mitigação e instrumentação, aceitar merge parcial ou adicionar índice interno a `docs/gta-patches.md` são decisões do dono, não deste chat. `docs/README.md` já aponta para `gta-patches.md`; ainda assim, perguntar se o dono quer um índice interno.

## Validação

- Tratar estes itens como backlog, sem presumir implementação ou resultados de teste.
- Validar cada mudança isoladamente e comparar execuções de benchmark apenas quando as condições forem equivalentes.
