[IA: Arena.ai | Agente/cliente: Arena.ai (Agent Mode — chat de streaming/servidor)]

# [SERV] Windows streaming server and audio mirror audit

**Audit snapshot:** 2026-11-01, supplied by the user from a separate Windows-capable chat.
**Evidence boundary:** The Windows observations below are transcribed and summarized from that supplied audit; this Arena session did not access the Windows machine and has not independently verified the installation, files, executable, or hashes. Repository behavior is checked against the current checkout where noted.
**Privacy and assets:** Machine/user names and absolute Windows paths are intentionally omitted. No proprietary game assets are included.

## Findings from the supplied Windows audit

- The installation reportedly has two separate services:
  - Native `glue5-serve.exe`, the GLUE 5 streaming protocol on TCP `47100`.
  - `serve_local.py`, a static PlayGTA5/WebAssembly HTTP mirror, reportedly on localhost `8000` or LAN `8080`. It is not the native GLUE protocol.
- The native server is launched with a mirror root containing `data/` and `b/<build>/shaders/`. Its launcher reportedly checks TCP `47100` before starting the server. The audit also reports a firewall helper for the native port, but its effective Windows firewall state was not verified.
- The audit could not identify the installed executable's source commit or exact version. The repository's `0.1.0` version is not evidence that the Windows executable was built from the current source.
- At the time of the Windows audit, it reported no running `glue5-serve.exe` process and no listener on TCP `47100`.
- The audio mirror reportedly contains 54 `.rpf` files under `data/x64/audio/sfx/` (about 16.66 GiB), three RPFs directly under `data/x64/audio/`, configuration files, and 12 `.awc` tracks with matching JSON files.
- The audit reports that the current mirror's sample RPF hashes match a processed audio set but differ from samples in a separately inspected PC game folder. A full 54-file hash comparison against the original source remains outstanding. The mirror should therefore not be described as a byte-for-byte copy of that inspected PC folder.

These are observations about one local installation, not a general installation recipe or proof of compatibility with every game build.

## Repository behavior checked in this checkout

- `crates/glue5-stream/src/main.rs` implements `glue5-serve`; the default port is `47100`, and the CLI accepts a game directory, optional build, cache, bind address, port, and rate limit.
- `crates/glue5-stream/src/catalog.rs` recursively catalogs files under `data/`, excluding only `manifest.json`, `bootset.json`, and `bootset_low.json`; it also catalogs the selected build's shaders. It has no audio-specific filter.
- `crates/glue5-stream/src/server.rs` serves the catalog entries. Audio archives are ordinary files/ranges when present in the selected `data/` tree.
- `crates/glue5-stream/src/pack.rs` packs small text-like files and shaders; large RPF archives are transferred as ordinary file data, not as a special speech pack.
- `tools/fill_audio.py` replaces/adds every `.rpf` directly inside a PC install's `x64/audio/sfx/` to a web build's matching directory. It has no speech-only selection and does not cover all audio paths. Existing targets of equal size are skipped without a hash comparison.
- There is no voice-only manifest, protocol endpoint, app setting, or independent character-dialogue download operation in the reviewed code.

Therefore the Windows mirror reportedly has audio archives available to the generic server, but neither the Windows launcher nor the current GLUE app provides a selective “download character voices” feature. The game's normal streaming requests files as needed; this is distinct from a user-controlled selective download.

## Speech-bank evidence and limitations

The supplied audit cites a community speech-to-subtitle map as evidence that detected English speech clips occur in banks including `S_FULL_AMB_M.rpf`, `S_MINI_GAN.rpf`, `S_MINI_SER.rpf`, `S_MISC.rpf`, `POLICE_SCANNER.rpf`, `SS_FF.rpf`, `SS_AC.rpf`, `SCRIPT.rpf`, and `RESIDENT.rpf`. It describes `SS_FF` and `SS_AC` as story-cutscene banks. This is community-derived, English-only evidence, not an official classification or proof that a whole archive contains only dialogue.

The audit also notes that radio talk/news/adverts are spoken broadcasts but distinct from character dialogue; music/score, ambience, effects, pain vocals, and mixed banks should not automatically be included in a “character voices” option. Filenames alone are insufficient to define a safe selective package. The `.awc` tracks and their dependencies need a separate usage check.

## Work still needed for optional character voices

1. Define the desired scope: ambient character lines, mission/script dialogue, story cutscenes, and whether police scanner or talk radio are included.
2. Verify the exact banks/files and dependencies for the web build served by the mirror. Build a build-specific, versioned inventory; use hashes/sizes for integrity, but do not add game assets or extracted proprietary manifests to this repository.
3. Design an opt-in transfer path distinct from **Download the whole game**, with progress, retry, integrity validation, storage estimate, offline/partial-download behavior, and compatibility handling.
4. Test downloading voices after the game is already installed, interruption/retry, server/build changes, missing-bank fallback, and actual mission dialogue on the target Android device.

## Related implementation and documentation

- [Streaming the game's files](streaming.md)
- `crates/glue5-stream/src/{main,server,catalog,pack,client,cache}.rs`
- `tools/fill_audio.py`

This audit document records the supplied report and repository facts. It does not claim that the reported Windows setup has been independently validated in this session.
