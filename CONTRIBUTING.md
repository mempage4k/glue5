# Contributing to GLUE 5

Thanks for helping. Bug reports, fixes, ports and documentation are all
welcome. This page says what to keep in mind so a change can go in quickly.

## The one rule: no game files

GLUE 5 contains none of the game's code or assets, and it must stay that
way. Never commit, attach or link `game.wasm`, the game's data, shaders or
anything extracted from them, in the code, an issue or a pull request. This
also goes for logs and screenshots: check them for personal information,
such as your user name in a file path. Bundled media whose provenance is
under review is listed in [Asset provenance](docs/asset-provenance.md).

## Reporting a bug

GitHub Issues are disabled in this repository. Record bugs in the Linear
project **GLUE 5 — GTA V para Android** (team `GLU`), with:

- the device, its OS version and GPU (on Android, the phone model and its
  chip);
- how you got the app (a GitHub build, a build from source and its commit);
- what you did, what happened and what you expected;
- the logs: `game.log` and the app's log from the
  [user folder](docs/getting-started.md#where-your-data-is) (on Android,
  `adb logcat` around the problem).

Keep **implemented**, **tested** and **validated on the device** separate: a
green CI check is not device validation. For a security problem (for example
in online play or the Studio), don't record it publicly: report it privately
from the repository's **Security** tab.

## Project tracking and AI agents

Use the GLU team and `P-GLU-1` project in Linear for task scope, priorities, decisions, blockers, acceptance criteria and handoffs. Search for an existing issue before creating one, and keep its status and comments aligned with verified work. Use [the Linear workflow](docs/linear-workflow.md) for the detailed coordination and AI-identification rules. GitHub remains authoritative for code, branches, commits, pull requests and CI results.

## Making a change

1. Read [Building from source](docs/building.md) to set up the toolchain,
   and the [design notes](docs/design.md) for the part you are changing.
2. Branch from `main` — there is no `next` branch — and open the pull request
   against `main`. Integration into `main` is centralized and gated on the
   Android build: see [AI_GUIDE.md](AI_GUIDE.md). Each area works on its own
   branch; nothing is pushed, merged or published without explicit
   authorization. Shared files (`app.rs`, `ui.rs`, `ui/pages.rs`,
   `settings.rs`, `imports.rs`, `docs/project-diary.md`) need notice before
   edits.
3. Keep each commit to one change, with a subject that says what it does in
   plain words, prefixed by the area: `Renderer: ...`, `Android: ...`,
   `Docs: ...` (`git log --oneline` shows plenty). Commits containing
   AI-produced material carry the `AI-Contribution:` trailer defined in the
   [identification protocol](docs/protocolo-identificacao.md).
4. Before opening it, run `mise exec -- cargo clippy` (no new warnings) and
   `mise run test` (`cargo test --workspace`), and build every platform your
   change touches. CI does **not** run the whole workspace: see
   [CI coverage](docs/ci-coverage.md). For a renderer change, compare
   screenshots before and after (`mise run headless -- --help`: `--shots`).

## Support scope

This release targets only the Samsung Galaxy S21 Plus (arm64, Android 11 or
later). Historical build tools and documentation for other platforms remain
in the repository, but they are not supported or validated.

## How the code is written

- Fix the cause, not the symptom. No special cases for one device, timing
  guesses or sleeps that hide a race: if the right fix is unclear, say so in
  the issue or pull request.
- Match the code around your change: its naming, its idioms and how much it
  comments. Comments say why, not what.
- Keep platform specifics where they belong (`platform/`, `cfg` blocks).
  No personal names, addresses or local paths in code, tests or commits.
- Changes to wgpu, wasm2c and the other patched dependencies go in
  [`patches/`](patches/README.md), each patch with a message that explains
  it.

## License

Unless you explicitly state otherwise, any contribution you intentionally
submit for inclusion in GLUE 5 is licensed as described in the
[README](README.md#license), without any additional terms or conditions.

Everyone taking part is expected to follow the
[code of conduct](CODE_OF_CONDUCT.md).
