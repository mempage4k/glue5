# Revisão de código para Android — GLUE 5 no Galaxy S21+ 5G (Exynos 2100, Mali-G78)

Revisão feita em **2026-10-28** sobre o commit `708c392` (`fix(ui): correct Home scene build errors`).

**Alvo declarado:** Galaxy S21+ 5G (SM-G996B, Exynos 2100, Mali-G78 MP14), Android 15 (One UI 7).
**Direção do projeto:** repositório **só Android**, com o objetivo de produto "GTA V para Android" —
a resposta para "como a Rockstar portaria GTA V para Android?".

> **Como ler.** Cada achado tem **evidência** (arquivo:linha), o **impacto no aparelho alvo**, a
> **correção sugerida** e as **referências**. A numeração `[n]` remete à lista em
> [Referências](#referências). O que não pude verificar está marcado como **(verificar no aparelho)**.

> **Limite desta revisão.** Este ambiente não tem `cargo`, `rustc`, `gradle`, `adb` nem o jogo, e
> `third_party/` (wgpu/wgpu-core/wgpu-hal/sp-fidelity) não está preparado — **nada foi compilado nem
> executado aqui**. As conclusões vêm de leitura estática do código, da documentação oficial das
> plataformas e do conhecimento das specs do aparelho. Toda correção precisa ser validada no S21+
> com os comandos da seção [Verificação no aparelho](#verificação-no-aparelho).

---

## 1. O alvo, e o que ele impõe ao código

| Item | Valor | Consequência para o projeto |
|---|---|---|
| SoC | Exynos 2100 (5 nm): 1× Cortex-X1 2.9 GHz + 3× Cortex-A78 2.8 GHz + 4× Cortex-A55 2.2 GHz [20] | 4 dos 8 núcleos são "little": trabalho paralelo (decode de BC, encode de ETC2, threads do wasm) não escala com `cores()` |
| GPU | Mali-G78 MP14 @854 MHz, Valhall 2ª geração; **sem BC**, **com ASTC LDR/HDR**, AFBC 1.3 [12][13] | Toda textura BC do build web passa por decode na CPU; ETC2 é o piso, ASTC é a saída |
| Vulkan | 1.1/1.2 no silício; o driver da Samsung na prática expõe **1.1** [12] | Nada de `sync2`, `load_store_op_none` (1.3) — o renderer tem de viver bem em 1.1 |
| RAM | **8 GB** (SM-G996B, versão internacional) [20][23] | O piso real de memória do jogo (~4 GB de wasm + alvos + texturas) fica apertado; ver A2 |
| Tela | 6.7", 1080×2400 (20:9), Dynamic AMOLED 2X, LFD 48–120 Hz, 394 ppi, câmera *punch-hole* central [20][21] | Insets de cutout em paisagem, `density` alta, taxa variável |
| SO máximo | **Android 15 / One UI 7** — suporte encerrado em fev/2026 [21][22] | O aparelho nunca verá Android 16/17: as mudanças de comportamento de `targetSdk 36` **não** valem nele; as de **API 35 (edge-to-edge)** valem |
| Termal | Exynos 2100 esquenta e a Samsung aplica GOS/políticas de jogo | Sem *governor* térmico no app, a experiência degrada sozinha em ~10 min |

Isso muda prioridades: `[Android 16 behavior changes][1]` importa para o **futuro** e para outros
aparelhos, mas o que o S21+ sente hoje é (a) edge-to-edge obrigatório desde API 35, (b) 8 GB de RAM,
(c) GPU tiler de 14 núcleos sem BC, (d) tela 120 Hz variável, (e) calor.

---

## 2. Resumo dos achados

| # | Prioridade | Achado | Onde |
|---|---|---|---|
| A1 | **P0** | Padrão de settings pede `MetalFx` (upscaler só de Apple) em Android, e o renderer age como "temporal" mesmo quando o upscaler não existe | `crates/glue5/src/settings.rs:259`, `crates/glue5-gpu/src/present.rs:976` |
| A2 | **P0** | Perfil `low_memory` não liga num aparelho de **8 GB**: limiar de 7000 MB contra `MemTotal` de ~7.2–7.6 GB | `crates/glue5/src/settings.rs:240,253` |
| A3 | **P0** | Instrução ao jogador Android ("copie para `Android/data/…`") é impossível de seguir sem adb desde o Android 11 | `crates/glue5/src/paths.rs:202-208` |
| A4 | **P0** | Descoberta/GTA Online na LAN **não recebe** multicast/broadcast sem `MulticastLock` — falta permissão e lock | `platform/android/AndroidManifest.xml:19`, `crates/glue5-runtime/src/p2p.rs:349`, `crates/glue5-stream/src/discovery.rs:390-396` |
| A5 | **P1** | O `minSdk 29` original não oferecia o acesso a arquivos usado por "Find files" (API 30+); resolvido com `minSdk 30` | `platform/android/gradle.properties:10`, `AndroidManifest.xml:31-33` |
| A6 | **P1** | Nenhum `StoreOp::Discard` no renderer: todo alvo é escrito de volta à memória em toda passagem | `gpu/src/scene.rs:322,327`, `exec.rs:1205,1389`, `ops.rs:426,441,683,750`, `present.rs:686`, `draw.rs:309` |
| A7 | **P1** | Haptics só no iOS, embora `VIBRATE` esteja declarada: toque e rumble mudos no Android | `crates/glue5/src/haptics.rs:5` |
| A8 | **P1** | Botão Voltar do Android fecha o app em vez de abrir o menu de pausa (SDL não está em `TRAP_BACK_BUTTON`, e `AC_BACK` não está no keymap) | `crates/glue5/src/app.rs:57-68`, `keymap.rs:12-29` |
| A9 | **P1** | Sem estado térmico no Android (só iOS); nenhum *governor* de qualidade/fps | `crates/glue5/src/app.rs:1244` (`#[cfg(target_os = "ios")]`) |
| A10 | **P1** | Pedido de taxa de quadros com `compatibility = DEFAULT` (0), sem `FIXED_SOURCE`/estratégia de troca | `crates/glue5-runtime/src/qos.rs:70` |
| A11 | **P1** | ASTC nunca é pedido ao wgpu, e não há caminho para pré-cozinhar texturas ASTC (o formato "natural" de Mali) | `crates/glue5-gpu/src/lib.rs:161-180` |
| A12 | **P2** | `targetSdk` e `versionCode` agora são independentes: `targetSdk=36`, `VERSION_CODE=2`, nome usa SemVer + codename | `platform/android/build.gradle.kts:37-42`, `VERSION_CODE` |
| A13 | **P2** | 16 KB de página: verificação de `p_align` agora está no APK build e na CI; execução bem-sucedida ainda pendente | `platform/android/build.gradle.kts`, `.github/workflows/import-tests.yml` |
| A14 | **P2** | `density` fora de `configChanges`: mudar zoom de tela recria a Activity (mata o jogo) | `AndroidManifest.xml:74-80` |
| A15 | **P2** | Testes do toque só com telas de iPhone; nenhuma fixture Android 20:9 com cutout | `crates/glue5/src/touch/tests.rs:1-30` |
| A16 | **P2** | Cache de pipeline indexado só por *vendor/device*: atualização de driver reaproveita cache velho | `crates/glue5-gpu/src/lib.rs:182` |
| A17 | **P2** | Sem hint de áudio de baixa latência do Android; `Initiated`/`Utility` não viram *nice* no Android | `crates/glue5/src/audio.rs:34`, `crates/glue5-runtime/src/qos.rs:36-45` |
| A18 | **P2** | Comentário diz "ADPF Android 13+"; `APerformanceHint_getManager` existe desde a API 31 | `crates/glue5-runtime/src/qos.rs:80` |
| A19 | **Dívida** | 113 pontos de `cfg(target_os/target_vendor)` e ~7 k linhas de caminhos não-Android para remover | ver §5 |

> **Já aplicado neste branch:** A1 (padrão por plataforma, *clamp* depois de detectar a GPU,
> atualização da `Presentation` antes do primeiro quadro e guarda `is_temporal() && has()`), A2
> (perfis Auto/Low/Normal, migração do booleano antigo e limiar Auto de 8500 MB), A3 (Find files
> recomendado e `adb push` como única alternativa manual; sem instrução para copiar pelo app Files),
> A7 (haptics Android em `haptics.rs`/`Glue5Activity.java`, ainda sem validação no aparelho), A8
> (hint `SDL_ANDROID_TRAP_BACK_BUTTON` e mapeamento de `AC_BACK` para Esc; o item 3 desta lista
> ainda corrige a ocultação dos controles), A12 (`targetSdk` separado e `VERSION_CODE`) e A14
> (`density` em `configChanges`). A2 ainda precisa da medição de `MemTotal` no S21+. A4 agora tem
> as permissões e o lock restrito à busca, à página Servers e ao Online em execução, com liberação
> no ciclo de vida da Activity; falta validar a descoberta no aparelho. A5 agora exige `minSdk 30`;
> A13 tem verificação de `p_align` no APK/CI, mas ainda não teve execução bem-sucedida neste branch.
>
> **Backlog complementar:** [android-performance-s21.md](android-performance-s21.md) registra hipóteses
> de desempenho específicas do S21+ e os critérios de medição; esta revisão cobre os achados de
> comportamento/API e seu status. Os documentos se complementam e fazem referência um ao outro.

---

## 3. Achados detalhados

### A1 — P0 — O upscaler padrão é de Apple, e o renderer "acha" que ele existe

**Evidência.** `Settings::default()` escolhe `upscaler: Upscaling::MetalFx` para **toda** plataforma
(`settings.rs:259`). Em Android, `present.rs:789-798` só liga `Bicubic` e `Fsr` (MetalFX nem é
instanciado fora de `target_vendor = "apple"`), e a UI monta a lista com
`UPSCALERS.iter().filter(|u| screen.has_upscaler(..))` (`app.rs:207`) — ou seja, o menu abre com um
valor que **não está na lista**. Pior: `upscale()` aplica `temporal_size()` sempre que
`upscaler().is_temporal()` (`present.rs:976`), mas `wants_scene()` exige `is_temporal() && has()`
(`present.rs:250-252`). Com `MetalFxTemporal` num aparelho sem MetalFX o renderer **reduz o quadro
como se fosse temporal e depois usa bicúbico** — imagem menor e borrada, trabalho jogado fora.

**Impacto no S21+.** O caminho padrão de um jogador novo é exatamente esse estado inconsistente; e,
mesmo no caso espacial, o ajuste salvo nunca corresponde ao que a GPU faz.

**Correção (já aplicada neste branch).**
1. Padrão por plataforma: `MetalFx` só em `target_vendor = "apple"`, `Bicubic` em Android.
2. Ao abrir a tela, "clampa" o ajuste salvo para o primeiro upscaler que a GPU tem
   (`app.rs`, logo depois da construção de `App`).
3. `upscale()` passa a exigir `has()` para o dimensionamento temporal (`present.rs:976`).

**Referências.** [2][9] (MetalFX é API de Metal), `crates/glue5-gpu/src/present.rs` (o próprio
`has()` documenta que só o que a GPU tem é oferecido).

---

### A2 — P0 — Num aparelho de 8 GB, o perfil "low memory" provavelmente **não** liga

**Evidência.** `const LOW_MEMORY_RAM_MB: i32 = 7000;` e
`low_memory: MOBILE && sdl3::cpuinfo::system_ram() < LOW_MEMORY_RAM_MB`
(`settings.rs:240,253`). O SDL devolve o **`MemTotal`** de `/proc/meminfo`, que num aparelho "8 GB"
fica em torno de **7,2–7,6 GB** (kernel, modem, reservas de GPU/câmera). O S21+ internacional tem
8 GB [20][23], exatamente o caso de fronteira.

**Impacto no S21+.** O jogo pede ~4 GB (memória wasm + alvos + texturas decodificadas) e o aparelho
roda o perfil **normal** (pools maiores, cap 3840×2160 em `render_size`, `low_preset_written=false`).
Resultado esperado: `lmkd` mata o app em cenas densas, justamente no aparelho alvo.

**Correção aplicada neste branch.** `Settings` agora salva `memory_profile` (`Auto`, `Low` ou
`Normal`); `Auto` escolhe o perfil reduzido em aparelhos móveis quando `sdl3::cpuinfo::system_ram()`
— o `MemTotal`, não a RAM de marketing — é menor que 8500 MB. O jogador pode forçar qualquer um
dos três valores em Settings › Graphics. O booleano legado `low_memory` é lido apenas para migração,
ignorado para que instalações antigas passem a `Auto`, e não é serializado novamente; o estado
`low_preset_written` permanece intacto. O log de início registra RAM, perfil e decisão efetiva.

**Verificação pendente no S21+.** Confirmar `MemTotal` com `adb shell cat /proc/meminfo | head -1`
e validar que o limite escolhido corresponde ao comportamento esperado de memória; nenhum valor
foi medido neste ambiente.

**Referências.** [20][23] (specs), `settings.rs` (limiar e comentário).

---

### A3 — P0 — A instrução para colocar os arquivos no Android não funciona sem adb

**Evidência.** `where_the_game_goes()` (`paths.rs:202-208`) diz:

> "Choose the folder with Find files: it asks for all-files access, and reads the game where it is.
> Or copy it, named game, into Android/data/<app>/files/ on the device."

Desde o Android 11, `Android/data/` de outros apps é inacessível a gerenciadores de arquivos e ao
próprio seletor SAF [24] — e o mesmo arquivo (`paths.rs:228-231`) reconhece que "the picker never
writes into the live save or game tree". Ou seja: a segunda metade da frase só é verdadeira via
`adb push` (ou MTP em alguns aparelhos), não pelo caminho que um jogador comum tem.

**Impacto no S21+.** O jogador lê a instrução, tenta pelo app Arquivos, não consegue, e conclui que
"o app está quebrado". É o primeiro contato.

**Correção aplicada neste branch.** `paths.rs`, as mensagens de `Glue5Activity` e o guia de início
agora recomendam **Find files** para escolher a pasta que o jogo lê no lugar. A alternativa
avançada mostra o comando exato `adb push game /sdcard/Android/data/local.glue5/files/` para rodar
em um computador. Não se sugere copiar com o app Files/SAF para `Android/data`. O `minSdk` agora
é 30 (Android 11), então não existe mais um fluxo de compatibilidade com Android 10.

**Referências.** [24] (escopo de armazenamento no Android 11).

---

### A4 — P0 — GTA Online na mesma Wi-Fi: o Android filtra o tráfego que o app espera receber

**Evidência.**
- `p2p.rs:349`: `socket.set_broadcast(true)` e, mais abaixo no mesmo arquivo, envios para
  `255.255.255.255` e por endereço individual (`p2p.rs:411-431`).
- `discovery.rs:390-396`: o *responder* entra no grupo multicast (`join_multicast_v4(GROUP, ..)`)
  para ouvir consultas mDNS.
- O manifesto declara **somente** `INTERNET` (`AndroidManifest.xml:19`).

O comentário em `discovery.rs:8-11` afirma "no multicast reception, so no Android multicast lock" —
isso vale para **encontrar** um servidor (a consulta sai, a resposta volta por unicast), mas **não**
para (a) anunciar-se a partir do telefone e (b) receber broadcast do `p2p`. A documentação do
`WifiManager.MulticastLock` é literal: *"Normally the Wifi stack filters out packets not explicitly
addressed to this device"* [4]. Broadcast não é "addressed to this device". Sem o lock, a descoberta
"funciona às vezes" e falha com a tela apagada/power save — o pior tipo de bug.

**Impacto no S21+.** As duas funções de destaque do projeto (GTA Online sem servidor e "streaming da
cópia do PC") dependem disso; e falham silenciosamente.

**Correção aplicada neste branch.** O manifesto agora declara `ACCESS_NETWORK_STATE`,
`ACCESS_WIFI_STATE` e `CHANGE_WIFI_MULTICAST_STATE`. `Glue5Activity.lanDiscovery(boolean)` cria
uma única `WifiManager.MulticastLock("glue5")`, não referenciada por contagem, e adquire/libera de
forma idempotente. `App::update` chama `lan::hold` apenas enquanto uma busca está ativa, a página
Servers está aberta ou um jogo em `Mode::Online` está rodando; os crates de rede não chamam JNI.
A Activity libera o lock em `onPause`/`onDestroy` e restaura uma solicitação ainda válida em
`onResume`. O comentário de `discovery.rs` agora distingue as respostas mDNS unicast do tráfego
multicast/broadcast recebido. A validação de descoberta com dois aparelhos e tela apagada continua
pendente no S21+.

**Referências.** [4] (MulticastLock), [5] (local network permission, para o futuro).

---

### A5 — P1 — `minSdk 29` vs. `MANAGE_EXTERNAL_STORAGE` (API 30)

**Evidência original.** `gradle.properties` declarou `glue5.minSdk=29` (API 29, Android 10),
enquanto `AndroidManifest.xml:31-33` declara `MANAGE_EXTERNAL_STORAGE`, disponível a partir da
API 30. A permissão e a tela `ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION` só existem a partir
da API 30 [10].

**Impacto.** Num Android 10 (API 29) — que o app declara suportar — "Find files" não tem como obter
acesso a arquivos; sobra o caminho de cópia (que, por A3, não é acessível).

**Correção aplicada neste branch.** `platform/android/gradle.properties` agora fixa
`glue5.minSdk=30` (Android 11), a primeira API que oferece `MANAGE_EXTERNAL_STORAGE`. O fluxo em
`Glue5Activity` usa diretamente o acesso a todos os arquivos e não apresenta mais a mensagem de
fallback para Android 10; o caminho avançado por `adb push` continua documentado para importar a
pasta pelo computador.

**Referências.** [10].

---

### A6 — P1 — Nunca se usa `StoreOp::Discard`: em GPU tiler isso é banda (calor e bateria) de graça

**Evidência.** `grep -c "StoreOp::Discard"` no crate `glue5-gpu` = **0**. Todos os alvos são
`StoreOp::Store`, inclusive profundidade que ninguém lê depois:
`scene.rs:327` (o passe de UI/cena por quadro), `exec.rs:1205,1389`, `ops.rs:426,441,683,750`,
`present.rs:686`, `draw.rs:309`.

**Impacto no S21+.** Mali é *tile-based*: `STORE_OP_STORE` num alvo que ninguém lê obriga a GPU a
escrever a tela inteira na memória em cada passagem [6][7]. A documentação da Arm mede ~600 MiB/s de
leitura evitável só por trocar `LOAD_OP_LOAD`→`CLEAR`; o *store* é o mesmo raciocínio ao contrário
[6][8]. Em 1080p com várias passagens (G-buffer do deferred, SSAO, coronas, pós) isso é banda que
vira watts e graus.

**Correção sugerida.**
1. Mapear no tradutor D3D11→wgpu: *depth-stencil* cujo conteúdo o motor não lê depois da passagem →
   `StoreOp::Discard` (equivale a `DONT_CARE`, que existe em Vulkan 1.1; `STORE_OP_NONE` só na 1.3,
   então `Discard` é o alvo certo aqui [11]).
2. `scene.rs:327` e os alvos de UI: `Discard` incondicional na profundidade.
4. Onde o motor reusa o alvo no quadro seguinte (history buffers), manter `Store` — a decisão tem de
   vir do uso declarado, não do hábito.

**Referências.** [6] Arm GPU Best Practices (load/store), [7] Khronos TBR best practices, [8] Arm
"Picking the most efficient load/store operations", [11] `VK_KHR_load_store_op_none` (1.3, indisponível).

---

### A7 — P1 — Haptics: permissão declarada, implementação ausente

**Evidência.** `haptics.rs:5`: `pub const SUPPORTED: bool = cfg!(target_os = "ios");`, com
`imp` vazio fora do iOS. O manifesto pede `VIBRATE` (`AndroidManifest.xml:16`).

**Impacto no S21+.** O S21+ tem motor linear de eixo X — dos melhores do mercado. Sem implementação,
(a) o toque nos controles não responde fisicamente, (b) o *rumble* do jogo (que o motor já calcula e
entrega a cada quadro) é descartado, (c) a permissão fica sem uso (cheiro de código morto). Isso é
uma das maiores diferenças percebidas entre "emulador" e "port".

**Correção sugerida.**
1. `haptics::SUPPORTED = cfg!(any(target_os = "ios", target_os = "android"))`.
2. Android: `VibratorManager` (API 31+) com fallback para `Vibrator`; toques com
   `VibrationEffect.createPredefined(EFFECT_TICK)` (ou `performHapticFeedback`), *rumble* com
   `createOneShot`/`createWaveform` modulando amplitude pela força dos dois motores
   (`rumble((low, high))` já dá a intensidade pronta).
3. Respeitar `Settings > Sound > Touch feedback` do sistema (`Settings.System.HAPTIC_FEEDBACK_ENABLED`)
   e o ajuste do app (`haptics`).
4. Rate-limit: um `OneShot` por quadro no máximo; o S21+ corta acima de ~20 pedidos/s.

**Referências.** [12] (a doc de `VibrationEffect`/`VibratorManager` está em developer.android.com —
validar a versão exata da API no aparelho).

---

### A8 — P1 — O gesto/botão Voltar fecha o jogo em vez de pausar

**Evidência.** `AndroidManifest.xml:41`: `android:enableOnBackInvokedCallback="false"` — decisão
**correta** para um jogo (é a saída de emergência que mantém `KEYCODE_BACK` sendo despachado no
Android 16 [1]); mas `app.rs:57-68` não liga `SDL_HINT_ANDROID_TRAP_BACK_BUTTON`, cujo padrão é `0`
("não intercepta; o sistema finaliza a Activity") [3], e `keymap.rs:12-29` não mapeia
`Scancode::AC_BACK` (o código que o SDL 3 envia para o Voltar do Android) para `Esc` (0x1B), que é o
que o app entende como "pausa" (`app.rs:565`).

**Impacto no S21+.** Um toque acidental no gesto de voltar **encerra o jogo** no meio de uma missão.
No Android 15 do S21+ isso não é mudança de plataforma: é só falta de fiação.

**Correção (aplicada neste branch).** `app.rs` liga `SDL_ANDROID_TRAP_BACK_BUTTON=1` e `keymap::vk`
mapeia o scancode 270 (`SDL_SCANCODE_AC_BACK`, pelo valor — o nome do variante no crate `sdl3` é
detalhe) para `Esc`, que no app volta uma página (`ui.back()`) e no jogo abre o menu de pausa.
`enableOnBackInvokedCallback="false"` fica, agora com o comentário explicando que é a ponte até
migrar para `OnBackInvokedCallback`, como a própria Google descreve [1]. O SDL ainda está mudando
esse caminho para o API 36 (issue #13794) — acompanhar.

**Referências.** [1] Android 16 behavior changes, [3] SDL3 hints (`SDL_HINT_ANDROID_TRAP_BACK_BUTTON`,
`SDL_HINT_ANDROID_BLOCK_ON_PAUSE`).

---

### A9 — P1 — Sem termal no Android: o jogo derrete sozinho

**Evidência.** `app.rs:1244` — `watch_thermal()` é `#[cfg(target_os = "ios")]` e usa
`NSProcessInfo.thermalState`. No Android, nada observa a temperatura; `docs/playing.md` (§Performance)
só diz "phones get hot".

**Impacto no S21+.** Exynos 2100 + GOS: em ~10 minutos o aparelho reduz clocks e o jogo cai de 60
para 30–40 fps com *stutter* irregular. Uma port de verdade reage antes do usuário perceber.

**Correção sugerida.**
1. Via JNI: `PowerManager.getCurrentThermalStatus()` + `addThermalStatusListener` (API 29+ [14]) ou
   `AThermal_getCurrentThermalStatus` no NDK (API 30+ [15]).
2. Escada de reação (o "dynamic resolution" de console):
   `LIGHT` → nada; `MODERATE` → `render_scale -= 0.05`; `SEVERE` → fps 60→40 e sombras do jogo um
   passo abaixo; `CRITICAL` → 30 fps + `low_memory` runtime; `EMERGENCY` → pausa e aviso.
3. Registrar no log e mostrar no overlay (o `perf.rs` já tem tudo pronto para exibir).
4. Pedir `Window.setSustainedPerformanceMode(true)` (ou o hint equivalente do SDL) para o SoC não
   tentar "turbo" e cair depois.

**Correção feita (2026-11-01, sem validação no aparelho).** O Thermal do NDK
(`AThermal_getCurrentThermalStatus`, API 30; `AThermal_getThermalHeadroom`,
API 31) é lido em tempo de execução em `crates/glue5-qos/src/thermal.rs`. Um
*governor* com histerese vira as leituras em níveis 0–3: sobe depois de 6 s de
calor pedido, desce um nível de cada vez a cada 45 s, e o veredito do sistema
(status moderate ou pior) é um piso que o nível alcança de uma vez. O nível
segura o jogo pelo hook do limitador (`crates/glue5-runtime/src/frame_pace.rs`):
40 fps no nível 1, 30 fps nos níveis 2 e 3 (em tela de 120 Hz). A opção
*Frame rate and resolution* baixa também a escala de renderização no último
nível. O alvo do *performance hint* acompanha o limite, para o sistema não
subir os clocks atrás de uma taxa que o host não segura. Configurável em
Settings › Video › *Thermal back-off*. Os limiares ainda são umapalpite:
as linhas `thermal:` do log e a linha *thermal* do overlay são o que falta
medir no S21+.

**Referências.** [14] Thermal API (PowerManager), [15] Thermal (NDK).

---

### A10 — P1 — Pedido de taxa de quadros: falta `FIXED_SOURCE`

**Evidência.** `qos.rs:59-77` chama `ANativeWindow_setFrameRate(native, fps, 0)`; `0` é
`ANATIVEWINDOW_FRAME_RATE_COMPATIBILITY_DEFAULT` [17].

**Impacto no S21+.** Desde o Android 15, jogos ficam em **60 Hz por padrão** e precisam *pedir*
explicitamente a taxa desejada [16]; com `DEFAULT` o sistema escolhe por heurística, e num painel LFD
48–120 isso pode significar 120 Hz entregando 60 fps (dobro de banda/energia para a mesma imagem) ou
decisões de *frame pacing* ruins. O app já reenvia o pedido ao voltar do background
(`app.rs:1232-1234` zera `asked_rate`) — bom.

**Correção sugerida.**
1. Para `max_fps > 0`: `ANativeWindow_setFrameRateWithChangeStrategy(native, fps,
   ANATIVEWINDOW_FRAME_RATE_COMPATIBILITY_FIXED_SOURCE, ANATIVEWINDOW_CHANGE_FRAME_RATE_ALWAYS)` [17]
   — "meu conteúdo é exatamente esta taxa".
2. Para `max_fps == 0` ("Display"): passar `0` (deixa o sistema decidir), como hoje.
3. Reavaliar sempre que `max_fps` mudar (já acontece) e no resume (já acontece).

**Referências.** [16] Optimize refresh rates (Android 15 default 60 Hz), [17] NDK native_window,
[18] Frame rate (estratégias de mudança).

---

### A11 — P1 — ASTC ignorado: o formato que o Mali desenha melhor

**Evidência.** `WANTED_FEATURES` (`lib.rs:161-180`) pede `TEXTURE_COMPRESSION_BC`,
`TEXTURE_COMPRESSION_ETC2`, mas **não** `TEXTURE_COMPRESSION_ASTC(_HDR)`. `format.rs:decoded()`
reencoda BC→`Etc2Rgba8` quando não há BC. `bc.rs` faz isso na CPU, multithread.

**Impacto no S21+.** O Mali-G78 tem ASTC LDR/HDR nativo, com decodificação eficiente e
qualidade por bit muito acima de ETC2 no mesmo 8 bpp [12][13][19]. O jogo hoje: (a) decodifica BC na
CPU no carregamento (tempo de "Lendo…" + picos de CPU nos A55), (b) guarda ETC2 de 4 bpp/8 bpp com
qualidade pior. Numa port de verdade o material já viria **pré-cozido** no formato da GPU.

**Correção sugerida, em três passos (do mais barato ao definitivo).**
1. **Declarar**: adicionar `TEXTURE_COMPRESSION_ASTC` a `WANTED_FEATURES` e expor em `caps` — base para
   os próximos passos e para o overlay dizer o que a GPU realmente tem.
2. **Cache de transcodificação**: gravar o resultado BC→ETC2 em disco (`cache_dir`), indexado por
   hash do texel + formato + GPU. Hoje o custo é pago a cada carregamento (streaming inclusive).
3. **Pré-cozinhar ASTC** num passo de build (mise task) sobre o `data/` do jogo, com fallback ETC2 —
   é o que fecha a conta de banda e de memória num aparelho de 8 GB. Requer um encoder ASTC
   (crate `astc-encoder`, bindings do encoder da Arm) rodando **fora** do aparelho: codificar ASTC em
   tempo real no celular é lento demais.

**Referências.** [12] Mali-G78 (ASTC LDR/HDR, AFBC 1.3), [13] ASTC User Guide ("ASTC é opcional no
Vulkan; verifique `textureCompressionASTC_LDR`"), [19] `VK_EXT_texture_compression_astc_3d`,
[9] `wgpu::Features`.

---

### A12 — P2 — `targetSdk` acoplado a `compileSdk`; `versionCode` fixo

**Evidência.** `build.gradle.kts:37`: `targetSdk = setting("compileSdk").toInt()`; `:38`:
`versionCode = 1`.

**Impacto.** Subir `compileSdk` para usar uma SDK nova muda silenciosamente o comportamento do app
(edge-to-edge, predictive back, permissões) — foi exatamente essa classe de mudança que quebrou
metade dos apps no API 36 [1]. E `versionCode = 1` impede qualquer ordenação de atualização fora do
adb (`adb install -r` ignora, mas Play/F-Droid/atualizador não).

**Correção (aplicada).** `glue5.targetSdk=36` separado em `gradle.properties`, lido por
`setting("targetSdk")`; e um `VERSION_CODE` ao lado de `VERSION_CODENAME`, no mesmo padrão que o
build já usa para o `versionName`.

**Referências.** [1].

---

### A13 — P2 — 16 KB de página: no caminho certo, mas sem prova

**Evidência.** `gradle.properties` usa AGP `9.4.1` e NDK `29.0.14206865`; o `.mise/tasks/android/lib.sh`
copia `libglue5.so`, `libSDL3.so` e `libc++_shared.so` para `jniLibs`.

**Situação.** NDK r28+ já alinha a **0x4000** por padrão e AGP ≥ 8.5.1 empacota corretamente
[25][26] — então provavelmente já está certo. Mas o requisito do Google Play (desde 01/11/2025, com
bloqueio de updates a partir de 02/2027) exige **prova** de que *todas* as `.so` (inclusive a do SDL,
compilada pelo CMake do crate, e a `libc++_shared.so`) têm `p_align ≥ 0x4000` [25][26].

**Correção aplicada neste branch.** O task Gradle `verifyArm64PageAlignment` depende de
`assembleRelease`, extrai todas as `.so` de `lib/arm64-v8a/` do APK e usa `llvm-readelf -lW` do NDK
para validar cada segmento `PT_LOAD`: qualquer `p_align < 0x4000` interrompe o build com o nome da
biblioteca e o valor. `.mise/tasks/android/build` chama esse task e a CI constrói o APK antes de
executar os testes.

**Validação pendente.** Gradle/Android SDK não estão instalados neste ambiente, então ainda não foi
possível executar o task nem confirmar o resultado nas bibliotecas empacotadas.

**Referências.** [25] Support 16 KB page sizes, [26] Android Developers Blog (16 KB).

---

### A14 — P2 — `density` fora de `configChanges`: mudar o zoom da tela mata o jogo

**Evidência.** `AndroidManifest.xml:74-80` lista `fontScale`, `screenLayout`, `screenSize`,
`smallestScreenSize`, `uiMode`, `orientation`, `keyboard*`, `navigation` — **não** `density`.

**Impacto no S21+.** "Configurações → Tela → Zoom da tela" (muito usado em 6.7") muda a densidade:
a Activity é recriada, o processo do jogo morre no meio da sessão. O mesmo vale para
"Tamanho da fonte" em combinações que não caem em `fontScale` (coberto) e para mudanças de resolução
de exibição.

**Correção.** Adicionar `density` a `configChanges` e já tratar o novo `pixels_per_point` no
`ui::pixels_per_point` (que hoje relê `window` a cada quadro — deve funcionar sem mudanças).

**Referências.** [27] (configChanges/density), `AndroidManifest.xml`.

---

### A15 — P2 — Os testes de toque não conhecem nenhuma tela Android

**Evidência.** `crates/glue5/src/touch/tests.rs:1-30` define só iPhone 13 Pro, iPhone 16 Pro, iPad Pro.
`touch.rs:47-53` dimensiona tudo por `PHONE_SCALE`/`PHONE_SIDE` (limiar de 500 pt no lado curto).

**Impacto no S21+.** Em paisagem, 1080×2400 a `density` 420 dá ~**914×411** unidades lógicas, com
*punch-hole* no meio da borda superior e barra de gestos de ~24 dp. É um caso diferente dos iPhones
em proporção (20:9 vs 19.5:9) e em insets assimétricos — e não há teste cobrindo isso. Como o
`touch_layout` já é o coração da experiência "GTA no celular", esse é o teste que mais falta.

**Correção.** Adicionar fixtures `(914, 411, insets)` e `(1080, 2400, density)` ao módulo de testes e
uma asserção de que nenhum botão cai fora da *safe area* nem sob o *punch-hole*.

**Referências.** [28] Display cutout, [27] `SDL_GetWindowSafeArea`.

---

### A16 — P2 — Cache de pipeline indexado sem a versão do driver

**Evidência.** `lib.rs:182`:
`format!("pipelines.{:?}.{:x}.{:x}.bin", backend, vendor, device)`.

**Impacto.** Uma atualização de driver da Samsung (mesmo vendor/device) pode reaproveitar um cache
gerado por outro driver. O Vulkan especifica que o app deve tratar o cache como "dica"; drivers
costumam **rejeitar o blob inteiro** se a versão não casar — o que joga fora o *pipeline cache* e
devolve o "primeiro Play leva minutos" (`docs/playing.md`).

**Correção.** Incluir `info.driver`/`info.driver_info` (hash curto) no nome do arquivo.

**Referências.** `crates/glue5-gpu/src/lib.rs` (`pipeline_cache_name`, `save_pipeline_cache`).

---

### A17 — P2 — Áudio e prioridades de thread sem ajuste Android

**Evidência.** `audio.rs:34` fixa `SDL_AUDIO_DEVICE_SAMPLE_FRAMES=256` (bom: 5,3 ms a 48 kHz), mas não
liga `SDL_HINT_ANDROID_LOW_LATENCY_AUDIO` [3]. `qos.rs:36-45`: `Initiated` e `Utility` não fazem nada
no Android (só `Interactive` entra na sessão ADPF).

**Impacto no S21+.** O S21+ suporta AAudio de baixa latência; sem o hint, o SDL pode ficar no caminho
de maior latência (rajadas no mixer, *underruns* em cena cheia). E as threads de streaming/IO competem
com o render nos A55 sem *nice* algum.

**Correção.** `sdl3::hint::set("SDL_ANDROID_LOW_LATENCY_AUDIO", "1")` antes de abrir o stream; para
`Initiated`/`Utility` usar `setpriority(PRIO_PROCESS, 0, ...)` (ou `Process.setThreadPriority`
equivalente) e, se necessário, `sched_setaffinity` para manter o render nos núcleos grandes.

**Referências.** [3] SDL3 hints, `qos.rs`.

---

### A18 — P2 — Comentário de ADPF com a versão errada da API

`qos.rs:80` diz "ADPF, Android 13+". `APerformanceHint_getManager` é **API 31 (Android 12)**
[30]; `APerformanceHint_setThreads` é que é 34. Como o código usa `dlsym`, funciona desde a 12 — o
comentário apenas subestima o alcance. Correção: uma linha de comentário.

**Referências.** [30] NDK Performance Hint.

---

## 4. O que já está bem feito (não regredir ao "androidizar")

Isto não é cortesia: são decisões que a maioria dos ports erra e que aqui estão certas.

- **`SDL_POLL_SENTINEL=0`** (`app.rs:57`) com o motivo escrito — um dos bugs clássicos de SDL+Android
  (spin em núcleo grande).
- **Re-criação de superfície no resume** (`app.rs:1226-1237`, `replace_surface`) e reenvio do pedido de
  taxa. O Android troca a `ANativeWindow` a cada retomada; a maioria dos apps esquece.
- **`frame_pace::set_background`** para o motor parar no fim do quadro em vez de brigar com o sistema.
- **ADPF já integrado** (`qos.rs`), inclusive o `end()` que evita o abort no `exit` — raro e correto.
- **Pipeline cache em disco** + `prepare` assíncrono: é o equivalente ao "shader preloading" que a
  Rockstar usa nos ports.
- **Tradução BC→ETC2** com encoder rápido e threads limitadas a 8 (`bc.rs:14-22`), medição real no
  comentário.
- **UI com safe area** (`ui.rs:1004-1016`) e referência a insets no `egui` (`ui.rs:790`).
- **`MANAGE_EXTERNAL_STORAGE` só sob demanda** com o motivo certo (SAF não entrega caminhos).

---

## 5. Resíduos de outras plataformas (repositório "só Android")

Contagem no commit base: **113** ocorrências de `cfg(target_os/target_vendor)` em `crates/`
(iOS 26, tvOS 15, macOS 7, Linux 9, Android 77, Apple-gated dezenas), mais os diretórios e tarefas
abaixo. Remoção em 4 fases, cada uma compilável e testável:

| Fase | O que sai | Onde | Ganho |
|---|---|---|---|
| **1. Build** | `.mise/tasks/{ios,macos,tvos,linux}`, matrix `ios/windows/macos/linux-*` do `build.yml`, `docs/{getting-started,streaming,playing}.md` com seções Apple, badges e tabela do `README.md` | `.mise/`, `.github/`, `docs/`, `README.md` | CI 3× menor; ninguém tenta compilar o que não existe |
| **2. Deps e código Apple** | `crates/glue5-gpu/src/metalfx.rs` (449 linhas), ramos `target_vendor = "apple"` de `present.rs`/`perf.rs`/`exec.rs`/`done.rs`/`slabs.rs`, deps `objc2*`/`block2`, `crates/glue5/src/haptics.rs` (substituído por A7), `Upscaling::{MetalFx, MetalFxTemporal}`, `PerfOverlay::MetalHud` | `crates/`, `Cargo.toml` | ~1.5 k linhas a menos, um backend só (Vulkan) |
| **3. Desktop** | `paths.rs` (ramos macOS/Linux/Windows, `APPIMAGE`, `.app`), `mimalloc`, `packager` metadata, `stream` no modo "servidor desktop" (decidir: o S21+ ainda é cliente), `examples/headless` | `crates/glue5/src/paths.rs`, `Cargo.toml` | caminhos com um caso só |
| **4. Constantes** | `MOBILE`, `TOUCH`, `TV` (`paths.rs:20-31`) e todo `if MOBILE`/`if TV` viram código reto | todo o `crates/glue5` | o compilador passa a provar o comportamento Android |

**Ordem recomendada:** 1 → 3 → 2 → 4. Fazer o passo 4 antes dos outros transforma o código todo de
uma vez (diff gigante e irrevisável); fazer por último deixa cada remoção testável.

---

## 6. Verificação no aparelho

```bash
# 1. O que a GPU realmente expõe (features/limits/driver)
adb logcat -s glue5:* | grep -i "gpu:"
#    esperado no S21+: "Mali-G78", backend=Vulkan, driver=1.1.x (Samsung), features sem BC

# 2. O perfil de memória escolhido
adb shell cat /proc/meminfo | head -1          # comparar com LOW_MEMORY_RAM_MB=7000 (A2)
adb shell run-as local.glue5 cat files/glue5.json 2>/dev/null || adb shell cat /sdcard/Android/data/local.glue5/files/glue5.json

# 3. Taxa de atualização efetiva (A10)
adb shell dumpsys SurfaceFlinger | grep -i "frameRate\|activeMode"
adb shell dumpsys display | grep -i "mActiveModeId\|fps"

# 4. Termal (A9) durante 15 min de jogo
adb shell dumpsys thermalservice | grep -i "Thermal Status"

# 5. 16 KB (A13)
adb shell getconf PAGE_SIZE                    # 4096 hoje no S21+; a checagem é do .so, não do aparelho
readelf -lW target/aarch64-linux-android/release/libglue5.so | grep LOAD

# 6. LAN (A4)
adb shell cmd wifi set-multicast-lock ... # não existe: validar pelo log do app + teste real com 2 aparelhos
```

---

## Referências

1. Android Developers — *Behavior changes: Apps targeting Android 16 or higher*.
   <https://developer.android.com/about/versions/16/behavior-changes-16>
2. Apple — *MetalFX* (upscaling/frame interpolation em Metal).
   <https://developer.apple.com/documentation/metalfx>
3. SDL Wiki — *CategoryHints* (`SDL_HINT_ANDROID_BLOCK_ON_PAUSE`,
   `SDL_HINT_ANDROID_TRAP_BACK_BUTTON`, `SDL_HINT_ANDROID_LOW_LATENCY_AUDIO`).
   <https://wiki.libsdl.org/SDL3/CategoryHints>
4. Android Developers — *`WifiManager.MulticastLock`*.
   <https://developer.android.com/reference/android/net/wifi/WifiManager.MulticastLock>
5. Android Developers — *Local network permission*.
   <https://developer.android.com/privacy-and-security/local-network-permission>
6. Arm — *GPU Best Practices: Efficient render passes with Vulkan*.
   <https://support.arm.com/documentation/101897/0304/Fragment-shading/Efficient-render-passes-with-Vulkan>
7. Khronos — *Tile Based Rendering (TBR) Best Practices*.
   <https://docs.vulkan.org/guide/latest/tile_based_rendering_best_practices.html>
8. Arm — *Picking the Most Efficient Load/Store Operations*.
   <https://developer.arm.com/community/arm-community-blogs/b/mobile-graphics-and-gaming-blog/posts/picking-the-most-efficient-load-store-operations>
9. wgpu — *`Features`* (`TEXTURE_COMPRESSION_ASTC`, `TEXTURE_COMPRESSION_ETC2`, `SUBGROUP`…).
   <https://docs.rs/wgpu/latest/wgpu/struct.Features.html>
10. Android Developers — *Manage all files on a storage device* (`MANAGE_EXTERNAL_STORAGE`).
    <https://developer.android.com/training/data-storage/manage-all-files>
11. Khronos — *`VK_KHR_load_store_op_none`* (Vulkan 1.3).
    <https://docs.vulkan.org/refpages/latest/refpages/source/VK_KHR_load_store_op_none.html>
12. Arm — *Mali-G78 Product Support* (Vulkan 1.1/1.2, ASTC LDR/HDR, AFBC 1.3).
    <https://developer.arm.com/Processors/Mali-G78>
13. Arm — *Adaptive Scalable Texture Compression User Guide* ("ASTC é recurso opcional no Vulkan;
    verifique `textureCompressionASTC_LDR`").
    <https://developer.arm.com/-/media/Arm%20Developer%20Community/PDF/ASTC_User_Guide_102162_0001_01.pdf>
14. Android Developers — *Thermal API* (ADPF, `PowerManager.OnThermalStatusChangedListener`).
    <https://developer.android.com/games/optimize/adpf/thermal>
15. Android NDK — *Thermal* (`AThermal_getCurrentThermalStatus`, API 30+).
    <https://developer.android.com/ndk/reference/group/thermal>
16. Android Developers — *Optimize refresh rates* (Android 15: 60 Hz padrão para jogos).
    <https://developer.android.com/games/optimize/display-refresh-rate-change>
17. Android NDK — *Native Window* (`ANativeWindow_setFrameRateWithChangeStrategy`,
    `ANATIVEWINDOW_FRAME_RATE_COMPATIBILITY_FIXED_SOURCE`).
    <https://developer.android.com/ndk/reference/group/a-native-window>
18. Android Developers — *Frame rate* (estratégias de mudança de taxa).
    <https://developer.android.com/media/optimize/performance/frame-rate>
19. Khronos — *`VK_EXT_texture_compression_astc_3d`* / formatos ASTC.
    <https://docs.vulkan.org/refpages/latest/refpages/source/VK_EXT_texture_compression_astc_3d.html>
20. GSMArena — *Samsung Galaxy S21+ 5G* (Exynos 2100, 8 GB, 1080×2400, 120 Hz).
    <https://www.gsmarena.com/samsung_galaxy_s21+_5g-10625.php>
21. Wikipedia — *Samsung Galaxy S21* (tabela de telas, 48–120 Hz, Android 15/One UI 7 final).
    <https://en.wikipedia.org/wiki/Samsung_Galaxy_S21>
22. SamMobile — *Software updates for Galaxy S21, S21+, and S21 Ultra have been terminated*.
    <https://www.sammobile.com/news/galaxy-s21-plus-ultra-software-updates-stopped/>
23. PhoneDB — *SM-G996B/DS Galaxy S21+ 5G* (8 GiB, Mali-G78, 2412×1080 efetivo).
    <https://phonedb.net/index.php?m=device&id=17964>
24. Android Developers — *Storage updates in Android 11* (acesso a `Android/data` de outros apps).
    <https://developer.android.com/about/versions/11/privacy/storage>
25. Android Developers — *Support 16 KB page sizes*.
    <https://developer.android.com/guide/practices/page-sizes>
26. Android Developers Blog — *Transition to using 16 KB page sizes*.
    <https://android-developers.googleblog.com/2025/07/transition-to-16-kb-page-sizes-android-apps-games-android-studio.html>
27. SDL Wiki — *`SDL_GetWindowSafeArea`*.
    <https://wiki.libsdl.org/SDL3/SDL_GetWindowSafeArea>
28. Android Developers — *Support display cutouts*.
    <https://developer.android.com/develop/ui/views/layout/display-cutout>
29. SDL Wiki — *README-android* (insets e safe areas a partir do Android 15).
    <https://wiki.libsdl.org/SDL3/README-android>
30. Android NDK — *Performance Hint Manager* (`APerformanceHint_*`, `setThreads` na API 34).
    <https://developer.android.com/ndk/reference/group/a-performance-hint>
