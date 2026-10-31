# GTA V runtime patches: durable notes for future work

This document is the starting point for any future Arena chat or coding task
about patching GTA V behaviour from GLUE 5. It records facts, hard limits, open
questions, and the evidence still needed. Read it before proposing a patch.

The project diary is chronological and may contain superseded hypotheses. When
the diary and this document disagree, prefer the evidence and status recorded
here, then update both.

## Non-negotiable rules

1. **Do not put Rockstar game assets in this repository.** No extracted `.gfx`,
   `.ysc`, textures, audio, RPF files, decompiled scripts, or whole game-data
   trees. The user may supply files for local analysis outside Git. Record only
   file names, sizes, hashes when useful, format facts, offsets, and behaviour.
2. **Patch the user's own runtime copy, never the original game files.** The
   existing model is a generated copy under the app's scratch/overlay area. A
   patch must fail open to the untouched file if its preconditions do not match.
3. **Keep the game's scripts and mission state in charge.** Do not rebuild the
   phone or another game system natively unless the user explicitly changes
   that scope. A patch should alter a narrow boundary and let the original
   scripts continue to own state.
4. **Do not fight memory every frame when a load-time or call-boundary patch is
   possible.** The confirmed "phone keeps opening and closing" bug came from
   rewriting the phone position every frame while the game wrote its own. See
   "Phone position patch warning" below.
5. **A device report is evidence; CI is not device validation.** There is no
   Rust/Android toolchain in the usual Arena sandbox. Do not call a gameplay
   patch fixed until the user tests it on the Galaxy S21+ and says so.
6. **External decompiled scripts are hypotheses, not ground truth for this
   build.** The game data and scripts can be from different revisions. The
   user's files and Import reports from the actual build override them.

## Current patch mechanism

GLUE already changes some game files as they are read:

- `crates/glue5-runtime/src/overlay.rs`
  - `OVERLAYS` lists paths below the game `data/` directory.
  - `overlay::game_file(rel, real, scratch)` writes a changed copy under
    `scratch/overlay/` and has the engine read that copy.
  - It currently handles text/XML/control metadata only.
  - It enforces the **same byte length** as the source because the engine or
    its manifest may know the original size. Do not assume a longer or shorter
    binary patch is accepted until that loader path is proven.
  - Current examples are `gameconfig.xml`, `pausemenu.xml`, `frontend.xml`, and
    controller `.meta` files.
- `crates/glue5-runtime/src/vfs.rs` calls the overlay for eligible game files.
- A binary Scaleform patch is not yet implemented. Extending the overlay to a
  `.gfx` requires knowing whether this build reads that movie as a loose file,
  from an extracted directory, from an archive/image, or from memory after the
  archive is parsed.

## Long-term architecture direction: active GLUE–GTA communication and split rendering

On 2026-10-31 the user recorded a future goal, not an implementation request for
this turn: build a more active, structured communication path between GLUE 5
and GTA V, and separate the 3D scene's render resolution from the game's HUD
and menus. The intended result is being able to lower the 3D resolution while
keeping the HUD/menu legible at the display resolution, and to make game
mechanisms easier to inspect and control through GLUE.

This is a design direction, not a proven capability or an approved patch plan.
The code already has specific hooks and input bridges, but no general-purpose
GLUE–GTA command/telemetry protocol is established. Before implementation:

- Map the process, WASM/host, hook, and renderer boundaries; distinguish the
  existing narrow hooks from a new general communication channel.
- Define a small, versioned, bounded contract for observations and commands.
  Start with read-only telemetry, then add narrowly scoped commands at safe
  game-owned boundaries; do not expose arbitrary memory writes or fight the
  game's per-frame state ownership.
- Map the render passes and targets to establish whether 3D scene and HUD/menu
  can be rendered or composed independently. Preserve the game's UI at the
  target display resolution while scaling the 3D scene only if the actual
  pipeline supports it; account for clears, depth, blending, ordering, and
  coordinate transforms.
- Gate any prototype on measurements and a fail-open path. Simply changing
  output resolution or redirecting a guessed pass is not evidence that the HUD
  will remain visible or crisp.

Other useful intervention points already present:

