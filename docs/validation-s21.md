# Validação no aparelho — Galaxy S21+ (SM-G996B)

O que o build tem que provar, em ordem. Cada passo é um build ou uma run do
`mise`; nada aqui foi rodado no sandbox (sem toolchain Rust, sem SDK Android).

Gate atual: **esta build**. As duas branches (a minha, `arena/b8d177aa-glue5`, e a
do chat de UI/menus, `arena/4f5cc306-glue5`) só se fundem depois que o aparelho
disser que o código está bom.

---

## 0. Antes de tudo

```bash
mise run setup          # toolchain, NDK, SDK (compila o wasm2c patched)
mise run android:build  # o APK (e confere o alinhamento de 16 KB das libs)
```

O `android:build` já instala o que falta no SDK. Se o `GLUE5_GAME_DIR` não
estiver apontado para o `game.wasm` da sua instalação, o build do jogo falha
logo no começo — é o primeiro erro que aparece se algo estiver errado.

**Se o build falhar:** mande a saída do `mise run android:build` inteira. O
erro mais provável é de compilação (o código nunca foi compilado por mim) e
cada mensagem aponta o arquivo e a linha.

---

## 1. A build compila e o jogo abre

O que já está no `main` (o pop-up, os hints, a fonte) **e** o que eu
adicionei precisam coexistir. O `app.rs` foi mexido pelos dois chats — esse
é o ponto do merge.

```bash
mise run android:run
```

**Olhe:** o jogo abre, o menu funciona, o pop-up de "Restore graphics" aparece
(o `[MENU]`), e o start page não quebrou.

---

## 2. O back-off térmico (o meu `[PERF]`)

O **Thermal back-off** está ligado por padrão no Android (Settings › Video).
Ele só age quando o telefone esquenta, então há duas formas de ver:

**2a. O caminho fácil — forçar o calor.** Jogue 10–15 minutos. O telefone
esfria devagar; o log mostra a curva:

```
RUST_LOG=info mise run android:run
```

Procure as linhas `thermal:` no logcat. Cada uma traz o nível, o estado do
sistema, o headroom e o cap:

```
thermal: level 1 moderate (headroom 0.83) cap 40
thermal: level 2 severe (headroom 0.91) cap 30
```

**2b. O caminho direto — o log sem esperar o calor.** Com o telefone frio, o
nível fica 0 e o cap 0 (nenhum). Para ver o *caminho* funcionando sem esperar,
desligue o **Thermal back-off** e ligue de novo nas Settings — o `thermal:`
aparece com o nível 0, o que confirma que a leitura está viva.

**O que olho:**
- As linhas `thermal:` aparecem (a leitura da Thermal API funciona).
- O nível sobe quando o telefone esquenta e **desce** quando esfria (a
  histerese: 45 s para cair um nível).
- Com o cap em 40, o overlay mostra a linha **thermal** e os quadros param em
  40 (não em 60).
- O **desempenho do hint**: com o cap, o sistema não sobe os clocks atrás de
  uma taxa que o host não segura.

**Os limiares são um palpite** (0,80 / 0,90 / 0,97 do headroom). Se no seu
telefone o nível subir cedo demais ou tarde demais, o ajuste é nos números de
`crates/glue5-qos/src/thermal.rs` (`UP`/`DOWN`) — me mande as linhas `thermal:`
do log e eu recalculo.

---

## 3. O preset de gráficos (o meu `[PERF]`)

Settings › Video › **Graphics preset** › *Phone*.

O preset escreve as settings do próprio GTA V (texturas menores, sem reflexos,
sem sombras suaves, sem grama, sem filtragem anisotrópica) **na próxima
inicialização**. Para ver o efeito:

1. Ligue o *Phone* e **feche o jogo** (o preset é escrito no start).
2. Abra de novo e jogue.

**O que olho:** a diferença de frame time no overlay (o GPU time cai — é o
que corta os bytes que a Mali-G78 lê da memória do aparelho). E que o menu de
gráficos do GTA V continua autoritativo depois: mudar lá tem efeito.

