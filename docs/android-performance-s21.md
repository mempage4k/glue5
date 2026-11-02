# Android performance investigation: Galaxy S21 Plus (Exynos 2100)

## Purpose and scope

This document tracks a device-specific, static review of GLUE 5 for the Samsung Galaxy S21 Plus 5G with Exynos 2100 and Mali-G78. It is an investigation record, not a claim that the device has a measured defect or that a proposed optimization is ready to implement.

The initial findings were supplied by the user from a Kimi 3.1 static review. That review says it did not run or profile the app on the device. Claims below are therefore labeled as reported and must be independently checked against the pinned source/dependency and, where they concern performance, measured on the phone before changes are made. The supplied report did not include source URLs or profiling data.

Este é o backlog complementar à [revisão Android](android-review.md): lá ficam achados de comportamento/API e o status das correções; aqui, as hipóteses de desempenho e os critérios para medi-las. Os dois documentos devem ser mantidos em conjunto.

## Findings to verify

### 1. Vulkan surface pre-rotation — high-risk investigation

**Reported:** Android is forced to landscape by `platform/android/AndroidManifest.xml`; the presentation path does not pre-rotate; `wgpu-hal` 30.0.1 uses `preTransform = IDENTITY`. The report hypothesizes that a portrait-native phone may rely on compositor rotation, with possible extra composition, bandwidth, power, and heat.

**Proposed by the report:** use `currentTransform`, rotate the final pass, update viewport/scissor and coordinate handling, and recreate/reconfigure the swapchain when the transform changes. The report notes that the public `wgpu` API may not expose the needed setting, so a backend patch or direct Vulkan swapchain work could be required.

**Gate:** first inspect the exact pinned `wgpu-hal` source and Android/Vulkan contract; then measure the current presentation path on the S21 Plus. Do not patch the backend based only on this static claim.

### 2. ADPF work-duration reporting

**Reported:** `crates/glue5-gpu/src/submit.rs` computes `now - last_shown` and passes it through `glue5_runtime::qos::frame`, which reports it using `APerformanceHint_reportActualWorkDuration`. This interval may include presentation pacing, VSync/backpressure, or idle time rather than CPU work alone.

**Proposed by the report:** measure the actual work interval for the participating threads, exclude deliberate sleeps and presentation waits, and keep frame cadence separate from work duration. CPU/GPU durations may also need distinct metrics.

**Gate:** confirm the Android API's required semantics and trace the participating thread lifecycle before changing the metric. Collect before/after timing evidence.

### 3. 60 Hz fallback versus 120 Hz displays

**Reported:** `crates/glue5-gpu/src/present.rs::frame_budget()` uses 60 fps when the configured rate is zero. The report also says the display refresh rate is read only at startup, so a later 60/120 Hz change may not update pacing, QoS, the performance overlay, or available settings.

**Proposed by the report:** follow display refresh/VSync changes dynamically and update those consumers, using approximately 8.33 ms at 120 Hz and 16.67 ms at 60 Hz.

**Gate:** verify the intended meaning of the “Display” setting, SDL/wgpu refresh-rate facilities, and whether the current behavior is a product decision or a defect. The 60 fps fallback is not inherently wrong when 60 fps is explicitly desired.

### 4. BC texture fallback, memory, and mip quality

**Reported:** when BC compression is unavailable, textures are decoded; some color formats are then re-encoded as ETC2. Approximate per-texel storage ratios in the report:

| Source | Fallback | Approximate relative storage |
|---|---|---:|
| BC1 | ETC2 RGBA8 | 2× |
| BC2 / BC3 / BC7 | ETC2 RGBA8 | 1× |
| BC4 | R8 | 2× |
| BC5 | RG8 | 2× |
| BC6H | RGBA16F | 8× |

The report also identifies an intentional drop of the largest mip for BC 2D textures with at least two mips and a largest dimension of 512 or more. This reduces resolution as a memory tradeoff.

**Proposed by the report:** measure loading time and memory per format; evaluate ETC2/EAC/ASTC only where the actual adapter exposes the needed features; cache transcodes; and consider making the mip-quality/memory tradeoff explicit.

**Gate:** preserve alpha and color-space behavior, validate format support and rendering correctness, and measure on-device before changing the fallback.

### 5. Nested texture-decoding parallelism

**Reported:** a decode pool can run two jobs while each BC decode/ETC2 encode may create up to eight scoped threads, potentially reaching 16 inner workers, in addition to other GLUE 5 and GPU compilation threads. The report hypothesizes oversubscription, context switching, cache pressure, and extra thermal load; it does not prove these effects occur on the S21 Plus.