- `crates/glue5-runtime/src/script_assets.rs`
  - `scaleform_request` hooks `REQUEST_SCALEFORM_MOVIE` and detects movies this
    data lacks.
  - `method_begin` hooks `CScaleformMgr::BeginMethodInternal`.
  - `param_int` hooks `CScaleformMgr::AddParamInt`.
  - `PHONE_MOVIE` is identified when the phone movie receives
    `SET_TITLEBAR_TIME`.
  - The phone home-screen workaround moves an unsupported app out of view 1.
- `crates/glue5-runtime/src/prompts.rs`
  - `scaleform_call_method` hooks
    `CScaleformMgr::CallMethodInternal(...)`, including GFxValue arguments.
- `crates/glue5-runtime/src/input.rs`
  - `scaleform_begin_method` and `scaleform_add_param_float` hook the other
    Scaleform method-building path.
  - They currently rewrite a browser `SET_INPUT_EVENT(1001.0)` to accept
    (`16.0`). Be careful: phone `SET_INPUT_EVENT` calls observed in scripts use
    integer parameters through a different path.
  - `control_update` hooks `CControl::Update()` and is the point where several
    runtime state patches are applied.
- `crates/glue5-game/hooks/*.rs`
  - Every hook must match an exact function name in the user's `game.wasm`.
  - A guessed hook name can fail the Android import/codegen build. Prefer
    existing hooks; add a new hook only after identifying the exact symbol and
    letting CI verify it.
- `crates/glue5-runtime/src/prompts.rs::hold_input`
  - Can pulse a `rage::InputType` for a minimum press. It is useful for a real
    game input, but repeatedly stepping a selection is not a direct touch.

## The user-supplied file tree received on 2026-10-31

The user supplied only the first level of a `playgta5.com` folder:

- It is the browser/launcher package, not a full expanded GTA V data listing.
- Important visible roots are `data/`, `_mods/`, `_movie/`, `glue5-scenes/`,
  `branding/`, and launcher JavaScript/CSS/HTML files.
- Inside `data/`, the supplied tree only shows `bootset.json`,
  `bootset_low.json`, `common/`, `manifest.json`, and `x64/`; it does not expand
  `common/` or `x64/`.
- It reported 25 directories and 83 files, so it cannot locate the phone movie
  or scripts. Do not infer from this tree that a file is absent.

The user's targeted search then confirmed the archive locations in this
installation:

- Scripts: `data/x64/levels/gta5/script/script_rel.rpf`.
- Scaleform movies: `data/x64/data/cdimages/scaleform_generic.rpf`.

These paths replace guesswork about where the phone assets live. A filesystem
search cannot see inside either RPF; open them with an RPF-aware tool (for
example OpenIV or CodeWalker) and search/export by entry name. Do **not** ask
the user to upload either complete RPF.

## Files received from the user's PC installation on 2026-10-31

The user uploaded `glue5/cellphone.zip` from Google Drive after exporting from
OpenIV on the GTA V PC installation described as the Optijuegos 40 GB install.
It contained six raw Scaleform files: the four story-phone variants requested
plus `cellphone_alert_popup.gfx` and `cellphone_cutscene.gfx`. The ZIP and
extracted assets were used only in temporary local analysis; do not add any of
them to Git.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `cellphone_ifruit.gfx` | 135,963 | `256fc5a858ff83ff60f9f6d25ddcd98b37a64949ba39d38feca0b76212d748c0` |
| `cellphone_badger.gfx` | 130,397 | `d88ff8ae5ee599f5c26aa8c297ac2084f195a348d4539dd2ad3439418b7d1697` |
| `cellphone_facade.gfx` | 126,403 | `806b9da50d8fa97b91bc5da197bee66880bcf7e5fa8c5b86ca87a9a382bceba8` |
| `cellphone_prologue.gfx` | 56,882 | `a6d51e700e994a23dc2c139e95c3b6e65c3550e7e2a91d7b644974488138bdc1` |
| `cellphone_alert_popup.gfx` | 14,545 | `b63477d5ce0f3bab57bcf5d3852930eaeb17bce946c96b77a382c3c1830c6ecf` |
| `cellphone_cutscene.gfx` | 6,676 | `70783b90e621769bf27b6e5466078986b39dd93324edc3dbb8b6dfddf38b7b99` |

