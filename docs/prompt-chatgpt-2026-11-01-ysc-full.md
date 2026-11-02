# Prompt para o ChatGPT (rodada 2 — 2026-11-01)

Colar no ChatGPT com acesso ao PC (Desktop Commander). Objetivos em ordem:
descompilar os 22 `.ysc.full` já descriptografados, confirmar a
proveniência do `script_rel` (base × update), e — se sobrar fôlego —
ficha de estilo dos itens sem evidência e mais filmes no FFDec. Contexto e
verificação da rodada 1: `docs/gta5-reference-inventory.md` (§7, §9, §11).
A entrega volta como `ENTREGAS-GLUE5-YSC-02`, zipada, em
`glue5/PACOTES ARENA/<data-hora>/` no Drive.

---

Você está no PC do usuário (Desktop Commander), com acesso a arquivos e
shell. Continue o trabalho da sua entrega anterior (`ENTREGAS-GLUE5-YSC`).
Objetivo geral do projeto: deixar os menus do app GLUE 5 com paridade
visual máxima com os menus/HUD do GTA V (PC, Legacy). Tudo o que você
produzir é **material de referência**; nunca modifique arquivos do jogo
nem os originais extraídos — trabalhe sobre cópias.

## Onde as coisas estão neste PC

- Extração completa: `C:\GLUE5-EXTRACAO\assets-base-2026-10-31\` (pastas
  `00-rpf-crus`, `01-scaleform_frontend`, `02-scaleform_generic`,
  `03-scaleform_platform_pc`, `04-scaleform-outros`, `05-common-rpf`,
  `06-x64a-texturas`, `07-scripts-ysc-EXCECAO-update`, `08-fontes-ffdec`,
  `script_rel-EXTRAIDO`).
- Os 22 scripts já descriptografados: `C:\GTA5\Ferramentas\YSC-full-script-rel-temp\`
  (`*.ysc.full` + `DECRYPT-INDEX.csv` + `logs\`).
- Sua entrega anterior: `C:\GTA5\Ferramentas\ENTREGAS-GLUE5-YSC\`.
- Descompilador YSC usado antes: commit
  `2958d816e8f6db02a399c7b570c7074a0b2b1567`, CLI 1.0.0.0.
- FFDec 26.4.0.

## Contexto da rodada anterior (verificação independente já feita)

- Os 22 `.ysc.full` foram re-hasheados: todos íntegros vs.
  `DECRYPT-INDEX.csv`; os 22 `.ysc` fonte conferem com
  `script_rel-EXTRAIDO`. **A descriptografia funcionou — não refaça.**
- A descompilação em massa falhou: `DECOMPILATION-INDEX.csv` =
  `NO_OUTPUT_EXIT_` nos 22, logs de **0 bytes**, sem código de saída
  registrado. Os únicos `.c` existentes (`cellphone_controller.c`,
  `cellphone_flashhand.c`, `ingamehud.c`) saíram de rodadas separadas,
  um a um (pastas `YSC-full-temp`/`YSC-full-update-temp`).
- Seu `RELATORIO-ESTILO-HUD.md` foi conferido nas fontes primárias e está
  correto (fontmap, hudcolor, POPUP_WARNING, botões instrucionais,
  alturas 27/25 px). Mantenha o mesmo padrão de evidência.

## Fase 1 — Descompilar os 22 `.ysc.full` (prioridade máxima)

Protocolo anti-falha silenciosa (obrigatório):

1. Rode **um arquivo primeiro**: `pausemenu_map.ysc.full`. Registre o
   comando exato, stdout, stderr, **código de saída** e bytes/linhas do
   `.c` gerado. Mostre esse resultado antes de seguir.
2. Se falhar, diagnostique antes de continuar. Hipóteses a testar e
   registrar: requisito de runtime .NET; o descompilador espera a entrada
   nomeada `*.ysc.full` ou `*.ysc`; diretório de saída/permissões; o modo
   bulk da GUI funciona?; o que diferencia as rodadas que funcionaram
   (`YSC-full-temp`) desta?
3. Só então rode os demais, **um processo por arquivo**, com log
   individual não vazio (comando + stdout + stderr + exit code +
   bytes/linhas da saída). Se um arquivo falhar, registre o erro literal
   e siga para o próximo.

Ordem de prioridade (avise a cada lote concluído):

1. `pausemenu_multiplayer`, `pausemenu_map`, `pausemenu`,
   `pausemenu_sp_repeat`, `pausemenu_example`
2. `timershud`, `appsettings`
3. `appcontacts`, `appemail`, `apptextmessage`, `appcamera`, `appmedia`,
   `apptrackify`
4. `cellphone_controller`, `cellphone_flashhand` — regenerar pela mesma
   pipeline e comparar o SHA-256 com os `.c` da entrega anterior;
   se diferir, explique por quê
5. `completionpercentage_controller`, `achievement_controller`,
   `floating_help_controller`, `hud_creator`, `mission_stat_alerter`
6. `ingamehud` — comparar com o `.c` existente (exceção update);
   a saída é idêntica?
7. `appinternet` (`.full` de 2,8 MB; pode demorar — por último)

Os 22 são: achievement_controller, appcamera, appcontacts, appemail,
appinternet, appmedia, appsettings, apptextmessage, apptrackify,
cellphone_controller, cellphone_flashhand,
completionpercentage_controller, floating_help_controller, hud_creator,
ingamehud, mission_stat_alerter, pausemenu, pausemenu_example,
pausemenu_map, pausemenu_multiplayer, pausemenu_sp_repeat, timershud.

## Fase 2 — Proveniência do `script_rel` (base × update)

A pasta `script_rel-EXTRAIDO` não tem o RPF pai registrado, e o
`ingamehud.ysc` dela é byte-idêntico ao da exceção tirada do
`update.rpf`. Para resolver de uma vez:

1. Abra `C:\GLUE5-EXTRACAO\assets-base-2026-10-31\00-rpf-crus\SCRIPT.rpf`
   no OpenIV (**somente leitura**). Registre: de qual caminho físico da
   instalação do jogo ele foi copiado (se não estiver documentado,
   pergunte ao usuário e registre a resposta literal) e a árvore interna
   até os `.ysc` (existe um `script_rel.rpf` interno? em que caminho?).
2. Extraia dele estes 5 scripts: `ingamehud.ysc`,
   `cellphone_controller.ysc`, `appinternet.ysc`, `pausemenu_map.ysc`,
   `appsettings.ysc` (registre o caminho interno de cada um).
3. Compare o SHA-256 de cada extraído com as referências abaixo (são as
   cópias que hoje estão em `script_rel-EXTRAIDO`):

   | Script | Bytes | SHA-256 (script_rel-EXTRAIDO) |
   |---|---:|---|
   | ingamehud.ysc | 243234 | 269550E4CC23A94037431BB9D7525AF434D991AA503C64B184223E775C8FD6E7 |
   | cellphone_controller.ysc | 21331 | 93973B08B85771102FCB990E327CF3D48C28D10A13A014C431A11412C9DFF9A8 |
   | appinternet.ysc | 1127567 | 0ED656EBFFDF220F9528E7631DB1148979031DB197E245BDE71ACC38AE8AFDF4 |
   | pausemenu_map.ysc | 26740 | 7AE76C820908A6ADE45C15258CF95408A79D5DA46224172C25FB30AB2DED135A |
   | appsettings.ysc | 4210 | 410EFCD3E08A834E95E360E672E5D66F3FA0FA6DCF8C3FFA7AA8BF9D71B4B236 |

4. Conclusão explícita e escrita no manifesto: o `script_rel-EXTRAIDO`
   veio deste `SCRIPT.rpf` base, veio do `update.rpf`, ou os dois são
   idênticos nesses 5? Confira também o SHA-256 dos dois RPFs locais
   (referências: SCRIPT.rpf
   `406124651269DCA045577192E5072E0D06089D683F7236E235A8EAFE7EBEB5F4`;
   script_rel.rpf
   `37390D2B1EC872210DBAB1FFCB199BB468E44DB9D58DA72BA8C5B0AC09C889DC`).

## Fase 3 — Ficha de estilo, rodada 2 (só depois das Fases 1–2)

Com os novos `.c`, produza `RELATORIO-ESTILO-HUD-02.md` atacando **só as
lacunas** do relatório anterior, sempre com valores + função de origem
(nada de métricas por contagem de linhas que casam texto):

- Legendas/subtítulos: fonte (ID), escala, cor, contorno/sombra
  (edge/dropshadow), wrap, justificação, posição.
- Feed (THEFEED): fluxo genérico; durações/flash quando houver
  constantes nomeadas.
- Nomes de rua/área/veículo e dinheiro: quem chama o quê, com que
  fonte/escala/cor.
- Estrelas de procurado e mensagens grandes (wasted, mission passed):
  sequências de natives e escalas.
- Nos `pausemenu*.c`: como as telas chamam os filmes (métodos,
  argumentos), textos de título/descrição e os **SoundId literais** de
  foco/aceitar/voltar (ex.: nomes como `NAV_UP_DOWN`, `SELECT`, `BACK`
  — anote exatamente os que aparecerem, sem traduzir).
- Em `appsettings.c` e nos apps do celular: labels e escalas de texto.

Se algo continuar sem evidência, escreva "sem evidência". Não aproxime.

## Fase 4 (opcional) — Mais filmes no FFDec

Exporte o ActionScript (mesmo método da rodada anterior, com log) de:
`hud.gfx`, `instructional_buttons.gfx`, `custom_warning_screen.gfx`,
`pause_menu_pages_map.gfx`, `pause_menu_pages_game.gfx`,
`pause_menu_pages_info.gfx`, `pause_menu_pages_save.gfx`,
`pause_menu_pages_stats.gfx`, `cellphone_badger.gfx`,
`cellphone_ifruit.gfx`, `cellphone_facade.gfx`, `hud_saving.gfx`,
`hud_mission_passed_message.gfx`. Use as cópias das pastas `01-`/`02-`
(build base); se houver variante de update acessível, exporte também e
rotule as pastas `base`/`update`.

## Entrega

Nova pasta `C:\GTA5\Ferramentas\ENTREGAS-GLUE5-YSC-02\` (**não**
sobrescreva `ENTREGAS-GLUE5-YSC`):

- `MANIFESTO-02.md` — versões das ferramentas, comandos exatos usados,
  diagnóstico da falha anterior, falhas com erro literal, conclusão da
  Fase 2.
- `DECOMPILATION-INDEX-02.csv` — script, SHA-256 da entrada, SHA-256 da
  saída, bytes, linhas, status, caminho do log.
- `descompilados\` com os `.c`; `logs\`; `RELATORIO-ESTILO-HUD-02.md`
  (Fase 3); `scaleform-ui-source-02\` (Fase 4).

Empacote em ZIP(s) de até ~64 MB dentro de
`ENTREGAS-GLUE5-YSC-02\PARA-UPLOAD\`, junto com `LEIA-ME-ARENA-02.md` e
`INDICE-PACOTES-02.csv` (SHA-256 de cada ZIP), e peça ao usuário para
subir no Google Drive em `glue5\PACOTES ARENA\<data-hora>\` (mesma
convenção da rodada anterior). Relatórios em pt-BR. Reporte o progresso
a cada fase, não só no fim.
