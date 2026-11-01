# Inventário do material de referência do GTA V (Google Drive)

Registro de 2026-11-01, a partir da verificação **completa** do Google Drive
conectado ao projeto. Este documento é o mapa durável do material de
referência visual/de código do GTA V disponível para o GLUE 5: o que existe,
onde está, o que foi confirmado por hash/inspeção direta, o que ainda é
relato de terceiros e o que falta fazer. Ele complementa
[`gta5-visual-parity.md`](gta5-visual-parity.md) (parecer visual e pedidos) —
aqui o foco é **inventário e proveniência**.

**Regra de ouro (inegociável):** nenhum asset do jogo entra neste
repositório — nem fonte, nem `.gfx`, `.ytd`, `.ysc`, `.rpf`, áudio ou código
descompilado. Tudo isso é material da Rockstar e vive no Google Drive e no
PC do usuário. Neste repositório ficam só **fatos derivados**: nomes,
tamanhos, hashes, medidas (px/pt/RGBA), nomes de métodos públicos dos filmes
e conclusões. É o mesmo princípio já usado para `docs/scenes.md` e
`docs/menu-sfx.md`: o app lê dos arquivos do próprio jogador em tempo de
execução.

Os CSVs desta pasta (`docs/gta5-referencia/`) são metadados gerados por nós
(caminhos, tamanhos, SHA-256) — servem para conferir qualquer re-download
futuro mesmo que o Drive mude:

- `INVENTARIO-COMPLETO.csv` — 4.523 arquivos do lote de 2026-11-01
  (caminho, bytes, extensão, nota de split, SHA-256).
- `GFX-INVENTARIO.csv` — os 467 filmes `.gfx` (pasta, bytes, magic, SHA-256).
- `FONTES-ANALISE.csv` — as 50 fontes TTF analisadas por parsing das
  tabelas `name`/`maxp`/`head`/`OS/2`/`cmap`.
- `INDICE-PACOTES.csv` — índice oficial dos 20 ZIPs (hash de cada ZIP).
- `MANIFESTO-ARQUIVOS-DIVIDIDOS.csv` — como remontar os 3 arquivos grandes.

> **Lição de 2026-11-01:** o workspace do sandbox **não persiste** — nem
> entre sessões, nem entre turnos: fora do repositório, tudo some (os
> downloads do lote 1 e a árvore extraída de 2,6 GB desta sessão
> desapareceram; só o repositório `glue5/` se manteve). A fonte da verdade
> é o **Google Drive** (+ o PC do usuário). Por isso os IDs do Drive e a
> receita de reconstrução (§10) são registrados aqui, e é no repositório
> que ficam os documentos e CSVs de metadados.

---

## 1. Mapa do Google Drive (estado em 2026-11-01, verificado item a item)

O Drive tem 93 itens no total; o escopo do projeto é a pasta `glue5/`
(id `1BoxFkMEoNrTCfRDzt156g7miXOKIOP2q`) e o `RESIDENT.rpf` da raiz. O
restante da raiz (documentos, imagens pessoais, outros jogos) está fora de
escopo e **não foi baixado nem inspecionado**.

### 1.1 `glue5/PACOTES ARENA/01-11 02:30` — lote completo atual

Pasta `PACOTES ARENA` `1dP18XFCtZbNuRNJHybdfDMi3s6FY3auv`, subpasta
`01-11 02:30` `1lSUYIU-LCMLlXFO7QLqSBRX_tDwlPyV1`, enviada em
2026-11-01T05:28–05:30Z. 24 arquivos: **20 ZIPs `GLUE5-ARENA-NN-DE-20.zip`
+ 4 arquivos de índice**. É a resposta ao guia de extração base/OpenIV e ao
prompt de descompilação YSC preparados em 2026-10-31.

Índices (IDs no Drive):

| Arquivo | Drive ID | Bytes |
|---|---|---:|
| `LEIA-ME-ARENA.md` | `14BYGyVvydIV-tKQ3xm6pNriAE2PAh_Zh` | 954 |
| `INDICE-PACOTES.csv` | `1a1Xdett1tvaO28wJS-wzFHAQmTtB5JyT` | 2.335 |
| `MANIFESTO-ARQUIVOS-DIVIDIDOS.csv` | `1T2JIH6vqKJnZggbuURXsU9GOL1RPzuys` | 1.757 |
| `RECONSTRUIR-ARQUIVOS-GRANDES.ps1` | `1k4rmXOtVOgsRXWfHL4JV6rseMmRKasoG` | 1.274 |

ZIPs (01–20, IDs em ordem): `1we830FEUseHoDWj9Mg8NQw8oFi3f0IMj`,
`1O9-4ZvBt5WZULOIAR1W255roHweaxuV_`, `1vRWndx8Ccail_DIgX0AY52zb7EYrteze`,
`1qWWQ5HVmwR2Pd0eirU2jnipyKMOD1M4i`, `1y1MgTYPXVnwWZ3h2lwSSIvjTWrKkf6VR`,
`1zr3WCPpb6x1c_6KdM8V6O4a8KoSCyzTo`, `1-3VCdxA_lsrAPiAaVKfu2qYw_UsFHhK0`,
`12yZPNF26xaoLJi4LfjKFUzN07s6myAVR`, `1YG7_aCcpxIDOZeYnuuTxpQ9Oc3V-CqZd`,
`15-2-lL-_S-yEZiSKWLRZ1ZMKQnXyMmXi`, `1zxK8MCZwy55uMq7kH4PgN2t61UqCyQHN`,
`1KYY6Skst73PFrEiF3vFRwLXPSwOWYqt1`, `1H46l8OHjFjiL63IdPYM78Pg66OqcA7gk`,
`1iqoNOlZ2PGYiGtEz7NKBBdh4MHcQDTfU`, `1gflr3tHRcvdKEYGzJaps3IAU3nxU2mp2`,
`1KOt11LOc8xeAGdcy30LXVhXsJLJp7S6C`, `1gVyfDiHzcFVKZqhJTsNS1HlXI9HD5Wzt`,
`1fDmNJ9_2UThA-AH1R5VMS7BEKUR_LsER`, `1V88eaAroiYVMnEoN8Kd9G_dISMqv4eFe`,
`1a_0kohv3P292YNu5L-XJK46m8Ai1aa4V`. Tamanhos: 0,36–70 MB por parte,
**893 MB no total**.

### 1.2 Outros itens do projeto no Drive