The main movies are GFX v8 containers. For inspection only, a temporary copy of
each header was normalized to the equivalent FWS signature and its AVM1
(ActionScript 1/2) blocks were disassembled with `swf-toolkit` 0.1.0. The user
provided originals were not altered. The exact GTA build/revision is still not
known, so keep all conclusions tied to these hashes until confirmed against
scripts or runtime traces from the same install.

### Additional targeted batch received from Google Drive on 2026-10-31

The user added `glue5/assets/assets31-10-26-12-05.zip`. It contains the ten
requested entries and many related pause-menu movies, textures, scripts, and
two complete `scaleform_frontend.rpf` archives. The archive was downloaded and
inspected locally; neither the ZIP nor Rockstar assets belong in Git. Its flat
entry names do **not** preserve whether a numbered variant came from `data/` or
`update/`.

Core entries and observed sizes:

| Entry | Bytes / observed variants | Status |
| --- | ---: | --- |
| `cellphone_controller.ysc` | 21,331 | Received; raw RSC7 resource, not yet decompiled. |
| `cellphone_flashhand.ysc` | 13,999 | Received; raw RSC7 resource, not yet decompiled. |
| `ingamehud.ysc` | 243,234 each | `ingamehud.ysc` and `(1)` are byte-identical. |
| `pausemenu.ysc` | 422 | Received. |
| `pausemenu_map.ysc` | 26,740 | Received. |
| `frontend.xml` | 26,189 / 36,998 | Both variants differ. |
| `pausemenu.xml` | 209,670 / 248,852 | Both variants differ. |
| `hud.gfx` | 238,772 | Received. |
| `minimap.gfx` | 207,370 / 136,647 | Both variants differ. |
| `pause_menu_pages_settings.gfx` | 50,214 / 60,053 | Both variants differ. |

The ZIP also contains `pausemenu_multiplayer.ysc`, `pausemenu_sp_repeat.ysc`,
`pausemenu_example.ysc`, `frontend.awc`, HUD/frontend YTDs, and many more
`pause_menu_pages_*.gfx`/`.ytd` variants. It includes
`scaleform_frontend.rpf` (13,664,768 bytes) and
`scaleform_frontend(1).rpf` (3,047,424 bytes). Keep these local and out of Git.

A local RPF table-of-contents cross-check provides only a partial correlation:
`scaleform_frontend.rpf` has an `OPEN` TOC with 70 entries (69 files), including
`pause_menu_pages_settings.gfx` with advertised uncompressed size 50,214 and
`pause_menu_pages_keymap.gfx` at 46,155 bytes. The first size matches the
non-numbered extracted Settings movie, and the Keymap movie is referenced by
`pausemenu(1).xml`. However, the payloads are encrypted, so this is not a
byte-for-byte content match or proof of `data`/`update` origin. The other RPF
uses NG-encrypted TOC data and cannot be indexed without the matching game
keys. Keep both archives and all extracted variants in parallel; this does not
resolve which copy the runtime loads.

**Variant handling is a hard requirement:** the user clarified that numbered
names represent two intentional game copies, one under `data` and one under
`update`; they are not accidental duplicates. Never infer which is which from
`(1)`/`(2)`, discard a differing variant, or select one for a patch until its
origin/active loading path is established. Byte-identical files may be treated
as identical for analysis, but retain the knowledge that both sources existed.
The ZIP flattened the original paths, so the mapping is still unresolved.

### Findings from the additional batch

- The two `ingamehud.ysc` files are byte-for-byte identical. Exact-equal page
  movie pairs include `pause_menu_pages_char_select.gfx`, `corona.gfx`,
  `corona_lobby.gfx`, `friends.gfx`, `friends_mp.gfx`, and `store.gfx`.
  Differing examples include `frontend.xml`, `pausemenu.xml`, `minimap.gfx`,
  `pause_menu_pages_map.gfx`, and `pause_menu_pages_settings.gfx`. Do not treat
  a repeated filename as evidence of an identical asset.
