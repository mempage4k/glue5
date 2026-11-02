# Paridade visual com o GTA V — verificação dos assets e estado atual

Registro de 2026-10-31, a partir dos arquivos colocados no Google Drive
(pasta `glue5/`) e de uma auditoria do código atual contra as sugestões de
redesign recebidas. **Nenhum asset do jogo entra neste repositório** — fonte,
filmes Scaleform, texturas, scripts e áudio extraídos são material da
Rockstar; ficam fora do git, como já acontece com as cenas (`docs/scenes.md`)
e os sons de menu (`docs/menu-sfx.md`), que o app lê dos arquivos do próprio
jogador em tempo de execução.

## Atualização de 2026-11-01 — lote completo recebido

O usuário subiu para `glue5/PACOTES ARENA/01-11 02:30` a extração completa
(20 ZIPs, 4.523 arquivos, 1,21 GB líquidos: filmes, fontes, `common.rpf`,
scripts, ActionScript descompilado e relatórios). O inventário verificado —
com hashes, proveniência, aliases de fonte e as constantes de estilo
medidas — vive em
[`gta5-reference-inventory.md`](gta5-reference-inventory.md) (CSVs em
`docs/gta5-referencia/`). O que muda neste documento:

- **As "duas cópias" do lote 1 foram desvendadas**: sufixo `(1)` = build
  *base* (byte-idêntica ao lote novo); sem sufixo = build *update*
  (filmes maiores, ex. `hud.gfx` 238.772 × 183.760 B). A referência do que
  o jogador roda hoje é a *update*; a base serve para estrutura.
- **Os 5 pedidos abaixo foram atendidos** (mapa em "Pedidos"): temos as
  bibliotecas de fonte e os TTF exportados (inclusive a condensada), o
  filme `popup_warning.gfx` com o ActionScript aberto, o `fontmap.xml`, o
  `hudcolor.dat` e 3 YSC descompilados. Falta descompilar os 22
  `.ysc.full` já descriptografados (a rodada em massa falhou).