| Caminho | Drive ID | Bytes | Data | O que é |
|---|---|---:|---|---|
| `assets/assets31-10-26-12-05.zip` | `1xl5IfkFnNqFT-th7u28-RQtCkGvsgvLH` | 27.261.683 | 2026-10-31 | **Lote 1**: 96 membros (44 `.gfx`, 35 `.ytd`, 9 `.ysc`, 4 `.xml`, 2 `.rpf`, 1 `.awc`, 1 `.meta`). Mistura **duas builds** — ver §8.1. SHA-256 do ZIP: `DC4F28AC788F05EC…` |
| `assets/cellphone.zip` | `1sC3jp1hLRengyw3CYmaiqRa5gWo9zq0C` | 130.954 | 2026-10-31 | 6 `.gfx` do celular; **redundante** com o lote atual (bytes idênticos em `02-scaleform_generic`). SHA-256: `8FF770F8649B4B31…` |
| `chalet.woff` | `1lftbeOTFZ4S8WyL8UdZyuL3LNBdUM5Hj` | 26.952 | 2026-10-27 | Chalet-LondonNineteenSixty Regular em WOFF artesanal (o campo `totalSfntSize` é igual ao tamanho do arquivo — exportação imperfeita). **Superada** pelos TTF do FFDec (§5). SHA-256: `CB7B6BDD85DDA67D…` |
| `pausemenu.m4a` | `1mbceEH8YZ8jWwMbkI7m4MB5JRvcP6xsp` | 31.344.316 | 2026-10-27 | Áudio AAC estéreo 48 kHz, **21min28s**, ~195 kbps, codificado por FFmpeg (`Lavf60.16.100`) — é uma **gravação/captura** (não um arquivo do jogo); baixado e sondado em 2026-11-01, conteúdo não ouvido. SHA-256: `90DE3F3BD92E00FB…` |
| `GTA5-frontend-AWC-REL-para-Arena.zip` | `148yxIysIGelFSjYpkmKavQZwlbrqlIQl` | 9.075.238 | 2026-10-28 | `AWC/frontend.awc` (6.060.128) + 17 `.rel` de configuração de áudio (`game.dat151.rel`, `sounds.dat54.rel`, `speech2.dat4.rel`, …) + `packlist_rel.txt`. Base do trabalho de SFX (`docs/menu-sfx.md`). SHA-256: `A677CDCF5F03DEB4…` |
| `RESIDENT.rpf` (raiz) | `1B9ELY8TNIXV5S6yYnnHl4hFvr4nPWf-3` | 5.330.944 | 2026-10-27 | Arquivo RPF7 de áudio (banco resident), baixado em 2026-11-01: magic `7FPR`, campos do header `17 / 0x10000100 / 0x0FFFFFF9` (constante de criptografia RPF7) e TOC de alta entropia logo após o header — **não abrimos** (precisa OpenIV/CodeWalker no PC). SHA-256: `3AB908CFE52681D7…` |
| `Claude/glue5-main-start-page.zip` | `1m6QLNh2tYPaX85dbX276vJXdBa6J10qj` | 4.221.305 | 2026-10-28 | Snapshot de código do GLUE 5 (528 membros: 222 `.rs`, 59 `.png`, 37 `.patch`, ícones de plataforma etc.) da etapa da start page. SHA-256: `71D2A9B092A7C5D0…` |
| `Claude/glue5-scenes.zip` | `1cWKuyzg_5KJR1qHx6CcO368pML4gfx-Q` | 6.624.918 | 2026-10-28 | Cenas da start page: `scenes.json`, `LEIA-ME.txt` e 8 PNG (background+figure de story/online/benchmark/sandbox). SHA-256: `E3F6718183CDB021…` |
| `Claude/start-page.patch` | `18bXstgEoyxNQny6AYJpJWieCHcvCT8vu` | 105.870 | 2026-10-28 | Diff unificado da start page (cenas lidas em runtime, gradiente desenhado em código). |

Item hoje **ausente** do Drive: um `RESIDENT-AWC-para-Arena.zip` visto em
sessões anteriores não existe mais na listagem; o que há é o `RESIDENT.rpf`
acima. (Na raiz há ainda um `Download.7z` de 345 MB — **não é do projeto**,
confirmado pelo usuário em 2026-11-01: tudo do projeto fica na pasta
`glue5/`; o único item do projeto fora dela é o `RESIDENT.rpf`, enviado
junto aos demais em 2026-10-27 — vale movê-lo para dentro de `glue5/`.)

---

## 2. Verificação do lote `PACOTES ARENA` (o que foi comprovado aqui)

Baixamos os 24 arquivos para fora do repositório
(`…/gdrive-ref/pacotes-2026-10-31/zips/`) e verificamos:

1. **SHA-256 dos 20 ZIPs confere com o `INDICE-PACOTES.csv`** (20/20).
2. **`testzip` íntegro** nos 20 ZIPs (nenhum membro corrompido).
3. **4.520 membros** extraídos no total.
4. **3 arquivos divididos reconstruídos e validados** por tamanho/offset
   do manifesto (concatenação ordenada das partes `_SPLIT`, equivalente ao
   `.ps1` fornecido — feito em Python):
   - `00-rpf-crus/SCRIPT.rpf` — 237.871.104 B (magic RPF `7FPR` ok);
   - `05-common-rpf/data/levels/gta5/paths.xml` — 145.182.312 B;
   - `script_rel-EXTRAIDO/script_rel.rpf` — 241.391.616 B (magic ok).
5. Inventário próprio gerado: **4.523 arquivos** com SHA-256 individual
   (`INVENTARIO-COMPLETO.csv`).

Regras de empacotamento declaradas no `LEIA-ME-ARENA.md`: origem
`…\GLUE5-EXTRACAO\assets-base-2026-10-31`; payload ≤ ~70 MB por ZIP;
excluída a pasta `SCRIPT-RPF-EXTRAIDO` (extração errada de `.awc`); nenhum
arquivo de chaves/senhas; splits apenas nos 3 arquivos acima (11 partes de
≤ 64 MB, offsets no manifesto).

⚠️ **Divergência importante de proveniência:** os relatórios dentro de
`ENTREGAS-GLUE5-YSC/` (`MANIFESTO.md`, `PROXIMA-ETAPA-OPENIV.md`) foram
escritos **antes** do empacotamento final e dizem que RPFs crus, `.ysc` e
`.ysc.full` ficariam só no PC, que `07-scripts-ysc` está vazio e que a
extração base ainda não existia. **O lote real contém tudo isso** (9 RPFs,
1.030 `.ysc`, 22 `.ysc.full`). Em caso de conflito, vale o inventário de
arquivos deste documento, não o manifesto antigo.

---

## 3. Conteúdo do lote — visão geral

Totais: 4.523 entradas no inventário = 4.489 arquivos-fonte + 20 notas
`00-INFO` (1 por ZIP) + 11 partes `_SPLIT` + 3 arquivos reconstruídos.
Volume bruto com duplicação split+remontado: 1.838.324.174 B; **volume
líquido único: 1.213.879.142 B** (descontadas as 11 partes, 624.445.032 B).