- `frontend(1).xml` has `ReplayHud` and `TextEntryBox` sections, three extra
  `ScriptHud` components (`HUD_CHIP`, `HUD_CHIP_CHANGE`,
  `HUD_DIRECTOR_MODE`), replay HUD components, one extra minimap interior, and
  six additional warning-screen button entries. The 21 core `<Hud>` components
  remain the same set, but priorities/layout values differ; for example
  `HUD_WEAPON_WHEEL_STATS.sizeX` is `0.098` vs `0.1156`. Minimap masks and the
  `GameStream` positions also differ. These are evidence of real layout/schema
  divergence, not a rendering-pass split.
- `pausemenu(1).xml` contains 147 `MenuScreen` entries vs 137, and references
  41 `cGfxFilename` entries vs 39. It adds the
  `PAUSE_MENU_PAGES_KEYMAP` movie and
  `FE_MENU_VERSION_LANDING_KEYMAPPING_MENU`; `MISSIONCREATOR` already appears
  in both variants. Counts of menu options/buttons/preferences also differ.
- The common `frontend.xml` has malformed comments and missing whitespace
  between some XML attributes. The comparison parser repaired these issues
  only in memory; originals were not modified. Treat parse-derived counts as
  structural inspection, not proof that a standards-compliant XML parser can
  consume the game's file.
- Temporary `swf-toolkit` inspection reports `hud.gfx` as GFX/SWF v8, stage
  1280 × 720 at 30 fps. Its bytecode exposes `SET_DISPLAY_CONFIG` and logic for
  safe-area/display configuration and HUD component placement. This proves
  movie-level layout handling only; it does **not** prove that 3D and HUD use
  separate render targets or resolutions.
- The two `minimap.gfx` variants are different generations (GFX/SWF v15 vs
  v10; both stage 191 × 136, but reported rates are 30 vs 49 fps). The
  pause-map and settings movies inspected are GFX/SWF v8, 868 × 430, 30 fps.
- Unlike the four phone movies, the pause-menu settings page contains concrete
  pointer logic in both variants: `mPress`, `_xmouse`/`_ymouse`, and, in one
  variant, `onPress`/`onRelease` plus hover/out callbacks. The pause-map page
  also uses `_xmouse` in `mPress`. This is a useful in-game reference for how a
  Scaleform movie can implement mouse navigation; it is **not** proof that the
  engine currently dispatches those events to a phone movie. The phone movies
  still have no pointer handler in their inspected bytecode, so direct phone
  taps need a phone-specific movie/runtime path, not D-pad automation.
- The raw phone scripts begin with `RSC7` and resource version `0x0c`. The user
  later supplied successful same-PC decompilation outputs for both; see
  "Same-install phone-script decompilation report" below. The Arena sandbox
  still lacks .NET/Mono, so inspect the supplied outputs locally and do not add
  Rockstar-derived source to Git. The raw scripts are already received; do not
  ask the user to upload them again.

The batch helps identify state owners, Scaleform APIs, and layout divergence.
It does not establish the active `data`/`update` copy, validate a phone patch, or
prove that the HUD and 3D scene can be rendered at different resolutions. The
latter still requires GLUE render-target/pass/clear/order analysis. No build or
device test was run during this asset-inspection step.

## Phone Scaleform facts established so far

The GFX facts below are from the exact user-supplied movies. Script-flow facts
now include the user's same-PC decompilation outputs; those are described in
"Same-install phone-script decompilation report". Treat `func_XX` identifiers
as generated decompiler labels, not original source names.

### Findings from the exact user-supplied GFX files

- All four requested variants were inspected as AVM1/AS2 bytecode. They are
  GFX/SWF version 8. The iFruit, Badger, and Facade stages are 256 × 344; the
  Prologue stage is 256 × 192. Their frame rates differ (60, 30, 30, and 24
  fps respectively), so preserve each movie's own header/timing when testing a
  patch.
- Each phone movie's `SET_INPUT_EVENT` method accepts only direction values
  1–4 and forwards them to the current interface's `navigate` method as
  `UP`, `RIGHT`, `DOWN`, or `LEFT`. `GET_CURRENT_SELECTION` asks the current
  interface for its selection. This directly corroborates the selection path
  in the public scripts for the supplied movie versions.
