# AI guide for GLUE 5

This file is the durable onboarding note for coding agents working in this repository. Read it at the start of every new Arena chat; then read the project docs relevant to the assigned task. The checked-out code, Git history, and focused docs are the source of truth for the current implementation—do not assume another chat's changes are present in this branch.

## What this repository is

GLUE 5 is a Rust application that recompiles the game's WebAssembly build into native code and provides the runtime around it. It is not an emulator, does not stream rendered gameplay from a cloud server, and does not include or distribute GTA V game files.

The app uses SDL for platform, window, input and audio services; egui for much of its UI; and wgpu for graphics. Read [the design notes](docs/design.md) before changing boundaries between these systems.

## Repository map

| Path | Responsibility | Start here when assigned… |
|---|---|---|
| `crates/glue5/` | Main application, settings, UI, touch controls and platform-facing app logic | UI, touch, menus, settings, app lifecycle |
| `crates/glue5/src/touch.rs` and its tests | On-screen controls, touch hit areas and touch-specific behavior | Phone and touchscreen controls |
| `crates/glue5-runtime/` | Runtime services and the game's input/host bridge | Input semantics or runtime behavior |
| `crates/glue5-gpu/` | GPU devices, render/present pipeline and graphics overlays | Rendering, HUD composition, GPU presentation |
| `crates/glue5-game/` | Game module bindings and build-time translation | Game ABI, imports or generated bindings; investigate carefully |
| `crates/glue5-stream/` | Game-file streaming protocol and server | Streaming or file transfer |
| `crates/glue5-mods/`, `crates/glue5-studio/` | Mod runtime and its Studio | Mods, scripting or Studio |
| `platform/android/` | Android activity, Gradle packaging and native integration | Android-only packaging or OS integration |
| `patches/` | Patches to vendored/upstream dependencies | Dependency internals; read `patches/README.md` first |
| `docs/` | User docs, design notes, investigations and scoped backlogs | Documentation and task-specific project history |

`docs/README.md` is the documentation index. `docs/building.md` explains the toolchain and commands; `docs/design.md` explains architecture and constraints.

## Linear and GitHub coordination

Read [`docs/linear-workflow.md`](docs/linear-workflow.md) before beginning project work. Linear is the GLUE 5 work tracker: use it for task scope, priority, ownership, status, acceptance criteria, decisions, blockers, and handoffs. GitHub is the implementation source of truth: verify code, branches, commits, pull requests, workflow definitions, and CI results there. A Linear status or comment never proves that a change exists on the current branch or has passed CI.

Before starting, search the GLU team and `P-GLU-1` project for the relevant issue, read its full description/comments/resources, and confirm the checked-out branch contains the expected code. Update the issue when work starts, when scope or blockers change, and when handing work off. Create a new `[AREA]` issue only when no matching actionable issue exists. Mark work Done only when its acceptance criteria and required validation are actually complete; leave owner-run Android/device validation visibly pending.

Every AI-authored Linear description, comment, or status update must begin with `[IA: <modelo> | Agente/cliente: <ferramenta e contexto>]`. AI-authored material included in a commit must carry the project-required `AI-Contribution: <modelo> | Agente/cliente: <ferramenta>` trailer while preserving Git author/committer fields. The current codename is `willow`; follow the codename change rules in the Linear project guidance and `docs/linear-workflow.md`.

## Working model for Arena's per-chat branches

Arena may give each chat its own branch. Treat that as intentional isolation, not as a shared working tree:

1. **One chat, one bounded responsibility.** A specialist should own a small set of files and avoid editing another specialist's domain as a side effect.
2. **Do not rename or switch away from the branch assigned to the chat.** Work only on that session's branch. Never push to `main` or another agent's branch.
3. **Do not assume changes transfer between chats.** Tell the next agent which branch/commit contains work and whether it was pushed; otherwise, re-check the current checkout and relevant docs.
4. **Keep integration centralized.** A designated coordinator reviews specialist work and integrates it only after the owner approves. Do not merge, cherry-pick, rebase, or open a PR just because another agent finished; ask the owner or follow an explicit integration task.
5. **Avoid concurrent edits to shared files.** In particular, agree on ownership before multiple agents touch `ui.rs`, settings, manifests, shared input types or GPU presentation.

A useful pattern is: specialist branch → review and validation → owner chooses whether to publish → coordinator integrates. A chat's commit is not automatically a release or a device-tested build.

## Start-of-task checklist

Before editing:

- Read `git status --short --branch`, `git log -5 --oneline`, and the relevant `git diff`. Preserve pre-existing changes, including untracked files. Never use broad staging such as `git add .` or `git add -A`.
- Read this guide and only the focused docs needed for the assigned work. Do not turn an unrelated Markdown file into a general project diary; give each document a clear purpose and commit project records to the repository.
- State the intended scope, identify the owning files, and inspect how the current code behaves before changing it.
- Prefer a small, testable change. Keep rendering, touch hitboxes, input routing and saved settings consistent when they represent the same control or geometry.

When done, report: what changed, exact files, tests/builds run and not run, device validation still needed, and whether the work was committed or published. Do not claim a test, Android workflow, or device check passed unless it actually ran and its result is known.

## Suggested specialist roles

Create a separate chat when a task has a distinct owner and file boundary. The first proposed specialist is **GTA V phone and touchscreen controls**; other roles can be added as the work grows:

- **GTA V phone and touchscreen controls** — phone interaction, virtual controls, touch aiming, control editing and touch-specific haptics.
- **Menu/UI and accessibility** — shell, pages, safe-area layout, keyboard/controller navigation; avoid changing gameplay touch behavior unless assigned.
- **Rendering/GPU** — GPU passes, scene composition, presentation and performance; avoid changing game controls.
- **Android platform/build** — Android activity, packaging, permissions and platform-specific integration; do not alter shared behavior casually.
- **Online/streaming/mods** — one independent domain per chat where possible.
- **Integration coordinator** — review ownership, test results and conflicts; does not silently absorb unrelated features.

### First specialist: GTA V phone and touchscreen

Start by reading [`docs/roadmap-touch-hud-benchmarks.md`](docs/roadmap-touch-hud-benchmarks.md), `docs/playing.md`, `crates/glue5/src/touch.rs`, its tests, and the settings/UI code that the task actually needs. Verify the current branch before relying on any recorded progress: some changes may exist only on another Arena branch or may not yet have been validated on a device.

The touch backlog includes direct tapping on the visible GTA V phone/app and its on-screen controls, stopping repeated open/close actions, making drive-by aiming usable while shooting, and an editor for moving/resizing controls with persisted positions and validated/versioned XML presets. On touch devices, prioritize tapping a visible target over requiring Select/directional gestures. If the phone is enlarged for touch, keep its hitbox aligned and do not enlarge it for non-touch players. Haptics on Android are a priority to consider.

Keep one source of geometry for drawing, hit-testing and occupied areas. Add focused regression tests for the applicable control profiles and input modes. Do not change the 3D engine, GPU passes or unrelated controls as a shortcut; raise a separate task if the behavior needs those boundaries changed. Update the touch roadmap only with relevant progress and clearly distinguish implemented, tested, device-validated and pending work.

Suggested first-task prompt:

> You own GLUE 5's GTA V phone and touchscreen controls. Read `AI_GUIDE.md` and `docs/roadmap-touch-hud-benchmarks.md`, inspect the current branch and preserve unrelated changes. First map the existing phone and touch-input path, then implement only the agreed phone/touch scope with focused tests. Keep visual geometry, hitboxes and persisted settings aligned. Do not alter engine or GPU rendering behavior, do not publish or integrate without explicit approval, and report anything that still needs the owner's Android build/device validation.

## Project-specific guardrails

- **Android is the current validation priority.** The user runs the Android workflow manually. Do not trigger it on their behalf or claim success without a result. The current integration gate is to wait for the user to share that Build Android result before integrating to `main`.
- **Preserve version identity.** SemVer remains `0.1.0` unless the owner asks otherwise. Change the codename only for a meaningful change that needs a distinct build identity, not for retries. New Alpha builds should show a small version label over GLUE 5 and the game.
- **No Apple feature work unless requested.** The owner has authorized removing Apple support, but that is a dedicated scope—not permission to remove or rewrite platform code during an unrelated task.
- **Do not use AI-generated artwork as a UI placeholder.** Prefer a deliberate code-drawn fallback. Do not change the engine or touch controls for a UI-only task.
- **Respect privacy and device-debugging limits.** For freezes, do not ask for screenshots as the investigation method. Keep logs limited to the GLUE 5 process PID. Shizuku requires explicit authorization.
- **Publication is explicit and scoped.** A request to edit or commit does not automatically authorize pushing, opening a PR, merging or releasing. Stage only the files for the task, state clearly whether they were published, and never force-push. The owner runs the Android workflow.
- **Do not assume input assets exist.** Check the workspace and the shared `glue5` Google Drive folder when relevant; never claim to have an RPF or other asset that was not provided.

## Validation

Use the commands and platform requirements in [`docs/building.md`](docs/building.md). The repository's normal test entry point is `mise run test`; setup may be required first. For a focused Rust test, use the repository's configured toolchain and the owning crate's test target. Android packaging/device validation is a separate gate from unit tests. Record unavailable tools and unrun checks honestly.