Por extensão (principais): `.ytd` 1.054 · `.ysc` 1.030 · `.gfx` 467 ·
`.sps` 391 · `.fxc` 321 · `.meta` 318 · `.xml` 214 · `.as` 206 · `.png` 145
· `.txt` 91 · `.dat` 50 · `.ttf` 50 · `.ugc` 40 · `.full` 22 · `.ipl` 15 ·
`.rpf` 9 · `.dds` 8 · `.c` 3.

| Pasta (raiz do lote) | Arquivos | Bytes | Conteúdo |
|---|---:|---:|---|
| `_SPLIT/` | 11 | 624.445.032 | Partes dos 3 arquivos grandes (apagar depois de remontar). |
| `assets-base-2026-10-31/script_rel-EXTRAIDO/` | 1.030 | 482.468.999 | `script_rel.rpf` remontado + 1.029 `.ysc` (§7). |
| `assets-base-2026-10-31/05-common-rpf/` | 1.379 | 303.868.510 | `common.rpf` extraído: `data/` (incl. `data/ui`, `levels/gta5/paths.xml` remontado) + `shaders/` (§4). |
| `assets-base-2026-10-31/00-rpf-crus/` | 1 | 237.871.104 | `SCRIPT.rpf` base remontado (não aberto por nós). |
| `assets-base-2026-10-31/04-scaleform-outros/` | 287 | 51.226.701 | `scaleform_web` (sites `www_*.gfx`), `scaleform_minimap`, `scaleform_minigames` + YTDs. |
| `assets-base-2026-10-31/02-scaleform_generic/` | 1.093 | 40.877.005 | Filmes genéricos (HUD, celular, páginas do pause) + YTDs companheiros. |
| `assets-base-2026-10-31/03-scaleform_platform_pc/` | 44 | 33.461.790 | Bibliotecas de fonte do PC + filmes de plataforma. |
| `assets-base-2026-10-31/06-x64a-texturas/` | 211 | 27.536.286 | `textures/*.ytd` + exportações PNG (`frontend-png`, `graphics-png`, `hud-png-…`). |
| `assets-base-2026-10-31/08-fontes-ffdec/` | 51 | 20.166.169 | 50 TTF exportados pelo FFDec + log (§5). |
| `assets-base-2026-10-31/01-scaleform_frontend/` | 38 | 6.951.597 | Filmes do frontend/pause + YTDs. |
| `YSC-full-script-rel-temp/` | 67 | 5.197.850 | 22 `.ysc.full` descriptografados + JSONs + logs (§7). |
| `ENTREGAS-GLUE5-YSC/` | 287 | 4.003.923 | Relatórios, 3 `.c` descompilados, ActionScript de 8 filmes, CSVs (§9). |
| `assets-base-2026-10-31/07-scripts-ysc-EXCECAO-update/` | 1 | 243.234 | Só `ingamehud.ysc` — exceção consciente de `update.rpf`. |

Os 9 `.rpf` do lote: `SCRIPT.rpf` 237 MB, `script_rel.rpf` 231 MB,
`common.rpf` ~27 MB, `scaleform_web.rpf` ~22 MB,
`scaleform_generic.rpf` ~18 MB, `scaleform_platform_pc.rpf` ~16 MB,
`scaleform_frontend.rpf` ~3 MB, `scaleform_minimap.rpf` ~0,7 MB,
`scaleform_minigames.rpf` ~0,1 MB (tamanhos aproximados = arquivo já
extraído, ver CSV).

---

## 4. `common.rpf` — a pasta de configurações de UI

`05-common-rpf/data/ui/` é, para o nosso trabalho, a pasta mais valiosa
depois dos filmes. Contém (espelhados em `ENTREGAS-GLUE5-YSC/referencias-ui/`,
hashes conferem):

- **`fontmap.xml`** (6.107 B) — mapa alias→família por região (§5).
- **`frontend.xml`** (26.189 B) e **`pausemenu.xml`** (209.670 B) —
  estrutura de telas do frontend/pause (RTTIs de telas/itens).
- **`hudcolor.dat`** (18.465 B) — paleta nomeada completa (seções SD/HD);
  valores usados em §6.3.
- `controls.xml`, `gamestream.xml`, `mpstatssetupui.xml`,
  `spstatssetupui.xml`, `reportplayer.xml`, `dialoguecharacters.meta`,
  `mapzoomdata.meta`, `pausemenueffects.meta`, `uiscenes.meta`.

Também relevantes: `data/scaleformpreallocation.xml`,
`data/scaleform/` e `shaders/win32_40_final/scaleform_shaders.fxc`. O grosso
dos 303 MB é `data/levels/gta5/paths.xml` remontado (145 MB) e os shaders —
fora do escopo de paridade visual de menus, mas inventariados no CSV.

---

## 5. Fontes — a resposta definitiva do "qual é a fonte do GTA V"

### 5.1 Aliases (`fontmap.xml`, variante ocidental/EFIGS)

O `$` é fixo ("the '$' names here CANNOT be changed", diz o próprio XML):

| Alias | Família real | Exportação PC disponível (§5.2) |
|---|---|---|
| `$Font2` | **Chalet-LondonNineteenSixty** | `font_lib_efigs_pc/fonts/9_$Font2_…ttf` |
| `$Font2_cond` | **ChaletComprime-CologneSixty** | via `$Font2_cond_NOT_GAMERNAME` (mesma face) |
| `$Font2_cond_NOT_GAMERNAME` | ChaletComprime-CologneSixty | `5_$Font2_cond_NOT_GAMERNAME_…ttf` |
| `$Font2_cond_MPTimer` | GTAV Timer Fixed Cond | `1_$Font2_cond_MPTimer_…ttf` |
| `$Font5` | SignPainter-HouseScript | `4_$Font5_…ttf` |
| `$FixedWidthNumbers` | ChaletLondonNineteenSixtyNumb | `6_$FixedWidthNumbers_…ttf` |
| `$gtaCash` / `$MISSION_MESSAGE` | PricedownGTAVInt | `3_$MISSION_MESSAGE_…ttf` |
| `$GTAVLeaderboard` | GTAV LeaderBoard | `2_$GTAVLeaderboard_…ttf` |
| `$RockstarTAG` | RockstarTAG | `8_$RockstarTAG_…ttf` |
| `$HandstyleHeist` | Handstyle Heist | `font_lib_heists/fonts/1_…ttf` |
| `$SOCIAL_CLUB_COND_REG` | HelveticaNeue LT 47 LightCn | `font_lib_sc/fonts/1_…ttf` |
| `$SOCIAL_CLUB_COND_BOLD` | HelveticaNeue LT 67 MdCn | `font_lib_sc/fonts/2_…ttf` |
| `$Taxi_font` | GTAVTaxiDigital | `font_lib_taxi/fonts/1_…ttf` |
| `$Courier` | GTAV Courier | `font_lib_typewriter/fonts/1_…ttf` |
| `$Font6` | GTAV Casino Dots | `font_lib_slots/fonts/1_…ttf` |
| `$WebFont1_Hbold` / `$WebFont2_Hitalic` | HelveticaNeueLT W1G 55 Roman | `font_lib_web/fonts/3_…` / `2_…ttf` |
| `$WebFont3_Times` | Times New Roman CE | `font_lib_web/fonts/1_…ttf` |
| `$WebFont4_Courier` | Courier Twelve MT Std | `font_lib_web/fonts/4_…ttf` |