- No pointer-coordinate or mouse-event identifiers were found in the AVM1
  disassembly of the four main movies (`SET_MOUSE_INPUT`, `GET_MOUSE_POSITION`,
  `_xmouse`, `_ymouse`, `onPress`, `onRelease`, `onMouseDown`, `hitTest`, or
  cursor methods). This is strong evidence against a built-in phone-film tap
  API, though the same-build scripts and runtime traces are still needed to
  close the loop.
- `SF_BASE_CLASS_MOUSE` appears in the three normal story movies, but its
  disassembly use is only as the value 10 in the generic
  `GameInterface` external-interface type enumeration; it does not implement
  or identify a mouse-coordinate handler. Do not treat the string by itself as
  evidence of touch support.
- The extra `cellphone_alert_popup.gfx` and `cellphone_cutscene.gfx` were also
  received and scanned. Keep them available for later call/popup/cutscene
  coverage, but they are separate from the four primary story-phone variants.

- One movie handle is used for the story phone. The script requests:
  - `cellphone_badger` for Franklin;
  - `cellphone_ifruit` for Michael and other normal story use;
  - `cellphone_facade` for Trevor;
  - `cellphone_prologue` in the prologue.
  A patch for only `cellphone_ifruit` is incomplete.
- In the inspected code, the app-launch path is selection-driven; a direct
  pointer-to-slot path has not been found:
  - the script sends `SET_INPUT_EVENT(1)` for up, `(2)` for right, `(3)` for
    down, and `(4)` for left;
  - in keyboard-and-mouse mode, controls 180 and 181 (phone scroll forward and
    backward) walk the selection through the 3 x 3 home grid in sequence;
  - confirmation reads the movie with `GET_CURRENT_SELECTION`;
  - the script maps that returned slot to an app it populated with
    `SET_DATA_SLOT`, then starts the app script.
- No `GET_MOUSE_POSITION`, `SET_MOUSE_INPUT`, or cursor method was found in the
  phone script or in public `cellphone_ifruit` API lists. Other Scaleform
  movies (`CONTROLLER_TEST`, `ATM`, warehouse/browser-like screens) do have
  mouse methods; do not transfer those APIs to the phone without evidence.
- Therefore, writing only absolute mouse position or a synthetic relative mouse
  delta into the engine input block is **not established** as a phone-home tap
  solution. The earlier delta change remains mouse-like input for other paths,
  but must not be described as the confirmed phone fix.
- The user's desired interaction is direct: touching a visible app should make
  the game open that app at the moment of the tap. Visible selection stepping
  through intermediate apps is not the desired final design.
- Candidate future injection points, in order of preference:
  1. a load-time movie patch that can set or expose the current selection for
     the exact tapped slot, followed by the game's normal confirmation path;
  2. a call-boundary intervention around `DISPLAY_VIEW` or
     `GET_CURRENT_SELECTION`, if traces prove it synchronises both the movie
     and the script's saved selection;
  3. a proven engine/Scaleform method invocation for the phone movie.
  Selection stepping is only a fallback or diagnostic, not the goal.
