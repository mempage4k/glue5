# Reading the game's own files: the RPF/AWC reader

*Research, 2026-11-01. The piece the product vision needs: because every
install must choose its game first (a folder, or one served over the network),
the app can take the menu's assets — sounds first (`docs/menu-sfx.md`), then
fonts and imagery — out of the player's own installation at setup. This records
what exists to build it on, so the work is "assemble and port", not "invent".*

## Where the files are

A GLUE 5 installation keeps the game as loose files: `<game>/data/…` is the
engine's `/game` (see `crates/glue5-runtime/src/vfs.rs`). The retail layout
survives inside it — RPF archives included (`data/x64/a.rpf`, per the VFS and
stream tests) — and a streamed copy holds the same paths as sparse 4 KB
blocks (`crates/glue5-stream/src/cache.rs`). The frontend sound banks live at
`x64/audio/sfx/*.rpf` (`RESIDENT.rpf` and friends). Two consequences:

- Folder mode: plain file reads; nothing new needed to *reach* the archives.
- Stream mode: a file may be partially absent; the extractor must make sure
  the bank is whole (ask the stream layer) before reading it.

## The RPF half: solved, off the shelf

**`rpf-archive` 0.10** (crates.io; `github.com/VIRUXE/rpf-archive-rs`) is a Rust
reader/writer for RAGE archives: RPF0–RPF8 (RPF7 = GTA V) and IMG1–IMG3, with
GTA V's full crypto (AES-256 ECB and the NG cipher), table-of-contents and
directory tree, nested archives, deflate extraction, RSC7 handling, and
`dlclist.xml`/`setup2.xml` load-order parsing. It is **Unlicense** (public
domain), dependency-light since 0.10 (the image/gltf/wasm deps moved to
`rage-formats`/`rage-render`), and small enough to vendor into `third_party/`
if a crates.io pin is ever a problem. Usage is one call:

```rust
let keys = GtaKeys::load("gta5keys.bin")?;
let file = RpfFile::open("x64a.rpf".as_ref(), Some(&keys))?;
```

The keys are the honest wrinkle. Retail archives are encrypted; the crate
*consumes* key files, and key recovery is its `rage-cli` sibling's job:
scan the user's own game executable for the AES key (newer `GTA5.exe` builds
no longer carry the NG tables as plain bytes, so the crate unwraps bundled,
masked "magic" data with that one scanned key — inert without a real
executable). **Our twist:** the executable here is `game.wasm`
(wasm2c-translated by `crates/glue5-game/build`). Whether the AES anchor
survives in the wasm image, or whether a player's folder also carries the
original `GTA5.exe` to scan, is the first thing to test. Either way: keys are
derived on-device from the player's own copy; none are ever committed.

Secondary reference: **`rpfd`** (`github.com/molchalih/rpfd`), a Rust RPF7
read/write toolchain whose format notes are unusually careful (per-entry vs
per-archive encryption, recorded disagreements with other implementations),
with a corpus-based test setup (`RPF_CORPUS`, `RPF_GAME_IMAGE`) worth imitating.

## The AWC half: the part we write

No Rust AWC decoder exists yet — the gap is tracked upstream as
`VIRUXE/rage-formats` issue #7 ("`.awc` audio container: read, write, WAV
export and import"). The references to port from:

- **CodeWalker** (`github.com/dexyfex/CodeWalker`, C#): `AwcFile.cs` is the
  canonical implementation — stream and multi-channel AWCs, `GetWavFile`
  exporting PCM/ADPCM to WAV, WAV/XML round trips.
- **`aidangilmartin/soundpack`** (TypeScript): `src/core/awc/` is a pure,
  fully unit-tested AWC reader/writer with IMA ADPCM decoding, in a product
  built on the same rule as ours ("never ships Rockstar's files; reads the
  decryption keys from your own gta5.exe"). Its tests even build a fake game
  folder with unencrypted RPFs — a testing pattern worth copying.
- **vgmstream**: decodes AWC incl. the chunked Vorbis variant — overkill here;
  frontend UI sounds are short PCM/ADPCM streams.

Scope to write: parse a (multi-)stream AWC, decode PCM and IMA ADPCM to WAV.
Waveforms are identified inside the bank by hash, so a one-time identification
pass with CodeWalker on the PC pins which waveform is `NAV_UP_DOWN`, `SELECT`,
etc. (names verified in `docs/menu-sfx.md`); the app ships that name table —
names, never audio. Check reference licenses before copying code text; port
logic, write our own.

## How it lands

1. A setup step beside the existing `sfx` loader: open the frontend bank(s)
   in `<game>/data/x64/audio/sfx/`, pull the menu AWC, decode the mapped
   waveforms, and write the twelve files into the user's `sfx/` folder under
   the names `sfx.rs` already wants. No change to the player; re-extract only
   when the bank's bytes change (cheap hash check).
2. The same reader later serves fonts/textures (`rage-formats` already parses
   RSC7 textures; the shell reads them the same way).
3. Tests use synthetic fixtures — `rpf-archive`'s `RpfBuilder` can write an
   unencrypted RPF7 — never real game data.

## Open questions

1. Does the AES-key anchor scan work against `game.wasm`, or must setup scan
   a `GTA5.exe` sitting next to it? (Decides the key flow.)
2. In the *wasm* installation layout, are the retail encrypted RPFs kept
   verbatim under `data/`, or rebuilt open? (If open, no keys needed at all.)
3. Streamed mode: which layer grants "make this file whole now" for a ~5 MB
   bank at setup?
4. Which bank/AWC holds each of the twelve, and does `.rel` metadata need
   reading to map soundset → waveform, or is the one-time name table enough?

Upshot: the missing piece is **half a dependency (`rpf-archive`), half a
focused port (AWC → WAV)** — a bounded build, not research.