Notas:
- Nas regiões KR/JP/CN quase todos os aliases trocam para HYRKorean /
  MotoyaExGothic / MHeiHK-Bold — **não** usar o TTF ocidental para essas
  variantes.
- O `gfxfontlib.gfx` (biblioteca compartilhada antiga, em
  `05-common-rpf`) embute outra `$Font2` = *HelveticaNeue LT 65 Medium* e
  fontes de fallback (`$Font1` Arial, `$Font3` Andy Std, `$Font4`
  Zeitgeist MT Std). A face PC atual é a de `font_lib_efigs_pc.gfx`
  (`03-scaleform_platform_pc`, 232.318 B).

### 5.2 As 50 exportações TTF (FFDec 26.4.0) — análise própria

`08-fontes-ffdec/` guarda exportações **por filme** (12 filmes: `font_lib_sc`,
`efigs`, `efigs_pc`, `chinese_pc`, `japanese_pc`, `korean_pc`, `heists`,
`slots`, `taxi`, `typewriter`, `web`, `gfxfontlib`), 20.158.716 B no total
(hashes individuais em `ENTREGAS-GLUE5-YSC/FONTES-INVENTARIO.csv` — conferem
com o nosso inventário). Nós parseamos as 50 TTFs (`FONTES-ANALISE.csv`).

Faces principais do PC (`font_lib_efigs_pc`), verificadas **glifo a glifo**
na `cmap` formato 4 (gid ≠ 0), não só pelo nome do arquivo:

| Arquivo | Bytes | Glifos | upem | A–Z/a–z/0–9 | Acentos pt-BR | Símbolos de menu |
|---|---:|---:|:--:|:--:|:--:|---|
| `9_$Font2_Chalet-LondonNineteenSixty.ttf` | 75.560 | 320 | 1024 | ✔ | ✔ completo | `‹ › · × — – … ° •` ✔; faltam `★` e `−` (U+2212) |
| `5_$Font2_cond_NOT_GAMERNAME_ChaletComprime-CologneSixty.ttf` | 72.712 | 320 | 1024 | ✔ | ✔ completo | idem |
| `6_$FixedWidthNumbers_ChaletLondonNineteenSixtyNumb.ttf` | 19.152 | 305 | 1024 | ✔ | ✔ completo | idem |
| `3_$MISSION_MESSAGE_PricedownGTAVInt.ttf` | 60.168 | 318 | 1024 | ✔ | ✔ completo | idem |
| `4_$Font5_SignPainter-HouseScript.ttf` | 138.232 | 317 | 1024 | ✔ | ✔ completo | idem |

Todas peso 400 (Regular) — **o GTA V não tem bold**; ênfase é
tamanho/cor/caixa. Caveats de qualidade:

- São reconstruções do FFDec a partir de `DefineFont` dos filmes: a `cmap`
  é sintética (1.648 codepoints mapeados para ~320 glifos — faixas inteiras
  apontam para poucos glifos). Cobertura real testada acima ≈ 218 glifos
  distintos só no Latin básico+estendido. Hinting inexistente. Para uso em
  runtime (se um dia aprovado: carregar dos arquivos do jogador) é
  aceitável; para embarcar no app, **proibido** (fonte comercial
  House Industries).
- A face condensada é byte-idêntica entre libs (`chinese_pc` =
  `efigs_pc` NOT_GAMERNAME, SHA `3CAEBEFA…`; `japanese_pc` = `korean_pc`,
  SHA `EF7F62C4…` mas 72.712 B também — sets de glifos regionais diferem).
  A `$Font2` de `font_lib_efigs` (console) tem 273 glifos; a do PC, 320.
  **Usar sempre as exportações `_pc`.**
- A `chalet.woff` antiga é a mesma face `$Font2`, mas em WOFF montado à mão;
  fica aposentada em favor do TTF acima.
- Curiosidade: o `$gtaCash` em `font_lib_efigs` exporta uma "Arial" de
  273 glifos (a Pricedown de verdade está no PC como `$MISSION_MESSAGE`).

---

## 6. Filmes Scaleform `.gfx` (467) e o ActionScript já extraído

Distribuição: `02-scaleform_generic` 243 · `04-scaleform-outros` 196 ·
`01-scaleform_frontend` 19 · `03-scaleform_platform_pc` 8 ·
`05-common-rpf` 1 (`gfxfontlib.gfx`). 466 começam com `GFX` (não
comprimido) e 1 com `CFX` (comprimido). **Nenhum hash duplicado** entre os
467. A lista completa com tamanhos está em `GFX-INVENTARIO.csv`.

### 6.1 Filmes prioritários presentes (82)

- **Frontend/pause**: `pause_menu_header`, `pause_menu_instructional_buttons`,
  `pause_menu_pages_settings`, `pause_menu_shared_components` (+`_02`,
  `_03`, `_mp_01`), `pause_menu_sp_content`, `pause_menu_calibration`,
  `popup_warning`, `loadingscreen_newgame`, `loadingscreen_startup`,
  `social_club2` e todas as `pause_menu_pages_*` (awards, char_mom_dad,
  char_select, corona*, crews, friends*, gallery, game, info, map,
  missioncreator, save, stats, store, weapons), `pause_menu_spchar_overlays`,
  `pause_menu_store_content`, `custom_warning_screen`.
- **HUD**: `hud` (183.760 B), `hud_area_name`, `hud_street_name`,
  `hud_district_name`, `hud_vehicle_name`, `hud_cash`, `hud_cash_change`,
  `hud_mp_cash`, `hud_mp_message`, `hud_mission_passed_message`,
  `hud_player_switch_alert`, `hud_reticle`, `hud_saving`,
  `hud_multiplayer_chat`, `hud_drugs_purse_01..04`, `instructional_buttons`,
  `store_instructional_buttons`, `remote_sniper_hud`.