- **Deltas medidos para o shell** (detalhes no inventário, §6.3):
  "branco" = RGB 240 (não 255); fundo de bloco = preto 73%
  (`0,0,0,186`); pop-up real = faixa de largura `textWidth+100`, altura
  `textHeight+21`, título em y=150 de 720, linhas brancas em cima/embaixo
  — o nosso `ui/popup.rs` é uma aproximação pelo vocabulário do shell,
  não esse desenho; o filme sequer tem aceitar/cancelar embutido.
  Alturas de linha de configurações: 27 px com realce `−2` (é o "respiro
  de ~2 px"), botões instrucionais 12 pt em palco de 720 px.

## O que há no Drive (verificado em 2026-10-31)

| Arquivo | Conteúdo | Utilidade |
|---|---|---|
| `assets/assets31-10-26-12-05.zip` (hoje) | 44 `.gfx` (hud, minimap, `pause_menu_pages_*`), 35 `.ytd`, 9 `.ysc` (`ingamehud`, `cellphone_*`, `pausemenu_*`), `frontend.xml`, `pausemenu.xml`, `pausemenueffects.meta`, `frontend.awc`, 2× `scaleform_frontend.rpf` (13,6 MB e 3,0 MB) | Referência direta de estrutura do HUD/peças do pause menu |
| `assets/cellphone.zip` (hoje) | 6 `.gfx` do celular, incl. `cellphone_alert_popup.gfx` | Estrutura do alerta do celular (`CREATE_ALERT`, `alertTitle`, `alertDialogBox`) |
| `chalet.woff` | **Chalet-LondonNineteenSixty Regular (400)**, 320 glifos, 1024 upem, exportada de um filme Scaleform pelo FFDec v26.3.0 | É o `$Font2` dos filmes — só o peso regular; ver "Fonte" |
| `GTA5-frontend-AWC-REL-para-Arena.zip` | `frontend.awc` + RELs de áudio | Sons de menu (já tratado em `docs/menu-sfx.md`) |
| `RESIDENT.rpf`, `pausemenu.m4a`, pasta `Claude/` | Áudio/pacotes de etapas anteriores | — |

Quase tudo vem em **duas cópias** (`arquivo` e `arquivo(1)`) com diferenças
reais: `pausemenu.xml` diverge em ~1200 linhas entre as duas, `frontend.xml`
em ~317, e alguns `.gfx` mudam de tamanho. São quase certamente duas fontes
(dados base × patch/update, ou builds diferentes). **Nas próximas extrações,
preserve o caminho de origem** (nome do RPF + caminho interno) em vez de
`nome(1)` — sem isso não dá para saber qual cópia corresponde aos dados que
o jogador realmente usa. Os `.ysc` estão crus (RSC7); descompilados são bem
mais úteis (ver pedidos no fim).

## Achados dos filmes `.gfx`

- **A fonte não está nos filmes.** Os 44 filmes importam bibliotecas
  compartilhadas: `gfxfontlib.swf` (todos) e `font_lib_efigs.swf` (13). Os
  nomes referenciados são `$Font2` (41 filmes), `$Font2_cond` (28),
  `$Font2_cond_NOT_GAMERNAME` (21), `$Font5` (21) e `$FixedWidthNumbers` (2),
  com tamanhos típicos de 12/15/18 nos textos embutidos
  (`<FONT FACE='$Font2_cond' SIZE='18'>` etc.). Para reproduzir a tipografia,
  o que precisamos extrair é a **biblioteca de fontes**, não cada filme.
- `pause_menu_pages_settings.gfx` confirma métodos como `SET_VIDEO_MEMORY_BAR`,
  `SET_DATA_SLOT`, `SET_DESCRIPTION`, `SET_FOCUS` — a página de Settings do V
  tem barra de memória de vídeo e descrição do item em foco (as nossas têm a
  descrição; a barra, não).
- O pop-up genérico do frontend **não está** neste lote: há o alerta do
  celular e só vestígios (`popupMC` em `pause_menu_pages_game.gfx`,
  `warningMC` no keymap). O filme-raiz do pause menu (o que hospeda abas,
  cabeçalho e warning screens) ficou dentro dos `scaleform_frontend.rpf`.

## Fonte (chalet.woff) — fatos e limite legal

- É a Chalet de verdade, mas **um peso só (400)** e um corte de filme: 320
  glifos, sem SemiBold/ExtraBold e sem a condensada (`$Font2_cond`) que os
  títulos do V usam. O `cmap` gerado pelo FFDec promete ~1600 caracteres para
  320 glifos — precisa de conferência glifo a glifo antes de confiar em
  acentos e símbolos (`‹ › · × —` …). **(2026-11-01: conferido. O lote novo
  traz as exportações TTF do FFDec de todas as faces do PC — `$Font2`,
  `$Font2_cond`, `$FixedWidthNumbers`, `$Font5`, Pricedown — 400 mesmo, 320
  glifos, acentos de pt-BR completos e `‹ › · × — – … ° •` presentes;
  faltam só `★` e o menos U+2212. Ver inventário, §5. A WOFF fica
  aposentada.)**
- egui/epaint lê TTF/OTF, **não WOFF**: precisa converter (o FFDec exporta o
  mesmo glifo também como TTF).
- **Não pode ser embarcada nem commitada.** É uma fonte comercial (House
  Industries) extraída de dados da Rockstar; o repo já registra isso
  (`crates/glue5/assets/ui/README.md`). O caminho compatível com as práticas
  do projeto é o mesmo das cenas/sons: uma pasta opcional do jogador
  (p. ex. `glue5-fonts/` junto aos arquivos do jogo) lida em tempo de
  execução, com Manrope como fallback — inclusive nas cadeias de famílias,
  para símbolos fora do corte caírem na Manrope. Isso é uma fatia própria,
  depois que tivermos também a condensada.

## Sugestões do Claude × código atual

O shell atual já absorveu parte (o print analisado era anterior a ele);
o resto, item a item:

| Sugestão | Estado | Evidência |
|---|---|---|
| Blocos pretos translúcidos com ~2 px entre itens, foco branco | **Parcial** | Foco branco já é o da linha (`widgets::row`); mas as linhas são separadas por regras de 1 px, não blocos, e nos menus de mods os itens ainda são cartões arredondados (`menus/rows.rs`, raio 14) |
| Cantos retos, paleta quase monocromática, cor só para estado | **Parcial** | Shell é quadrado e monocromático + lima; sobram pílulas/switch arredondados e o preenchimento lima do slider (`widgets::slider`/`toggle`, `menus/rows.rs`) |
| Seletores `‹ Valor ›` apontando para fora, só na linha em foco | **Parcial** | A direção está certa no código (`widgets::choice`, stepper dos mods), mas as setas aparecem **sempre** e o toggle continua `ON/OFF` com caixinha em vez de `‹ Off ›` |
| Settings como lista de categorias; fundir "GLUE 5" na barra de abas | **Não** | Categorias são uma grade 5×2 de blocos de 27 pt com texto de 9,5 pt (`pages.rs::categories`); o cabeçalho ocupa 48 pt próprios (`shell.rs`) |
| Painel direito contextual em todas as páginas; barra de memória/desempenho em Graphics | **Parcial** | Só PLAY e SETTINGS reservam a coluna de descrição (`shell::has_help_column`), e sem seleção ela mostra um texto-placeholder; barra de memória/desempenho não existe |
| Rodapé com glifos reais do controle, tocáveis; "‹ Voltar" no lugar do X | **Não** | Rodapé é texto (`A SELECT B BACK…`) e o fechar é um X desenhado (`shell.rs`, `widgets::close_button`) |
| Listas longas: contador "4/11" + ▲▼ em vez de linha cortada | **Não** | `ScrollArea` sem contador; o pop-up novo usa `1/2` no cabeçalho como primeiro uso do padrão |
| Pop-ups do V | **Feito (1ª fatia)** | `ui/popup.rs`: aviso modal escurecendo a página, título + texto + escolhas como blocos com foco invertido, `A SELECT / B BACK`, seleção inicial na opção segura. Ligado a Restore graphics (Settings), Clear the local copy (Settings › Files) e Delete da cópia em Servers — que aposentou o padrão "Sure?" de duplo toque |
| Acabamento: Chalet, arte em tela cheia com blur/vinheta, tique ao mover foco, linhas ≥ 48 dp | **Parcial** | Blur/acrílico já existem (painel de 62% da tela, `shell::panel_rect`) e o tique de foco também (`sfx`); fonte é Manrope e a linha do shell é 38 pt na escala 1,0 (`metrics::ROW`) |

Contexto que envolve tudo isso: os tamanhos de fonte estão espalhados
(~60 valores fixos entre 9,5 e 21 em `widgets.rs`, `shell.rs`, `pages.rs`,
`menus/*`, multiplicados à mão por `scale`), a família `"condensed"` é só um
alias da Manrope ExtraBold (os arquivos Barlow em `assets/ui/` nem são mais
carregados) e o piso do `layout_scale` (0,72) leva textos de 9,5–11 pt a
~7 pt no menor aparelho. Qualquer fatia tipográfica começa por reunir isso em
tokens no `theme.rs`.

## HUD e menus dentro do jogo (Scaleform)

O GLUE não redesenha esses textos: eles vêm dos filmes dos dados do jogo.
Os pontos de influência que o runtime já tem são (1) hooks de métodos
Scaleform (`glue5-runtime/src/prompts.rs`, `input.rs`, `script_assets.rs`),
(2) patches de mesmo comprimento em arquivos soltos servidos aos dados do
jogo (`overlay.rs`, hoje em `pausemenu.xml`/`frontend.xml` e controles) e
(3) nitidez via escala de renderização/upscaler — o census do HUD de
performance já mede a fase HUD separada da cena (`ui/perf.rs`). Trocar fonte
ou tamanho dentro dos filmes do usuário seria um projeto de patch em tempo de
carga sobre RPF, não uma mudança de tema; não prometer antes de um spike.

## Pedidos para as suas ferramentas (RPF/YSC) — status em 2026-11-01

Em ordem de valor para as próximas fatias:

1. ~~**Lista de arquivos dos dois `scaleform_frontend.rpf`**~~ — **Feito**:
   o lote traz os filmes extraídos por arquivo-fonte (`01-…` a `04-…`) e os
   9 `.rpf`; as bibliotecas moram em `scaleform_platform_pc.rpf` (PC) e
   `gfxfontlib.gfx` no `common.rpf`.
2. ~~**As bibliotecas de fontes extraídas**~~ — **Feito**: 50 TTF do FFDec
   por filme + `fontmap.xml` (inventário §5).
3. ~~**O filme-raiz do pause menu e o do frontend**~~ — **Feito o
   essencial**: `popup_warning.gfx`, `pause_menu_header`,
   `pause_menu_pages_settings` e os componentes compartilhados vieram com
   o ActionScript aberto (inventário §6). Falta exportar os demais filmes
   quando formos mirar cada tela.
4. **YSC descompilados** — **Parcial**: `ingamehud` e `cellphone_controller`
   (+`cellphone_flashhand`) vieram; `pausemenu_sp_repeat` e os demais
   prioritários estão descriptografados (`.ysc.full`) mas a descompilação
   em massa falhou — **este é o pedido nº 1 agora**: rodar o descompilador
   nos 22 `.ysc.full` já prontos (inventário §7.2).
5. Próximas extrações com **origem rotulada** (pasta por RPF/caminho
   interno) — desta vez veio organizado por arquivo-fonte ✔; segue
   pendente confirmar o RPF pai do `script_rel` e os prints do
   aparelho-alvo das telas atuais (Settings, Servers, um menu de mod) e de
   um pop-up de aviso do GTA para comparação lado a lado.
