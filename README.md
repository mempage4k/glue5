<p align="center">
  <b>GTA V, running natively on Android.</b><br>
  GLUE 5 targets the Samsung Galaxy S21 Plus: no emulator, no JIT, nothing rendered in the cloud.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Android-3DDC84?style=for-the-badge&amp;logo=android&amp;logoColor=white" alt="Android">
</p>

<p align="center">
  <a href="https://github.com/mempage4k/glue5/stargazers"><img src="https://img.shields.io/github/stars/mempage4k/glue5?style=for-the-badge&amp;logo=github&amp;color=yellow" alt="GitHub stars"></a>
  <a href="https://github.com/mempage4k/glue5/forks"><img src="https://img.shields.io/github/forks/mempage4k/glue5?style=for-the-badge&amp;logo=github" alt="GitHub forks"></a>
  <a href="https://github.com/mempage4k/glue5/commits/next"><img src="https://img.shields.io/github/last-commit/mempage4k/glue5/next?style=for-the-badge&amp;logo=github" alt="Last commit"></a>
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&amp;logo=rust&amp;logoColor=white" alt="Rust">
</p>

> [!IMPORTANT]
> GLUE 5 contains **no game files**. It turns files you supply, from GTA V's
> **WebAssembly build** (`game.wasm`, its translated shaders and its prepared
> data), into an app for your device. Files from a regular PC or console
> installation are not enough. GLUE 5 is not affiliated with Rockstar Games
> or Take-Two Interactive.

> **Support scope:** this release targets only the Samsung Galaxy S21 Plus (arm64,
> Android 11 or later). Other devices and operating systems are not officially
> supported or validated. Historical build tools and documentation for other
> platforms remain in the repository, but do not represent current support.

## Highlights

- **Native Android build.** The WebAssembly game is compiled ahead of time
  for the Galaxy S21 Plus's arm64 CPU and rendered with Vulkan. FSR 2
  upscaling where supported, with frame pacing matched to the display.
- **Touch controls designed for the S21 Plus.** Choose the Five Star touch
  profile or the existing controls, with haptics, aim assist and the game's
  button prompts; gamepads and keyboards are supported too.
- **GTA Online, without servers.** Freemode sessions between GLUE 5
  players, found automatically on the local network or over the Internet.
  Friends, invites and joins are end-to-end encrypted.
- **Your game files, on Android.** Keep the game on a computer and stream
  it to the Galaxy S21 Plus, or copy it once and play offline.
- **Mods in Luau, with a Studio.** Write mods in a browser, with the game's
  6,000+ native functions typed and documented. Menus work with touch and
  controllers; coding agents can drive the Studio too. Glue Trainer comes
  with it.
- **Memory options.** Auto chooses a low-memory profile from reported RAM;
  Low and Normal are available as manual overrides.

## Supported target

| Device | Status | Graphics | Requirements |
|---|---|---|---|
| **Samsung Galaxy S21 Plus** | 🎯 Sole official target; device validation pending | Vulkan | arm64-v8a, Android 11 or later. Other device/platform builds are unsupported and unvalidated. |

## Get started

1. **Have your game files.** GTA V's WebAssembly build: `game.wasm`, its
   `shaders` folder and its `data` folder.
2. **Build the app.** The game's code ends up inside the app, so nobody
   can hand you a ready-made one: you build your own. The easiest way
   needs only a GitHub account: [build the Android app on GitHub](docs/github-build.md),
   free, from your own `game.wasm`, which stays private. With a computer
   set up for development, [build it yourself](docs/building.md).
3. **Install it and add the files.** [Getting started](docs/getting-started.md)
   covers installation and game files for the Galaxy S21 Plus.

## Documentation