- **Celular**: `cellphone_alert_popup`, `cellphone_badger`,
  `cellphone_facade`, `cellphone_ifruit`, `cellphone_cutscene`,
  `cellphone_prologue`.
- **Fontes**: `font_lib_efigs_pc`, `font_lib_chinese_pc`,
  `font_lib_japanese_pc`, `font_lib_korean_pc`, `font_lib_efigs`,
  `font_lib_sc`, `font_lib_heists`, `font_lib_slots`, `font_lib_taxi`,
  `font_lib_typewriter`, `font_lib_web`, `gfxfontlib`.
- **Minimap/web**: `minimap`, `minimap_main_map`, `web_browser` + 81 sites
  `www_*.gfx` (em `04-scaleform-outros/scaleform_web`), minigames
  (golf/tennis…).

A maioria dos filmes tem um `.ytd` companheiro de mesmo nome nas mesmas
pastas (texturas referenciadas pelo filme) — parte dos 1.054 `.ytd`.

### 6.2 ActionScript exportado (8 filmes, em `ENTREGAS-GLUE5-YSC/scaleform-ui-source/`)

Exportado com FFDec 26.4.0, exit code 0; log registra 8 avisos de
"SWF externo ignorado" (imports de `gfxfontlib.swf`/font libs não são
resolvidos — as classes locais vieram inteiras):

| Filme | `.as` | `.txt`* | Peças-chave |
|---|---:|---:|---|
| `popup_warning` | 25 | 9 | `POPUP_WARNING.as`, `INSTRUCTIONAL_BUTTONS.as` |
| `pause_menu_header` | 28 | 7 | `PAUSE_MENU_HEADER.as` |
| `pause_menu_instructional_buttons` | 21 | 1 | `PAUSE_MENU_INSTRUCTIONAL_BUTTONS.as` |
| `pause_menu_pages_settings` | 27 | 24 | `PAUSE_MENU_PAGES_SETTINGS.as`, `PauseMenuSettingsTextItem.as` |
| `pause_menu_shared_components` | 46 | 9 | Listas (`PAUSE_MENU_VERTICAL_LIST`, `PAUSE_MENU_TEXT_LIST_DOUBLE`), `GTAVUIConfig.as` |
| `pause_menu_shared_components_02` | 33 | 5 | idem (variante MP) |
| `cellphone_alert_popup` | 5 | 1 | `CELLPHONE_ALERT_POPUP.as` |
| `tattoo_keys` | 21 | 0 | (teclas da tatuagem) |

\* `.txt` = dumps de textos/recursos exportados junto.

### 6.3 Constantes de estilo **conferidas por nós** nas fontes primárias

Estes números foram relatados no `RELATORIO-ESTILO-HUD.md` da entrega e
**nós os re-verificamos** nos arquivos (`fontmap.xml`, `hudcolor.dat`,
`POPUP_WARNING.as`, `INSTRUCTIONAL_BUTTONS.as`, `PAUSE_MENU_HEADER.as`,
`PauseMenuSettingsTextItem.as`, `PAUSE_MENU_TEXT_LIST_DOUBLE.as`). Bateram.

**Cores** (`hudcolor.dat`, seção HD; IDs em `HudColour.as`):

| Nome | ID | RGBA |
|---|---:|---|
| `HUD_COLOUR_WHITE` | 1 | 240, 240, 240, 255 |
| `HUD_COLOUR_BLACK` | 2 | 0, 0, 0, 255 |
| `HUD_COLOUR_PAUSE_BG` | 117 | 0, 0, 0, 186 |
| `HUD_COLOUR_INGAME_BG` | 140 | 0, 0, 0, 186 |

Ou seja: o "branco" do GTA é **240** (não 255) e os fundos de bloco são
preto a **73%** (186/255).

**Pop-up de aviso** (`popup_warning.gfx`, classe `POPUP_WARNING`,
métodos `SHOW_POPUP_WARNING(msecs,titleMsg,warningMsg,promptMsg,showBg,
alertType)` / `HIDE_POPUP_WARNING(msecs)`), referência de palco 1280×720:

- Tipo 0 (padrão): linha superior e inferior `HUD_COLOUR_WHITE`, faixa
  central `HUD_COLOUR_INGAME_BG`.
- Sem título → localização `GLOBAL_ALERT_DEFAULT`.
- Título em **y = 150**; corpo em `y = título.y + título.textHeight + 4`,
  multilinha, centrado, auto-size; `promptMsg` é concatenado ao corpo.
- Faixa: **largura = `labelTF.textWidth + 100`**, **altura =
  `labelTF.textHeight + 21`**, posicionada em **y = corpo.y − 7**; o
  conjunto é centralizado contra a altura de referência **720**
  (`(720 − alturaTotal) × 0,5`, com 12 px de respiro).
- `showBg` controla o fundo escurecido; tipo 1 = painel de calibração.
- `fadeSpeed = 300` existe mas **não é usado** por SHOW/HIDE; o parâmetro
  `msecs` também não é consumido. Não citamos 300 ms como duração de fade.
- A classe **não implementa aceitar/cancelar** — quem chama (script)
  decide; o filme só mostra/esconde (alpha 0/100) e herda a fileira de
  botões instrucionais.
- Bônus: `SET_LIST_ROW` monta linhas de 27 px (nome 18 pt, nível 22 pt).

**Botões instrucionais** (`INSTRUCTIONAL_BUTTONS.as`, linhas 29/32/39/
59/61/76/82 conferidas):

- `PADDING = 6`, `TEXT_BOX_PADDING = 1`, `iconSize = 24` (ícone 20×20 em
  x=2, y=2 dentro do slot de 24), espaçamento interno **6 px**, entre
  instruções **12 px** (`spaceBetweenInstructions`), `_leadingSize = 2`.
- Texto: `$Font2` **12 pt** branco; trecho de gamertag (`<C>…</C>`):
  `$Font2_cond` **14 pt**.
- Fundo padrão `HUD_COLOUR_INGAME_BG`; a variante de pausa usa
  `HUD_COLOUR_PAUSE_BG`. Itens quebram em várias linhas ao exceder a
  largura; `SET_DATA_SLOT` aceita strings (texto), números (ícones) e
  tokens com `%`, prefixo `t` = tecla textual.

**Cabeçalho do pause** (`PAUSE_MENU_HEADER.as`):

- `SET_TEXT_SIZE`: padrão 13 (variante asiática 16), override limitado a
  13–18. Título em `$Font2_cond_NOT_GAMERNAME`, descrição em `$Font2`.
