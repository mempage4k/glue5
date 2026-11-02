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
- [Windows streaming server and audio mirror audit](windows-stream-audio-audit.md): a supplied Windows audit snapshot, repository streaming behavior, and open questions for optional character-voice downloads.
- [Streaming flow map](streaming-flow.md): source map from server announcement and discovery to client cache and engine reads; includes the current Android-only support note.
- [Project diary](project-diary.md): temporary chronological notes; not the source of truth for durable design decisions or backlogs.