- Current diagnosis in PR #12 adds an Import report section, "Recent phone
  movie methods", recording `SET_INPUT_EVENT(n)`, `DISPLAY_VIEW(view,
  selection)`, and `GET_CURRENT_SELECTION`, bounded to 64 lines. Use it to see
  what the actual build does around a tap before choosing an injection point.

### Same-install phone-script decompilation report (2026-10-31)

The user supplied Desktop Commander outputs for the two raw scripts, with the
reported input sizes matching the targeted exports:

| Input | Input bytes | Decompiled C output bytes |
| --- | ---: | ---: |
| `cellphone_controller.ysc` | 21,331 | 176,874 |
| `cellphone_flashhand.ysc` | 13,999 | 153,958 |

The report says both commands exited 0 and logged `All done & saved!`. It used
`maybegreat48/GTA-V-Script-Decompiler`, commit
`2958d816e8f6db02a399c7b570c7074a0b2b1567`; the local build target was adjusted
to .NET SDK 10.0.103. The repo does not declare a license, so do not vendor or
redistribute its code or the full decompiled Rockstar scripts. The user's
outputs/logs are analysis material only and are not in Git.

Confirmed from the supplied decompilation:

- `cellphone_flashhand::func_83` fills home-screen slots with `SET_DATA_SLOT`
  and builds `Global_8786[slot] = app_id`.
- `func_20` requests `GET_CURRENT_SELECTION` on a normal frontend accept
  (`Global_20352`) or under special conditions (`Global_8254` bit 10 or local
  `iLocal_122 == 1`). The result is asynchronous; `func_44` reads it, maps the
  selected slot through `Global_8786`, and follows the game's existing app
  start/request path.
- This is an existing **selection-return/confirm path**, not a pointer hit
  test. It returns whatever selection the Scaleform currently has; by itself
  it does not select the icon under a finger or prove that a touch event reaches
  the movie. `SET_INPUT_EVENT`/`CELL_SET_INPUT` values 1–4 remain directional
  navigation.
- The special flags are not generic tapped-slot selectors. Bit 10 forces
  script app ID 2 (Internet in the populated home-screen table) and is cleared.
  When `Global_2803654 == 1`, the main loop schedules `iLocal_122`; the next
  selection return is forced to app ID 0 (Contacts in the populated table) and
  the flag is cleared. In these two scripts, there is no write setting bit 10
  or `Global_2803654` to 1; only reads/clears/resets appear. No literal `TOUCH`
  string was found. Their producers, if relevant, are outside these two scripts.
- The same-install scripts also confirm the position writers. In
  `cellphone_flashhand.func_111` (decompiler position `0x7263`): for HD the
  visible endpoint `Global_20329[0]` is
  `{GET_SAFE_ZONE_SIZE()*117.2,
  GET_SAFE_ZONE_SIZE()*-158.8, -113}` and the hidden endpoint
  `Global_20336[0]` is `{GET_SAFE_ZONE_SIZE()*117.2,
  GET_SAFE_ZONE_SIZE()*-53.3, -113}`. Non-HD uses visible
  `{GET_SAFE_ZONE_SIZE()*85.7, GET_SAFE_ZONE_SIZE()*-121.8, -91.5}` and
  hidden `{GET_SAFE_ZONE_SIZE()*85.7, GET_SAFE_ZONE_SIZE()*-35.3, -91.5}`.
  It initializes `Global_20311` from the visible endpoint and calls
  `SET_MOBILE_PHONE_POSITION`; `func_15` (decompiler position `0x277E`)
  interpolates the phone animation between endpoints and calls that native,
  and `func_5` (position `0x2036`) sets the position during transitions.
  `cellphone_controller.func_89` (position `0x4F5D`) has a parallel transition
  path and also calls `SET_MOBILE_PHONE_POSITION`.
  `SET_MOBILE_PHONE_SCALE(500f/575f)` appears in separate flashhand state paths;
  its role as a user-facing enlargement control has not been established. A
  community native reference describes 500 as minimum/default, warns that
  scale resets on close, and recommends adjusting position for a larger scale;
  treat that as a candidate to test, not validation for this game build
  ([Cfx native reference](https://docs.fivem.net/natives/?_0x41F37C3427C75AE0=)).
  The native position boundary is shared by both scripts; writing `CPhoneMgr`
  position from a separate per-frame hook would still race the game's writers.
- The practical direct-touch route remains: obtain the tapped slot from the
  phone movie/runtime, then let the existing script map that slot to its app
  and launch it. A movie-side hit test/selection setter or a narrowly filtered
  Scaleform-result intervention could be explored; it must preserve the normal
  app table and both `Global_20387/Global_20388` return-selection state. Do not
  use directional stepping as the final solution.

### Cross-check with the current GLUE touch path

- `Touch::Finger::Phone` keeps the actual screen point; a tap emits a mouse
  click there, while a swipe emits directional pad steps. `App` forwards the
  pointer position, relative motion, and mouse-button state through the game's
  input block. This proves that GLUE forwards pointer/click data, not that the
  phone movie turns the point into a slot.
- The decompiled script's ordinary path detects frontend accept and asks the
  movie for its existing `GET_CURRENT_SELECTION`. Together with the reported
  symptom (tap opens the preselected Contacts app), this is consistent with
  the click acting as accept while the movie selection stays unchanged. The
  exact engine dispatch still needs runtime trace; the tap path does not
  currently provide an app-slot value to the script.
- Therefore the next direct-touch patch needs an explicit coordinate-to-slot
  hit-test plus a narrow way to set/return that slot through the movie/script
  selection path. Swiping through directional steps is navigation only and is
  not an acceptable final substitute.

### Android runtime evidence (build `eeb689c`, 2026-10-31)

- The user ran the build with Phone touchscreen input on and Bigger phone off.
  While the phone was published as out, the input-device trace changed to
  keyboard/mouse and `pos_z` stayed at `-91.5`; this run did not exercise the
  enlargement override.
- The movie-method trace contains `SET_INPUT_EVENT(1..4)`, several
  `GET_CURRENT_SELECTION` calls, and home-page `DISPLAY_VIEW(1, n)` calls with
  different selection values. This confirms the existing navigation and
  selection/confirm path ran. The trace does **not** log the integer returned
  by `GET_CURRENT_SELECTION`, the touch point, or which gesture preceded each
  call, so it cannot establish whether direct taps opened the icon under the
  finger. The user's description says multiple apps were selected, but the
  gesture/result still needs clarification.
- The same report ends with a Vulkan GPU-device-lost failure on Mali-G78. Its
  renderer snapshot was not sampled, so this is a separate unresolved graphics
  failure, not evidence for or against phone hit-testing. Keep it separate from
  the selection diagnosis.

This report closes the need to obtain/decompile these two files again. It does
not yet identify how the game dispatches the GLUE touch click into its frontend
accept path, the writer of the special flags, or the exact safe binary
patch/loader boundary. The exact phone GFXs are already received; no more phone
assets are needed for this next analysis.

## Phone position patch warning

The old 1.25x phone enlargement rewrote `CPhoneMgr`'s position from
`control_update` while the game rewrote its own swaying position every frame.
The renderer sampled either value and the phone appeared to open and close,
while the manager's state bytes stayed stable. The user confirmed that turning
**Bigger phone** off stops the cycle.

Durable lesson: a safe enlargement must happen at a single-writer boundary
(for example the script-native call that sets the phone position,
`SET_MOBILE_PHONE_POSITION`) or through a patched movie/script asset. Today's
per-function engine hooks do not automatically cover script natives.
`Settings::phone_enlarge` is therefore off by default and labelled experimental.

## General patch design checklist

Before writing a GTA patch:

1. Name the observable symptom and the exact user test that proves success.
2. Identify the owner of the state: engine code, a named script, a Scaleform
   movie, a data file, or GLUE's touch layer.
3. Find the narrowest boundary: file read, asset request, movie method,
   script-native return, control update, or input value.
4. Log first if ownership or timing is uncertain. Bound every trace and keep
   personal content, typed text, save contents, and precise finger coordinates
   out of reports.
5. Check every variant: character, mission/prologue, story/online, view/app,
   and data revision. A phone patch must cover all four movies above.
6. Preserve size and encoding unless the loader is proven to accept a change.
7. Make the patch conditional on a signature/hash/size and fail open.
8. Avoid per-frame writes that race the game. If a value must be maintained,
   find the game's own write or call and patch there once.
9. Add unit tests for pure transformations. Runtime memory patches need a
   device protocol in the PR.
10. State clearly what is proven, what is a hypothesis, and what still needs the
    user's device report.

## Evidence protocol for a phone test

Use Bigger phone **off** and Phone touchscreen input **on** unless the test is
specifically about Bigger phone. Ask the user to:

1. open the phone;
2. swipe once on the phone screen;
3. tap an app other than the pre-selected one;
4. press Back;
5. copy the Import report immediately.

Read, in order:

- touch trace: what GLUE emitted (but remember directional steps there can be
  swipes on the phone screen, not touches on the side D-pad keys);
- phone/input-device trace: `CPhoneMgr` bytes, published phone state, device,
  and `pos_z`;
- phone movie methods: the script's actual `SET_INPUT_EVENT`, `DISPLAY_VIEW`,
  and `GET_CURRENT_SELECTION` sequence.

Do not interpret a directional pulse as use of the layout's phone keys without
asking; the touch layer converts a swipe on the phone into directional steps.