- Título: 36/31 pt, 32/28 pt ou 26 pt conforme o comprimento da
  descrição; descrição 13→11 pt com leading −1,6/−2/−2,2/−3. Título de
  desafio encolhe para caber, mínimo 13 pt.

**Linhas de configurações/listas**:

- Item de configurações: **27 px** de altura (último item **25 px**); o
  realce branco tem `altura − 2` — **é daí que nascem os ~2 px de respiro**
  entre blocos que víamos nas capturas. Fundo normal `HUD_COLOUR_PAUSE_BG`,
  realce branco com texto preto.
- `PAUSE_MENU_TEXT_LIST_DOUBLE`: `rowSpacing = −2`, máscara de 430 px,
  15 itens visíveis, `visibleItems: 15`, estilo `SCROLLALL`.
- Não há um tamanho único de fonte para todas as linhas: passa por
  `UIText.setSizedText` por componente.
- `GTAVUIConfig.as`: `DEFAULT_FONT = "$Font2"`,
  `CONDENSED_FONT = "$Font2_cond"`, `PONCY_FONT = "$Font5"`,
  `DEFAULT_IMG_FADE_DURATION = 0,25` (fade de **imagem**).

**Alerta do celular**: `CREATE_ALERT(iconID, x, y, titleString?)` anexa
`alertDialogBox`; IDs observados nos scripts: 26 (SLEEP_MODE),
52 (INVITES_PENDING), 53 (ON_CALL), 55 e 60 (sem rótulo na tabela
extraída), quase sempre em (222, 222) — coordenadas locais do telefone.
`CLEAR_ALL` recria o container. Sem confirmação/cancelamento nesse helper.

---

## 7. Scripts `.ysc` — estado real (o que dá para usar e o que falta)

### 7.1 O acervo bruto

`script_rel-EXTRAIDO/` tem **1.029 `.ysc`** (todos magic RSC7
`52534337`, versão 12) + o `script_rel.rpf` original remontado;
`07-scripts-ysc-EXCECAO-update/` tem a cópia separada do `ingamehud.ysc`
(243.234 B, vinda de `update.rpf`). **Proveniência a confirmar:** o RPF
pai do `script_rel-EXTRAIDO` não foi registrado no manifesto; como o
`ingamehud.ysc` de lá é **byte-idêntico** ao da exceção-update (SHA
`269550E4…`) e o `SCRIPT.rpf` base foi entregue fechado (não extraído),
é provável que o `script_rel` também venha de `update.rpf` — tratar como
"build atual do PC do usuário", não como "base pura", até confirmarmos.
Os `cellphone_controller.ysc`/`cellphone_flashhand.ysc` do lote 1 são
byte-idênticos aos daqui (mesma build).

### 7.2 Descriptografados: 22 `.ysc.full` ✔ validados

`YSC-full-script-rel-temp/` traz 22 scripts já convertidos para
`.ysc.full` (RSC7 inflado/descriptografado), cada um com
`*.decrypt-info.json`. **Nós re-hasheamos os 22**: todos conferem com
`DECRYPT-INDEX.csv`, e os 22 fontes conferem com os `.ysc` de
`script_rel-EXTRAIDO`. Header `.full` = mesmo RSC7 v12, com ponteiros
legíveis. Resumo (code_len/natives/strings do header):

| Script | Fonte (B) | `.full` (B) | code_len | natives | strings (B) |
|---|---:|---:|---:|---:|---:|
| achievement_controller | 33.277 | 81.936 | 75.908 | 85 | 1.541 |
| appcamera | 19.637 | 81.936 | 66.585 | 198 | 1.619 |
| appcontacts | 40.210 | 131.088 | 79.414 | 259 | 8.749 |
| appemail | 11.771 | 32.784 | 23.714 | 81 | 3.635 |
| appinternet | 1.127.567 | 2.818.064 | 2.646.559 | 1.246 | 133.880 |
| appmedia | 2.249 | 8.208 | 3.295 | 44 | 511 |
| appsettings | 4.210 | 16.400 | 8.751 | 56 | 354 |
| apptextmessage | 10.432 | 49.168 | 20.181 | 82 | 6.718 |
| apptrackify | 1.593 | 8.208 | 2.177 | 36 | 82 |
| cellphone_controller | 21.331 | 49.168 | 41.637 | 184 | 4.668 |
| cellphone_flashhand | 13.999 | 40.976 | 30.835 | 172 | 1.239 |
| completionpercentage_controller | 66.990 | 180.240 | 143.550 | 100 | 30.351 |
| floating_help_controller | 778 | 8.208 | 943 | 19 | 1 |
| hud_creator | 137 | 8.208 | 8 | 0 | 0 |
| ingamehud | 243.234 | 638.992 | 570.150 | 574 | 53.023 |
| mission_stat_alerter | 226 | 8.208 | 66 | 2 | 0 |
| pausemenu | 422 | 8.208 | 264 | 7 | 56 |
| pausemenu_example | 491 | 8.208 | 328 | 8 | 56 |
| pausemenu_map | 26.740 | 65.552 | 53.963 | 145 | 4.235 |
| pausemenu_multiplayer | 286.125 | 811.024 | 733.875 | 511 | 52.985 |
| pausemenu_sp_repeat | 12.421 | 32.784 | 22.289 | 46 | 6.146 |
| timershud | 23.817 | 81.936 | 66.012 | 83 | 1.029 |

### 7.3 Descompilados: só 3 (a rodada em massa falhou)

`ENTREGAS-GLUE5-YSC/descompilados/prioritarios/` contém **3 pseudocódigos
`.c`** (formato SC-CL do descompilador do zorg93, commit
`2958d816…`, CLI 1.0.0.0):

| Saída | Linhas* | Bytes | Origem do `.ysc` |
|---|---:|---:|---|
| `cellphone_controller.c` | 7.071 | 176.874 | lote 1 (= `script_rel`) |
| `cellphone_flashhand.c` | 6.148 | 153.958 | lote 1 (= `script_rel`) |
| `update-excecao/ingamehud.c` | 112.525 | 2.517.123 | exceção `update.rpf` |

\* Nossa contagem (`wc -l`, LF). O `INDICE.csv` da entrega registra
6.631/5.784/108.636 — diferença de contagem de linhas (CRLF/última linha);
os bytes e SHA-256 batem, então as citações "linha N" do relatório deles
podem estar deslocadas em relação a editores que contam diferente.

A tentativa de descompilar os 22 de §7.2 de uma vez **falhou inteira**:
`DECOMPILATION-INDEX.csv` marca os 22 como `NO_OUTPUT_EXIT_` e os 22 logs
têm **0 bytes** — não ficou registrada a causa (provavelmente invocação
errada do descompilador). Os 3 `.c` acima saíram de uma rodada anterior,
separada. **Trabalho futuro de maior valor:** rodar o descompilador nos
22 `.ysc.full` já prontos (não precisa descriptografar nada de novo).

