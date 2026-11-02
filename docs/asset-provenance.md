# Asset provenance

`AI_GUIDE.md` and `CONTRIBUTING.md` forbid committing files extracted from
the game. Two bundled media items need an explicit decision from the owner.
This page records what is actually in the repository; it does **not** remove
anything.

## `crates/glue5/assets/audio/pausemenu.mp3`

- Size: 25,769,324 bytes. Embedded with
  `include_bytes!("../assets/audio/pausemenu.mp3")` at
  `crates/glue5/src/audio.rs:83` and streamed by SDL3_mixer.
- Provenance per `crates/glue5/assets/audio/README.md`: converted from a
  **user-provided** `pausemenu.m4a` (AAC-LC, 48 kHz stereo, ~21:28) to
  48 kHz stereo MP3 at 160 kbit/s; the original stays in Google Drive.
- `docs/menu-sfx.md` acknowledges the track is in the app, while stating
  that interface sounds are **not** shipped and must be read at run time.
- Open question: is the source M4A an original/recording supplied by the
  owner, or a track extracted from the game's audio archives? The first case
  is compatible with the repository's rules if the owner confirms the
  licence; the second is not.
- Options for the owner: (a) confirm provenance/licence and document it
  here; (b) keep the music out of the binary and load it at run time like
  `sfx/` and `scenes/`; (c) replace it with an unambiguously licensed track.

## `branding&art.zip`

- Size: 19,494,727 bytes; 48 entries; 19,564,081 bytes uncompressed.
- `art/`: 43 images — `beach_*` and `ls0`–`ls16` background/foreground
  `.webp` layers, `beach_bg.stock.png`, `beach_fg.stock.png`,
  `beach_stock.jpg` (stock-photo sources), and character foregrounds such as
  `ls12_foreground_michael.webp`, `ls1_foreground_franklin.webp` and
  `ls2_foreground_chop.webp`.
- `chalet.woff` (26,952 bytes): the commercial Chalet font. GLU-14 states it
  is never to be committed.
- Audio: `loading-music-full.m4a` (10,172,702 bytes),
  `loading-music-loop.m4a` (1,402,354 bytes) and `loading-music.m4a`
  (1,470,601 bytes).
- Also `logo.png` (90,239 bytes) and `spinner.png` (3,301 bytes).

The zip is a raw transfer archive from the Drive/PC workspace, not a set of
runtime assets with recorded licences. No image, font or music file from it
is referenced by a build script as far as a static search shows; the
`art/` layers are candidate design references. `chalet.woff` and the three
M4A tracks are the highest-risk entries.

## Recommendation (no action taken)

1. Keep the raw archive and its extracted counterparts in Drive/PC, outside
   the repository, following the same rule as the 20 split ZIPs (~0.94 GB).
2. Record per-file provenance/licence before any asset is referenced at run
   time; never commit `chalet.woff` (GLU-14) or game-derived audio.
3. Any removal from Git history is a separate, owner-authorized operation —
   it rewrites history and affects every specialist branch.

Related: `docs/menu-sfx.md`, `crates/glue5/assets/touch/README.md` and
`crates/glue5/assets/ui/README.md` already record licences for icons
(CC BY 3.0 / MIT) and fonts (OFL). Those are the pattern to follow.