**Proposed by the report:** use a shared worker budget or persistent pool and compare controlled limits such as 2, 4, and 6 workers, recording texture load duration, frame times, memory, and temperature.

**Gate:** benchmark representative texture streaming before redesigning the pools. Do not assume that either maximum parallelism or a smaller worker count is always faster.

### 6. Android thermal adaptation

**Reported:** the app has a thermal watcher behind an iOS-only target condition, but no equivalent Android thermal adaptation was found in the reviewed paths.

**Proposed by the report:** investigate Android Thermal APIs and, if measurements justify it, gradually adjust resolution/upscaler, frame-rate limit, and worker counts with hysteresis; restore quality gradually as thermal headroom returns.

**Gate:** first add/collect thermal and frame-time instrumentation on the S21 Plus. Do not claim throttling or add adaptive quality solely from static inspection.

**Implemented (2026-11-01, not yet measured on the device):** the Android
Thermal API is now read through the NDK (`AThermal_getCurrentThermalStatus`,
API 30, and `AThermal_getThermalHeadroom`, API 31, both looked up at run
time), in `crates/glue5-qos/src/thermal.rs`. A governor turns the readings
into a level (0 to 3) with hysteresis: it rises after the heat has been
asked for six seconds and falls one level at a time, every 45 seconds, so a
phone near a threshold does not flap. The system's own verdict (status
moderate or worse) is a floor the level jumps to at once.

The level holds the game through the engine's limiter hook
(`crates/glue5-runtime/src/frame_pace.rs`): 40 fps at level 1, 30 fps at
levels 2 and 3, on a 120 Hz panel (30 on a 60 Hz one). With
*Frame rate and resolution*, the last level also renders at 85% of the
chosen scale (the engine rebuilds its render targets for it: one hitch).
The performance hint's target follows the cap, so the system does not
raise clocks chasing a rate the host is not keeping, and the display is
asked for the same rate. The setting is in Settings › Video › *Thermal
back-off* (Off / Frame rate / Frame rate and resolution), on by default on
Android and off elsewhere. The benchmark is left alone: it measures what
the phone can do.

**Still to measure on the S21 Plus:** the thresholds (0.80 / 0.90 / 0.97 of
the headroom forecast) are a first guess; the `thermal:` log lines and the
overlay's *thermal* row are what to tune them from. The headroom forecast
is not available before Android 12: there the system's status alone decides,
which is slower to react.

## Second round: the other suggestions (2026-11-01)

The rest of the advice that came with the thermal work was checked against
this fork, and applied where it was safe to do so without the device at
hand. What follows is what is already in the tree, what was added, what
was rejected and why, and what still needs the phone.

### Already there (verified, not duplicated)

- **The upstream command-buffer work.** The `next` branch's "one command
  buffer per pass" and "passes before barriers" changes are in
  `patches/wgpu-core` (0007, 0009, 0011) and `patches/wgpu-hal` (0005),
  and `patches/wgpu-hal/0010` is the read-only depth/stencil store-op
  change. The Mali command-buffer interleaving that the report describes
  is therefore not this fork's problem.
- **The pipeline cache is persisted.** `glue5_gpu::PipelineCache` is
  created from the user's cache folder (`glue5-gpu/src/lib.rs`), filled
  after the first frame and written back (`present.rs::save_caches`), so
  the second launch does not rebuild the pipelines.
- **`LoadOp::Clear` is what the passes use.** A pass whose attachment has
  a pending clear loads it as a clear (`ensure_pass`,
  `glue5-gpu/src/draw.rs`); a clear of another subresource becomes a pass
  of its own (`exec.rs::record_clear`), which is what a tiling GPU wants
  (`vkCmdClearAttachments` is the slower path on Mali).
- **ThinLTO is available and measured.** `GLUE5_GAME_LTO=1`
  (`crates/glue5-game/build/optimise.rs`); it gave no gain on the
  desktop, so it is off.

### Applied in this round

- **The phone's big cores** (`crates/glue5-game/build/main.rs`): the game's
  C is compiled for Android with `-mtune=cortex-a78` (the Exynos 2100's
  big cores; `-march` still says what is generated). `GLUE5_ANDROID_TUNE`
  picks another tune (`cortex-x1`) or turns it off (`none`), and the tune
  is part of the objects' cache key, so a change rebuilds them.