Contagens de padrões no `INDICE.csv`/`analise/*.csv` (1.029 scripts
inventariados em `INVENTARIO-SCRIPT-REL.csv`; 591 linhas em
`OCORRENCIAS-UI.csv`; 259 em `OCORRENCIAS-INGAMEHUD.csv`) são **contagem
de linhas que casam um texto** (Select-String), não contagem semântica de
chamadas — úteis para navegar, não como métrica.

### 7.4 O que o `ingamehud.c` já prova (exceção update)

- Avisos nativos via `HUD::SET_WARNING_MESSAGE_WITH_HEADER` (visto por nós
  nas linhas ~104.492+), com inteiro de prompt `16384` ou `instructionalKey`
  — fluxo de **warning nativo**, não necessariamente o filme
  `popup_warning.gfx`.
- Texto desenhado à mão: helpers que montam
  `BEGIN_TEXT_COMMAND_DISPLAY_TEXT` e configuram fonte/escala/RGBA/wrap/
  efeito (1 = contorno+sombra, 2 = contorno, 3 = sombra). Um rótulo "V"
  usa font 0, escala 0,202×0,276, branco 255 — **é só aquele rótulo**.
- Feed de salvamento com esperas de 1.000 ms / 10.000 ms — fluxo
  específico, não tempos globais.
- Scripts prioritários presentes no `script_rel` além dos 22:
  `pausemenu*` (5 no total), `app*.ysc` (33), `main*` (4), `startup*` (5),
  `boot*` (2), `fm_*` (68), `shop*` (2) e `*menu*` (12) — lista completa
  no `INVENTARIO-COMPLETO.csv`.

**Lacunas conhecidas** (o relatório da entrega é honesto aqui e nós
concordamos): sem evidência suficiente ainda para fonte/escala de
legendas, feed genérico, nomes de rua/área, dinheiro, estrelas e
mensagens grandes ("Wasted"). Não preencher por aproximação — descompilar
os `.ysc.full` e exportar mais filmes primeiro.

---

## 8. Lotes anteriores — relação com o lote atual

### 8.1 Lote 1 (`assets31-10-26-12-05.zip`) mistura DUAS builds

Descoberta desta rodada, por comparação de SHA-256 membro a membro:

- Os arquivos com sufixo `(1)` são a **build base**: 56 dos 96 membros têm
  hash presente no lote atual (todos os `(1)`, os 9 `.ysc`, `frontend.xml`
  e `pausemenu.xml` sem sufixo, `pausemenueffects.meta` etc.). Ex.:
  `pause_menu_pages_settings(1).gfx` (60.053 B) = o atual de
  `01-scaleform_frontend`; `scaleform_frontend(1).rpf` (3.047.424 B) = o
  atual.
- Os 40 membros **sem par** são a **build de update** (quase certamente
  `update.rpf`): versões maiores dos mesmos filmes — `hud.gfx` 238.772 B
  (base: 183.760), `minimap.gfx` 207.370 (base: 136.647),
  `scaleform_frontend.rpf` 13.664.768 B (base: 3.047.424) — mais itens
  exclusivos: `frontend.awc` (6.060.128 B, também está no ZIP de áudio),
  `frontend(1).xml`/`pausemenu(1).xml` (XMLs da build update),
  `pause_menu_pages_keymap.gfx/.ytd` (ausentes no lote base!),
  `mpgogospacefrontend.ytd`, `hud.ytd` 469.536 B.

**Uso:** o lote 1 é a nossa referência da build *update* (a que o jogador
efetivamente roda hoje, pois o `update.rpf` sobrescreve a base), e o lote
de 2026-11-01 é a referência *base*. Ao medir algo destinado a hooks no
jogo real, prefira a variante update quando as duas existirem; ao estudar
estrutura genérica, a base serve. (No documento de 2026-10-31 nós já
suspeitávamos de "duas fontes" — agora está provado e mapeado.)

### 8.2 `cellphone.zip` é subconjunto do lote atual

Os 6 `.gfx` são byte-idênticos aos de `02-scaleform_generic`. Manter no
Drive como atalho, mas não rebaixar.

### 8.3 Áudio

- `GTA5-frontend-AWC-REL-para-Arena.zip`: `frontend.awc` + 17 `.rel`
  (catálogo em §1.2). É a fonte dos sons de frontend já mapeados em
  `docs/menu-sfx.md`; os `.rel` permitem localizar nomes/parâmetros de
  som sem reabrir o jogo.
- `pausemenu.m4a` (21min28s, gravação via FFmpeg) e `RESIDENT.rpf`
  (RPF7 de TOC criptografada) foram baixados e sondados por metadados em
  2026-11-01 (§1.2); análise de conteúdo deles é de baixo valor para a
  paridade visual de menus agora.

### 8.4 Pasta `Claude/` (código do GLUE, 2026-10-28)

Material do próprio projeto: snapshot da etapa da start page
(`glue5-main-start-page.zip`, 528 membros), o `.patch` correspondente e
`glue5-scenes.zip` (as cenas PNG da Home). Referência histórica; o código
vivo é o deste repositório.

---

## 9. Avaliação da entrega `ENTREGAS-GLUE5-YSC/` (ChatGPT/Desktop Commander)

Quem produziu: sessão assistida no PC do usuário (OpenIV + FFDec 26.4.0 +
descompilador YSC), a partir do nosso guia/prompt de 2026-10-31.
Conteúdo (287 arquivos, 4 MB): `MANIFESTO.md`, `RELATORIO-ESTILO-HUD.md`,
`ERROS.md`, `RESUMO-10-LINHAS.md`, `fontes.md`, `PROXIMA-ETAPA-OPENIV.md`,
`INDICE.csv`, `FONTES-INVENTARIO.csv`, `analise/` (3 CSVs + resumo),
`descompilados/prioritarios/` (3 `.c` + índices), `referencias-ui/` (4
arquivos de §4), `scaleform-ui-source/` (§6.2), logs do FFDec.

Confiança por parte:

- ✅ **Conferido por nós e correto**: aliases de fonte (§5.1), cores
  (§6.3), geometria do `SHOW_POPUP_WARNING` (inclusive o aviso de que
  `fadeSpeed`/ `msecs` não são usados), constantes dos botões
  instrucionais, alturas 27/25 px, `rowSpacing −2`/430/15, contrato do
  `CREATE_ALERT`, inventário das 50 fontes com hashes, inventário dos
  1.029 `.ysc`, os 3 `.c` (bytes/hashes batem).
