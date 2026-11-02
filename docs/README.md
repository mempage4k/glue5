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

**Project coordination**

- [Linear workflow](linear-workflow.md): how contributors and AI agents use Linear for task tracking while GitHub remains the implementation source of truth.

**Engineering investigations**

- [Android performance: Galaxy S21 Plus](android-performance-s21.md): device-specific hypotheses, verification gates and measurements; static review only until validated on hardware.
- [Validation on the S21 Plus](validation-s21.md): what a build has to prove on the device, in order — the thermal back-off, the phone preset, the PGO round and the frame rate goal.
- [Touch, HUD and benchmark roadmap](roadmap-touch-hud-benchmarks.md): scoped implementation backlog and progress for touch/HUD and GLUE 5 benchmarks.
- [GTA V runtime patches](gta-patches.md): durable rules, evidence, patch boundaries, and open questions for future GTA patch work.
- [Windows streaming server and audio mirror audit](windows-stream-audio-audit.md): a supplied Windows audit snapshot, repository streaming behavior, and open questions for optional character-voice downloads.
- [Streaming flow map](streaming-flow.md): source map from server announcement and discovery to client cache and engine reads; includes the current Android-only support note.
- [Project diary](project-diary.md): temporary chronological notes; not the source of truth for durable design decisions or backlogs.

**Conventions and governance**

- [Identification and collaboration protocol](protocolo-identificacao.md): the official rules for AI contributions, mirrored from the `[MENU]` branch so agents can read them without Linear access.
- [CI coverage](ci-coverage.md): what each GitHub Actions workflow actually tests — and what it does not.
- [Asset provenance](asset-provenance.md): bundled media under provenance review; decisions pending with the owner.
- [Integration status](integration-status.md): which branch holds what, shared-file ownership, known conflicts and the Android build gate.
- [Contributing](../CONTRIBUTING.md): tracker, branches, tests and commit rules.
- [AI_GUIDE.md](../AI_GUIDE.md): guardrails for agents, including the Android build gate.

**Documentation living on specialist branches (not in `main`; snapshot before PR #12)**

This table records the pre-integration state. `gta-patches.md` is the `[TOUCH]`
document contributed by PR #12 and linked above; its branch-only status below
is historical once that PR lands.

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
| `validation-s21.md` | `[PERF]`, `arena/b8d177aa-glue5` per Linear comments | **Published**: the branch is on the remote; the document lands in `main` with this PR |
| `documentacao-branch-menu.md` | `arena/4f5cc306-glue5` `[MENU]` | Local commit `84befb0`, **not pushed**; the detailed index of the branch-only docs |
| `streaming-flow.md` | `arena/57946d80-glue5` `[SERV]` | Branch absent from the remote; changes not committed |
| `windows-stream-audio-audit.md` | `arena/57946d80-glue5` `[SERV]` | Windows snapshot supplied by another chat, **not verified** by that agent |
| `validation-s21.md` | `[PERF]`, commit `19f751d` (the `710d364` of an earlier clone) | **Published**: the branch is on the remote; the document lands in `main` with this PR |

**[SERV] status update (2026-11-02):** The table above is a snapshot from before the server/streaming work was committed and pushed. `streaming-flow.md` and `windows-stream-audio-audit.md` are now tracked on `arena/57946d80-glue5` at `55badd4`; their links are listed above. The Windows audit remains a supplied snapshot, not independently verified by that chat. At the time of this update, the PR to `main` is pending.

`menu-sfx.md` and `project-diary.md` do exist in `main`, but the `[MENU]`
branch carries different versions; see its index before merging either.

**Coordination warning (pre-PR #12 snapshot):** `docs/README.md` itself now has three authors
(`main`, `[SERV]` and general review), plus the version carried by PR #12.
`docs/project-diary.md` has `[MENU]`, `[PERF]` and `[SERV]` bullets. Both were
append-on-append conflicts at integration time; see
[Integration status](integration-status.md). The area code chosen by the
server/streaming chat is `[SERV]`, not the `[STREAM]` proposed in the
delegation messages.

**[TOUCH] resolution in PR #12:** The README keeps the `main` index, the
`[TOUCH]` runtime-patch and roadmap entries, and the `[SERV]` links already
merged via PR #15. The diary is merged additively: its existing `main` text,
the `[SERV]` entry already landed via PR #15, and the `[TOUCH]` additions are
retained. No unpublished content from another specialist branch was imported.