| | |
|---|---|
| [Getting started](docs/getting-started.md) | Install the app and add the game's files on the Galaxy S21 Plus. |
| [Playing](docs/playing.md) | Game modes, controls, settings, performance and troubleshooting. |
| [GTA Online and friends](docs/online.md) | Sessions, friends and invites without servers, and how they stay private. |
| [Streaming the game's files](docs/streaming.md) | Play from a computer's copy on the Galaxy S21 Plus. |
| [Mods and GLUE 5 Studio](docs/mods.md) | Write mods in Luau, in your browser or with a coding agent. |
| [Build the Android app on GitHub](docs/github-build.md) | Build from your own `game.wasm`, with no development setup. |
| [Building from source](docs/building.md) | Toolchain, tasks, platforms, options and the repository's layout. |
| [Design notes](docs/design.md) | How it works inside: the recompiler, the renderer, streaming, mods. |

## FAQ

<details>
<summary><b>Is the game included?</b></summary>

No. GLUE 5 is the recompiler and the runtime around the game. You supply
the game's files, from its WebAssembly build.
</details>

<details>
<summary><b>Why can't I just download the app?</b></summary>

The app is made from the game's own code (`game.wasm`), compiled for your
device. An app built from it contains the game, so it is never shared:
everyone builds their own, from their own files. GitHub can do it for you
in your account ([how](docs/github-build.md)).
</details>

<details>
<summary><b>Will it run on my phone?</b></summary>

This release targets only the Samsung Galaxy S21 Plus (arm64, Android 11 or
later); other devices are not officially supported or validated. The memory
profile defaults to Auto, with Low and Normal available in Settings. For
performance expectations and troubleshooting, see
[Performance](docs/playing.md#performance).
</details>

<details>
<summary><b>Is GTA Online the real one?</b></summary>

It is the game's own freemode, played between GLUE 5 players. Nothing
connects to Rockstar's services: sessions are hosted by the players, and
characters are kept on the device. See [GTA Online and friends](docs/online.md).
</details>

<details>
<summary><b>Do I need a JIT, a jailbreak or developer mode?</b></summary>

No. The game is compiled before installation and runs natively in the
Android app. Install the APK on the supported device; no root, jailbreak or
JIT is required.
</details>

## Contributing

Bug reports and fixes for the supported Android target are welcome: read
[CONTRIBUTING.md](CONTRIBUTING.md) first, and follow the
[code of conduct](CODE_OF_CONDUCT.md). Historical multi-platform tools and
notes are retained but are outside the current support scope.

For project coordination, contributors and coding agents use [Linear](docs/linear-workflow.md)
to track task scope, priorities, decisions, blockers, status and handoffs.
GitHub remains the source of truth for repository files, branches, commits,
pull requests and CI results.

## License

GLUE 5 is free software: you can redistribute it and modify it under the
terms of the [GNU General Public License](LICENSE), version 3 or (at your
option) any later version.

The patches in [`patches/`](patches/README.md) change other projects and
are offered under each project's own license, so they can go upstream.
The fonts and touch icons in `crates/glue5/assets` keep their licenses
([OFL](crates/glue5/assets/ui/OFL.txt),
[icons](crates/glue5/assets/touch/README.md)). The game itself is not part
of GLUE 5 and is covered by none of this.

## Acknowledgements

GLUE 5 is built on [wasm2c](https://github.com/WebAssembly/wabt/tree/main/wasm2c),
[wgpu](https://wgpu.rs), [SDL](https://libsdl.org), [egui](https://github.com/emilk/egui),
[Luau](https://luau.org) with [mlua](https://github.com/mlua-rs/mlua),
[Leptos](https://leptos.dev), [CodeMirror](https://codemirror.net),
[snow](https://github.com/mcginty/snow), [mise](https://mise.jdx.dev) and
many other open-source projects: the app's Credits page lists them all,
with their licenses.

Grand Theft Auto and GTA are trademarks of Take-Two Interactive Software.
GLUE 5 is an independent project, not endorsed by or affiliated with
Rockstar Games or Take-Two Interactive.