- ⚠️ **Desatualizado**: afirmações de escopo do `MANIFESTO.md`/
  `PROXIMA-ETAPA-OPENIV.md` (§2) — foram escritas antes da extração/
  empacotamento final. Ignorar as frases "não entra nos pacotes",
  "07 vazio", "extração não feita".
- ⚠️ **Não verificado / cautela**: citações de número de linha dentro dos
  `.c` (contagem difere — §7.3); contagens de padrões = linhas com match
  (§7.3); tempos de feed são de um fluxo de salvamento específico;
  classes de filmes que importam SWF externo podem estar incompletas no
  ActionScript exportado (8 avisos do FFDec).
- ❌ **Falhou**: a descompilação em massa dos 22 `.ysc.full`
  (`NO_OUTPUT_EXIT_`, logs vazios). A descriptografia, sim, funcionou.

Ferramentas declaradas (para repetir no PC): FFDec **26.4.0**;
descompilador YSC local commit
`2958d816e8f6db02a399c7b570c7074a0b2b1567` (CLI 1.0.0.0); OpenIV para a
extração. Caminhos no PC aparecem nos relatórios como
`…\GLUE5-EXTRACAO\assets-base-2026-10-31` (extração) e
`…\GTA5\Ferramentas\…` (ferramentas/entregas).

---

## 10. Como reutilizar este acervo (receita)

1. Baixar do Drive os 24 arquivos de `glue5/PACOTES ARENA/01-11 02:30`
   (IDs em §1.1) para **fora** do repositório.
2. Conferir o SHA-256 de cada ZIP contra `docs/gta5-referencia/
   INDICE-PACOTES.csv`; extrair tudo para a mesma raiz.
3. Remontar os 3 grandes: concatenar as partes `_SPLIT/…PARTE-NN…` em
   ordem crescente de `PartNumber` (manifesto em
   `MANIFESTO-ARQUIVOS-DIVIDIDOS.csv`) e validar o tamanho final
   (§2, item 4). Apagar `_SPLIT/` depois.
4. Conferir arquivos individuais contra `INVENTARIO-COMPLETO.csv` quando
   a procedência importar (hash = identidade).
5. Para estudar um filme ainda sem ActionScript: FFDec 26.4.0, exportar
   scripts (e fontes se for `font_lib_*`); lembrar do aviso de imports
   externos.
6. Para scripts: os 22 `.ysc.full` já estão prontos para o descompilador;
   novos scripts pedem o passo de descriptografia antes.

Nesta sessão o material ficou em `…/gdrive-ref/pacotes-2026-10-31/`
(`zips/`, `extraido/`, scripts `analisa.py`/`inventario.py`/`fontparse.py`
e os CSVs gerados) e os lotes antigos em `…/gdrive-ref/lotes-antigos/` —
mas lembre: **isso não persiste**; o que persiste é este documento + os
CSVs em `docs/gta5-referencia/` + o Drive.

## 11. Próximos passos sugeridos (em ordem de valor)

1. **Descompilar os 22 `.ysc.full`** — desbloqueia pausemenu(s), timers e
   os apps do celular de verdade. Investigar por que a rodada em massa
   saiu sem output (logs vazios) antes de repetir.
2. **Confirmar a proveniência do `script_rel`** (abrir o `SCRIPT.rpf`
   base entregue e comparar 2–3 scripts por hash): decide se tratamos o
   acervo de scripts como base ou como update.
3. **Exportar com FFDec os filmes prioritários que faltam** (os demais
   `pause_menu_pages_*`, `hud`, `instructional_buttons`,
   `custom_warning_screen`, celulares), nas duas builds quando houver
   (base de `02-…`/`01-…`; update do lote 1).
4. **Decidir a estratégia de fonte do GLUE** com os fatos de §5 (medidas e
   cobertura já permitem casar métricas com fonte livre; uso em runtime a
   partir dos arquivos do jogador continua sendo a única via para a face
   original — nunca embarcar).
5. Aplicar os deltas medidos aqui no shell (ver atualização de 2026-11-01
   em `gta5-visual-parity.md`): geometria do pop-up, RGB 240, fundo
   0/0/0/186, alturas 27 px com realce −2, rodapé 12 pt @720p.
6. Pendência antiga: **compilar e testar no aparelho** o código do pop-up
   (`ui/popup.rs`) — o sandbox segue sem toolchain Rust.

---

## 12. Completude do mapeamento (fechamento de 2026-11-01)

**Download:** a pasta `glue5/` do Drive foi baixada **por inteiro** pelo
menos uma vez nesta data (24 arquivos do `PACOTES ARENA`, 2 ZIPs de
`assets/`, 3 itens de `Claude/`, `chalet.woff`,
`GTA5-frontend-AWC-REL-para-Arena.zip`, `pausemenu.m4a`) + o `RESIDENT.rpf`
da raiz. Conferência final às 12h38–12h45 (horário local): **nada novo** no
Drive desde o lote das 02:30 (05:30Z).

**O que permanece sem análise de conteúdo — e por quê:**

| Item | Estado | O que falta / onde destrava |
|---|---|---|
| 22 `.ysc.full` | Descriptografados e validados, **não descompilados** | Rodar o descompilador (PC do usuário) — §7.3/§11.1 |
| `SCRIPT.rpf` (base, no lote) | Remontado, **não aberto** | OpenIV no PC; resolve a proveniência do `script_rel` (§11.2) |
| `RESIDENT.rpf` | Header sondado; TOC criptografada | OpenIV/CodeWalker no PC (só se o áudio resident virar pauta) |
| 459 `.gfx` sem ActionScript | Inventariados (nome/bytes/hash); só 8 filmes têm `.as` | Exportar com FFDec sob demanda, por tela (§11.3) |
| `pausemenu.m4a` | Metadados lidos; áudio **não ouvido** | Ouvir é trabalho humano (referência de mixagem/SFX do pause) |
| `Download.7z` (raiz do Drive, 345 MB) | **Não é do projeto** (confirmado pelo usuário); escopo do projeto = pasta `glue5/` | Ignorar |
| Restante do Drive | Fora de escopo (arquivos pessoais/outros jogos) | — |
| Código `ui/popup.rs` | Escrito, **não compilado** neste ambiente | `cargo test`/build + teste no aparelho (§11.6) |

**Estado local no fechamento:** como o sandbox apaga tudo fora do
repositório entre turnos, o material bruto **não está em disco agora**
(exceto `pausemenu.m4a`/`RESIDENT.rpf` recém-baixados em área temporária).
O conhecimento extraído está todo neste documento, em
`gta5-visual-parity.md` e nos CSVs de `docs/gta5-referencia/`; o material
bruto volta em minutos pela receita do §10 quando a próxima etapa pedir.