- **PGO on the phone** (`mise run pgo:android`): an instrumented build
  (`GLUE5_GAME_PGO=generate`) runs the benchmark on the device
  (`GLUE5_BENCHMARK=1`: the benchmark starts at once and the app ends when
  it is over, so the profile is written), the profile is pulled from the
  app's own folder in the shared storage and merged with the NDK's
  `llvm-profdata` into `target/glue5-tools/pgo/android/game.profdata`,
  which the next build uses (`GLUE5_GAME_PGO=<that file>`).
- **The graphics preset** (Settings › Video › *Graphics preset*, Android):
  the *Phone* preset writes the game's own settings down — smaller
  textures, no reflections, no soft shadows, no grass, no anisotropic
  filtering (`glue5_runtime::game_settings::PHONE`). It is the one lever
  that cuts the bytes the GPU reads, which is what a Mali-G78 fed by a
  phone's memory feels most.

### Rejected after inspection

- **Arm's Accuracy Super Resolution.** The library
  (`arm/accuracy-super-resolution-generic-library`) is C++ with GLSL and
  HLSL shaders for Vulkan, DirectX 12 and Metal — there is no WGSL, so it
  would need a full port to this renderer — and it is derived from
  **FSR 2.2.2**, while this fork already ships **FSR 2.3** (through
  `patches/sp-fidelity`). It is not an upgrade.
- **Swappy.** It paces Vulkan and OpenGL swapchains; the pacing of an
  Android game is `Choreographer`'s and
  `ANativeWindow_setFrameRate`'s, which the app already uses
  (`glue5-gpu/src/present.rs`). There is no place for it here.
- **Turnip and libadrenotools.** They are Adreno drivers; the S21 Plus has
  a Mali. The app's driver picker says so, and the phone's own driver is
  what runs.

### Still to do with the phone at hand

- **Measure before touching the passes.** The pass census is already in
  the GPU log (`op N/frame` lines, `Stats.pass_ends`): the split between
  the copies, the clears, the mip generation, the readbacks and the
  dispatches is what says where the frame goes. The mip generation alone
  is 12 to 14 passes, about 1 ms.
- **`StoreOp::Discard` has no safe case yet.** Every attachment this
  engine draws into is read again later (the scene's colour by the
  compose pass, the depth by the motion vectors, the mip chain by the
  next level), and a discarded attachment is not in memory to be sampled.
  The one place it would pay is the frame's final image, which the
  compose pass still reads.
- **The thermal thresholds** (0.80 / 0.90 / 0.97 of the headroom
  forecast) are a first guess; the `thermal:` log lines are what to tune
  them from.
- **The 30 fps goal.** Cap the frame rate at 30 (40 on the 120 Hz
  panel), lower the resolution scale and turn the graphics preset on;
  measure with the game's own benchmark (Benchmark on the start page) and
  the performance overlay.
- **Samsung's Game Booster** (Settings › Game Booster on the phone) can
  hold the performance or the battery; its own settings are outside the
  app's reach, and the thermal back-off is the app's answer to the same
  heat.

## Behaviors the report says to preserve
- Request only GPU features exposed by the adapter.
- Keep the current Vulkan 1.1 requirement unless separate compatibility evidence supports changing it.
- Keep FSR conditional on the required adapter features.
- Reapply the requested frame rate after surface recreation.
- Do not add Turnip/libadrenotools as a Mali path; those are Adreno-specific mechanisms.

These are also report claims to verify where relevant, not a substitute for the code's existing regression tests.

## Suggested investigation order

1. Confirm the current Android Build on the current branch tip and resolve any compile blocker before interpreting runtime results.
2. Verify ADPF work-duration semantics and instrument cadence versus actual work.
3. Measure texture fallback memory/load time and nested worker behavior.
4. Verify 60/120 Hz pacing and dynamic refresh changes.
5. Independently confirm the Vulkan pre-rotation claim; treat any backend/swapchain change as high risk.
6. Evaluate Android thermal adaptation and ASTC/EAC/mip options only after obtaining on-device measurements.

## Status

- This is a static-review backlog; the S21-specific changes so far are the
  Android thermal back-off (finding 6), written from the API's
  documentation, and the second round above (the CPU tune, PGO on the
  phone, the graphics preset). None of them has been measured on the
  device.
- No device profiling or validation result is recorded yet.
- Record each investigation and its measurements here; put stable architectural decisions in `design.md`, and user-facing settings in `playing.md`.
