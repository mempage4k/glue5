# CI coverage

What each GitHub Actions workflow actually runs. This page exists because
"CI is green" has been used as evidence for claims the workflows cannot
prove — in particular for GLU-12 ("`cargo test` never ran") and for branches
that never had a test run at all.

## Matrix

| Workflow | Trigger | What it runs | What it does **not** run |
|---|---|---|---|
| [Build](../.github/workflows/build.yml) | `workflow_dispatch` only, manual; default platform `android` | Builds one platform's app from `game.wasm` in a private draft release, puts the result back into that draft, notifies ntfy. 360 min, `contents: write`. | No tests. Never runs automatically. |
| [GPU tests](../.github/workflows/gpu-tests.yml) | `push` filtered by paths (no branch filter) or `workflow_dispatch` | `mise exec -- cargo test -p glue5-gpu`. 360 min, `contents: write`, ntfy. | No APK build; no `glue5` (platform/UI/touch) tests; no `cargo test --workspace`. |
| [Android import tests](../.github/workflows/import-tests.yml) | `push: branches: [main]`, `pull_request: branches: [main]` (both path-filtered), or `workflow_dispatch` | Full Android arm64 + APK build, then `cargo test -p glue5 --lib imports::tests`, followed by per-filter runs of `cargo test -p <package> --lib <filter>`: `ui::`, `sfx::`, `touch::`, `synthetic_pointer`, plus three `glue5-gpu` filters (`present::tests::present_shaders_have_no_image_parameter_functions`, `present::tests::panel_scissor`, `overlay::tests::backdrop_blur`). Each filter fails if it matches zero tests. 120 min, `contents: write`, ntfy. | `cargo test --workspace`; anything outside those filters; pushes to specialist branches. |
| [Release guard](../.github/workflows/release-guard.yml) | release events | Reverts published releases containing a `wasm` or APK to draft. | No tests. |
| [ntfy test](../.github/workflows/ntfy-test.yml) | manual | Checks the notification script. | No tests of the project. |

## The gap

`mise run test` is `cargo test --workspace` (`.mise/tasks/test/_default`), and
`mise run test:shaders` validates every translated shader with naga. **No
workflow runs either of those.** A green check on a PR means the filtered
suites above passed on that PR's head.

`import-tests.yml` is restricted to `main` and PRs against `main`. Pushes to
`arena/*` branches therefore never trigger it. This matters for GLU-12:

- The `ui::` filter **does** cover `ui::popup::tests::*` (4 tests in
  `crates/glue5/src/ui/popup.rs`; ~49 `ui::` tests in total by grep). So the
  filter is not the problem.
- `arena/4f5cc306-glue5` (`[MENU]`) has **zero import-tests runs** — only
  Release guard runs and manual Builds (`ce99d1b` passed, `6aeb8b9` failed
  with E0015). Those are builds, not test suites. Its `ui::` tests have not
  been executed in CI.
- PR #12 (`arena/107599ac-glue5`) *did* get those checks, because it targets
  `main`.

So "the tests were never written" is wrong, and "CI covers them" is also
wrong: **the code exists, the filter would catch it, but no run happened on
that branch**. GLU-12 stays In Review until a run is produced.

## Closing a gap without guessing

`workflow_dispatch` exists on both GPU tests and Android import tests, but
`import-tests.yml` is a full 120-minute Android build that writes to the
repository and pages the owner over ntfy. Per `AI_GUIDE.md` it is reserved
for the owner; agents must not trigger it. To close GLU-12, either the owner
dispatches "Android import tests" on the branch, or the branch gets a PR
against `main`. Locally, with a toolchain:

```sh
git submodule update --init     # third_party/wabt (empty in a fresh clone)
mise run setup                  # patched wgpu/wabt/sp-fidelity dependencies
mise run test                   # cargo test --workspace
mise run test:shaders
cargo test -p glue5 --lib ui::  # the filter used by import-tests
```

A build also needs `game.wasm` from the private draft release (or
`GLUE5_GAME_DIR`); it is not in the repository. Sandboxes without `cargo`,
`mise` or `cmake` cannot run any of this — record that as "not executed",
never as "passed" or "does not compile".