**Cuidado:** o preset é gravado **uma vez**, no start. Depois disso o menu do
jogo manda.

---

## 4. O PGO no aparelho (o meu `[PERF]`, o passo mais longo)

Este é o round completo: compila o jogo instrumentado, roda o benchmark no
aparelho, puxa o profile e recompila com ele.

```bash
mise run pgo:android
```

O que a task faz, e o que pode travar:

| Passo | Se travar |
|---|---|
| compila o jogo instrumentado (`GLUE5_GAME_PGO=generate`) | o erro de compilação é o do item 0 |
| instala o APK e roda o benchmark (`GLUE5_BENCHMARK=1`) | o app precisa fechar sozinho ao fim do benchmark — é o `benchmark_only` |
| puxa o `.profraw` da pasta do app | o aparelho precisa permitir (Android 11+ pede autorização de "arquivos de outro app" em `/sdcard/Android/data/`) |
| funde com o `llvm-profdata` do NDK | o NDK tem que estar no `ANDROID_NDK_HOME` |

Sai no fim:

```
profile: target/glue5-tools/pgo/android/game.profdata (1234 functions run)
build the game with it: GLUE5_GAME_PGO=.../game.profdata mise run android:build
```

**Para usar:**

```bash
GLUE5_GAME_PGO=$PWD/target/glue5-tools/pgo/android/game.profdata mise run android:build
```

O profile muda o desempenho de verdade? Meça com o benchmark do jogo (start
page › Benchmark) antes e depois, **com o mesmo preset e a mesma resolução** —
o thermal back-off tem que estar desligado durante a medição, senão ele
segura o jogo a 40 fps e a comparação não vale.

---

## 5. O objetivo: 30 fps (ou 40 no painel de 120 Hz)

A ordem que eu sugiro:

1. **Graphics preset › Phone** + **Frame rate › 30** (ou 40, se o painel
   aguentar e o jogo não ficar lento demais).
2. Meça com o benchmark.
3. Se ainda não chegar, baixe a **escala de render** (Settings › Video ›
   resolução) — mas saiba que hoje o HUD sai na mesma escala (é o que a
   GLU-25 vai resolver).
4. Se a resolução não for o problema, meça o **split dos passes** no log do
   GPU (as linhas `op N/frame`) — é onde eu quero saber se o custo está nas
   cópias, nos clears, na geração de mips ou nos dispatches.

**Alvo realista:** 30 fps travados. O painel é de 120 Hz, e o passo par
abaixo de 60 é 30 — 40 fps num painel de 120 dá 3 refrescos por quadro, o que
custa menos que 30 (2 refrescos) e dá margem térmica.

---

## 6. O que eu preciso que você me mande

1. **A saída do build**, se ele falhar (ou um print do logcat).
2. **As linhas `thermal:`** de uma sessãolonga, com o telefone esquentando.
3. **O benchmark** (antes e depois do preset): fps médio, mínimo, e a
   porcentagem de quadros sob 33 ms.
4. Se a **GLU-25** (separar a UI do 3D) entrar antes disso, me avise — o
   `CapAndScale` passa a mexer só na escala do 3D, e o alvo de 30 fps fica
   sem borrar o HUD.

---

## O que **não** foi validado (e por quê)

- **Nenhuma compilação.** O sandbox não tem toolchain Rust nem SDK Android, e
  `static.rust-lang.org`/`crates.io` estão bloqueados. Todo o código Rust
  aqui foi conferido por inspeção (campos de `Status` nos dois literais,
  chaves do `settings.xml` do motor uma a uma, sintaxe balanceada), **não**
  por `cargo check`.
- **Nenhum teste unitário rodado.** Os 8 testes do governor térmico
  (`crates/glue5-qos/src/thermal.rs`) e o teste do preset existem mas nunca
  executaram.
- **Nenhuma medição no aparelho.** Nenhum número de fps aqui é medido — os
  limiares e os passos de frame rate são hipóteses, e é por isso que o passo 2
  existe.
