# Extracting the menu fonts from the player's game (design)

The rule (product decision, 2026-11-01): **no manual import**. The player
installs GLUE 5, points it at their GTA V (server or folder), and the app
takes the menu fonts *from that installation* — the same posture as the menu
sounds (`docs/menu-sfx.md`). The faces are commercial (Chalet, House
Industries) as extracted from a Rockstar file, so nothing is ever bundled or
committed; the extraction happens on the player's device, from the player's
copy, into a local cache.

## Evidence (2026-11-01, round-2 reference lot)

A tag walk over the 24 `.gfx` inputs of the round-2 lot
(`scaleform-ui-source-02\_input-base\`, SHA of the lot `EBC1AA55…`):

- **0 of 24 films embed a font.** Every one imports font characters via
  `ImportAssets2` from two shared libraries: `gfxfontlib.swf` and
  `font_lib_efigs.swf`.
- The menu faces (`$Font2`, `$Font2_cond`) live in the **`_pc` variant of
  the EFIGS library** (`font_lib_efigs_pc`), per the glyph audit in the
  inventory (§5): 320 glyphs, weight 400 only, pt-BR complete, missing `★`
  (U+2605) and `−` (U+2212).
- GFX framing is standard SWF: `GFX` signature, uncompressed, RECT then
  tag stream — a ~40-line walker traversed all 24 films cleanly, so the
  same code reaches `DefineFont2`/`DefineFont3` inside the libraries.

## The chain

```
user's install ─► scaleform_platform_pc.rpf ─► font_lib_efigs_pc.gfx
                                                      │  DefineFont2/3
                                                      ▼
fonts/font.ttf + fonts/font-condensed.ttf  ◄── synthesize TTF (this doc)
                                                      │
                              theme::install loader (15dd7f5) reads cache
```

1. **Reach the bytes — GLU-21.** The RPF reader (another chat's task;
   `docs/rpf-awc-reader.md`) opens `scaleform_platform_pc.rpf` in the chosen
   installation (folder or stream cache) and returns the
   `font_lib_efigs_pc.gfx` entry. Exact in-RPF path is an open question (see
   below), answered once on the PC.
2. **Convert — a pure function.** `fn font_ttf(gfx: &[u8], face: Face) ->
   Option<Vec<u8>>`. No filesystem, no game knowledge: bytes in, TTF out.
   Testable against synthetic fixtures (build a tiny DefineFont by hand) —
   never against real game bytes in the repo.
3. **Cache.** Write the two TTFs to `paths::fonts_dir` next to a small
   sidecar key (source identity: RPF path + entry size + a hash of the
   `.gfx` bytes). Re-extract only when the key changes. Write via temp file
   + rename; any failure logs once and leaves the previous cache (or none).
   The folder is *only* a cache for this — `15dd7f5`'s loader already reads
   `font.ttf`/`font-condensed.ttf` from there at startup and validates the
   sfnt header, so the converter plugs in without touching `theme.rs`.
4. **When.** During the same setup step that will decode the menu sounds
   (GLU-21's plug point), where the game dir is known. If extraction hasn't
   run yet, the first launch shows Manrope and the next shows the game face;
   extraction must never block first paint (run it off the UI thread).

## Converting DefineFont2/3 → TTF

SWF glyph outlines are the **same primitive as TrueType `glyf`**: contours
of on-curve points and quadratic Bézier off-curve control points. The
conversion is re-encoding, not tracing:

- **Walk** to the `DefineFont2` (48) / `DefineFont3` (75) whose name matches
  the wanted face (`$Font2` / `$Font2_cond`). Parse flags (layout present,
  wide offsets, wide codes), the glyph offset table + code table, and —
  with the layout flag — advances, bounds, ascent/descent/leading.
- **Scale.** DF2 shapes live on a 1024-unit EM square; DF3 stores shapes at
  20× sub-pixel precision. Getting this factor right (and where it applies:
  shape coordinates vs. advance/bounds fields) is **the first thing to
  validate**, by diffing against the FFDec TTF exports from the Drive lot
  (glyph 0.x advances are a cheap oracle) on the PC, before trusting it.
- **Emit a minimal sfnt**: `head`, `hhea`, `maxp` (v0.5 is *not* enough —
  `glyf` needs 1.0 fields), `hmtx`, `cmap` (format 4; format 12 if any code
  point exceeds BMP), `loca`, `glyf`, `name`, `post` (format 3.0, no names),
  and `OS/2` conservatively. No hinting (`fpgm`/`prep` absent is legal).
  epaint/ab_glyph is a lenient consumer, but table checksums and the
  `head` checksum adjustment must be correct — other parsers in the chain
  (fontdb in some paths) are not all lenient.
- **Glyphs**: SHAPE records → `glyf` simple glyphs (respect SWF's
  implied-on-curve midpoint rule — same rule as TrueType, so sequences map
  1:1; only re-encode flags/runs). Empty glyphs (space) get no outline and
  an advance. Glyph 0 is `.notdef` — SWF fonts have no guaranteed `.notdef`;
  prepend an empty glyph 0 and shift indices so `cmap` never needs ID 0 for
  a real character.
- **Kerning**: the layout block's kerning records can fill a legacy `kern`
  table later; v1 ships without (FFDec's exports are likewise kern-light)
  and measures whether anyone can tell.
- **Names**: `name` from the tag's font name (and `DefineFontName` 88 when
  present). This stays on the player's device; the file is their extraction.

## Open questions (answer once, on the PC / with GLU-21)

1. Exact path of `font_lib_efigs_pc.gfx` inside `scaleform_platform_pc.rpf`,
   and whether the wasm install's RPFs are retail-encrypted or rebuilt open
   (same unknown as GLU-21's SFX banks — answer once for both).
2. DF2 or DF3 in the `_pc` library, and the exact EM scale factor (oracle:
   FFDec exports).
3. Is `$Font2_cond_NOT_GAMERNAME` ever needed? No for our menus — skip.
4. Server mode: is the platform RPF guaranteed whole before setup, or must
   setup ask the stream layer to complete it first (GLU-21's "make file
   whole" question)?

## Division of labour

- **GLU-21** (other chat): RPF bytes for an entry path.
- **This task** (UI chat): the pure converter + cache writer + setup hook.
  The loader half already landed in `15dd7f5` (`theme::install` +
  `paths::fonts_dir`); the drop-a-TTF-in behaviour it has today is a
  *testing seam* for the converter's output, not a user feature.
- **Validation**: converter unit tests with synthetic DefineFont fixtures;
  on-PC diff of advances/coverage against the FFDec exports; on-device
  screenshot next to the real pause menu.
