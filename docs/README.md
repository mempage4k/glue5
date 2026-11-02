# GLUE 5 documentation

**Playing**

- [Getting started](getting-started.md): install the app and put the game's files on each device.
- [Playing](playing.md): game modes, controls, settings, performance and troubleshooting.
- [GTA Online and friends](online.md): sessions, friends and invites without servers.
- [Streaming the game's files](streaming.md): play from a computer's copy on your other devices.
- [Mods and GLUE 5 Studio](mods.md): write mods in Luau, in your browser or with a coding agent.

**Building**

- [Build on GitHub](github-build.md): the app from your `game.wasm`, with no setup.
- [Building from source](building.md): toolchain, tasks, platforms, options and layout.
- [Design notes](design.md): stable architecture and design decisions.

**Engineering investigations**

- [Android performance: Galaxy S21 Plus](android-performance-s21.md): device-specific hypotheses, verification gates and measurements; static review only until validated on hardware.
- [Touch, HUD and benchmark roadmap](roadmap-touch-hud-benchmarks.md): scoped implementation backlog and progress for touch/HUD and GLUE 5 benchmarks.
- [Project diary](project-diary.md): temporary chronological notes; not the source of truth for durable design decisions or backlogs.

**Conventions and governance**

- [Identification and collaboration protocol](protocolo-identificacao.md): the official rules for AI contributions, mirrored from the `[MENU]` branch so agents can read them without Linear access.
- [CI coverage](ci-coverage.md): what each GitHub Actions workflow actually tests — and what it does not.
- [Asset provenance](asset-provenance.md): bundled media under provenance review; decisions pending with the owner.
- [Integration status](integration-status.md): which branch holds what, shared-file ownership, known conflicts and the Android build gate.
- [Contributing](../CONTRIBUTING.md): tracker, branches, tests and commit rules.
- [AI_GUIDE.md](../AI_GUIDE.md): guardrails for agents, including the Android build gate.

**Documentation living on specialist branches (not in `main`)**

Seven reference documents are cited by the Linear project as if they were
part of the repository, but they do not exist in `main`. The `[MENU]` area
keeps them on `arena/4f5cc306-glue5` until the owner approves publishing
them; its own detailed index is `docs/documentacao-branch-menu.md` on that
branch (local commit `84befb0`, not pushed). Do not treat these as a source
of truth for `main` until they land there.

| Document | Where it lives | Status |
|---|---|---|
| `gta5-reference-inventory.md` | `arena/4f5cc306-glue5` `[MENU]` | Branch-only |
| `gta5-visual-parity.md` | `arena/4f5cc306-glue5` `[MENU]` | Branch-only |
| `font-extraction.md` | `arena/4f5cc306-glue5` `[MENU]` | Branch-only |
| `rpf-awc-reader.md` | `arena/4f5cc306-glue5` `[MENU]` | Branch-only |
| `shell-notifications.md` | `arena/4f5cc306-glue5` `[MENU]` | Branch-only |
| `prompt-chatgpt-2026-11-01-ysc-full.md` | `arena/4f5cc306-glue5` `[MENU]` | Branch-only transcript, not a design document |
| `protocolo-identificacao.md` | `arena/4f5cc306-glue5` `[MENU]` | **Mirrored verbatim in this branch** (from `a2e5bfe`) |
| `gta5-referencia/` (7 CSVs) | `arena/4f5cc306-glue5` `[MENU]` | Branch-only measurement data |
| `gta-patches.md` | `arena/107599ac-glue5` (PR #12) `[TOUCH]` | Branch-only; linked from that branch's `docs/README.md` |
| `validation-s21.md` | `[PERF]`, `arena/b8d177aa-glue5` per Linear comments | **Not published**: that branch is absent from the remote |
| `documentacao-branch-menu.md` | `arena/4f5cc306-glue5` `[MENU]` | Local commit `84befb0`, **not pushed**; the detailed index of the branch-only docs |
| `streaming-flow.md` | `arena/57946d80-glue5` `[SERV]` | Branch absent from the remote; changes not committed |
| `windows-stream-audio-audit.md` | `arena/57946d80-glue5` `[SERV]` | Windows snapshot supplied by another chat, **not verified** by that agent |
| `validation-s21.md` | `[PERF]`, local commit `710d364` | **Not published**: `arena/b8d177aa-glue5` is absent from the remote |

`menu-sfx.md` and `project-diary.md` do exist in `main`, but the `[MENU]`
branch carries different versions; see its index before merging either.

**Coordination warning:** `docs/README.md` itself now has three authors
(`main`, `[SERV]` and general review), plus the version carried by PR #12.
`docs/project-diary.md` has `[MENU]`, `[PERF]` and `[SERV]` bullets. Both are
append-on-append conflicts at integration time; see
[Integration status](integration-status.md). The area code chosen by the
server/streaming chat is `[SERV]`, not the `[STREAM]` proposed in the
delegation messages.
