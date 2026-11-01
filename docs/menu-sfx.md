# Menu sound effects

The menu music (`pausemenu.mp3`) is already in the app and is now muffled behind
the menus (see `crates/glue5/assets/audio/README.md`). What is missing is the
interface's own sounds: the ticks, clicks and swells that a console menu makes
when you move through it.

These are **not** shipped with GLUE 5. Like the scenes (`glue5-scenes`), they are
read from a folder at run time, so nothing copyrighted travels with the
repository, and a missing file simply means silence for that event.

## Status

The app reads these files already (Settings → VIDEO → **Menu sounds** and
**Sounds volume**; the events are wired to the movement, the pages, the
switches and Play). Dropping the files in the folder below is all that is
missing — no code change.

The sounds live inside the game's audio archives (`.rpf` → `.awc`, the
`FRONTEND_MENU`/`HUD_FRONTEND_DEFAULT` soundsets in `x64/audio/sfx`), so they
come out as containers that need extracting and trimming (OpenIV, CodeWalker or
vgmstream, then any editor). `residents.rpf` extracted already has them.

## Where the files go

```
<user folder>/sfx/          (Android: the app's folder, beside scenes/)
glue5-sfx/                  (beside or inside the game folder, if that is where
                             you keep your own assets — same lookup as scenes)
```

Any file may be missing; the app plays what it finds and stays silent for the
rest. No manifest is needed.

**Format**: mono, 48 kHz, WAV (16-bit) or MP3/OGG/FLAC — whatever SDL_mixer
decodes. Peak around −6 dBFS, no leading silence, no long tail (except the three
files marked "swell" below). Names exactly as below, `<name>.wav` or
`<name>.mp3`.

## The sounds

| # | File | When it plays | What to look for in GTA V |
|---|------|---------------|---------------------------|
| 1 | `focus` | The selection moves one line or one tab: D-pad/stick/arrow keys, or the pointer entering another line. Short, quiet tick; it plays a lot. | `NAV_UP_DOWN` (`HUD_FRONTEND_DEFAULT_SOUNDSET`) — up and down are the same sound |
| 2 | `confirm` | A/Enter/tap on a line that does something: Play, Open, Choose…, a mode. | `SELECT` (same set) |
| 3 | `back` | B/Esc/tap on Back: leaves a page, cancels an edit, closes GTA Online's card. | `BACK` (same set); `CANCEL` is its stricter sibling, for dropping what was being edited |
| 4 | `tab` | Switching category or page: the settings' nine categories, the start page's pages. Softer than `confirm`, no pitch hit. | `NAV_LEFT_RIGHT` (same set), left and right alike |
| 5 | `adjust` | A value changes while it stays on the same line: `‹ ›`, a slider step, a keyboard step. Repeats while held, so keep it under ~120 ms. | `NAV_LEFT_RIGHT` again, quieter — the same file as `tab` will do (the app already plays it softer) |
| 6 | `toggle_on` | A switch goes on (VSync, Reduce motion, Touch controls, Haptics…). | no literal name verified (see below) — a short click, rising |
| 7 | `toggle_off` | The same switch goes off. | no literal name verified — the same click, falling |
| 8 | `open` (swell) | A menu appears over the game or the start page: the settings' gear, the mods' menus (F7), the friends. Up to ~1 s. | the pause-menu opening sound; no literal name verified |
| 9 | `close` (swell) | That menu goes away and the game/start page is back. Up to ~1 s. | the pause-menu closing sound; no literal name verified |
| 10 | `error` | Something cannot happen: Play with no game files, a disabled Choose…, a refused action. | no `ERROR` literal in the decompiled scripts; the phone's `Click_Fail` (`WEB_NAVIGATION_SOUNDS_PHONE`) is the closest verified refusal |
| 11 | `launch` (swell) | Play was accepted: the game is about to start. The one sound that may be loud. Up to ~1.5 s. | the "yes/confirm" stinger before a mission starts; no literal name verified |
| 12 | `notify` | A notification slides in (a friend online, a mod's message). | `Text_Arrive_Tone` / `Notification` in the phone scripts (their set is passed dynamically) |

Items 1–11 are the ones the menus need; 12 is optional (the notifications are
already there and work without it).

The named sounds were verified against the game's own decompiled scripts
(307 `PLAY_SOUND_FRONTEND` calls across the 22 pause-menu / HUD / phone
scripts, mined 2026-11-01): the pause menus use exactly `NAV_UP_DOWN`,
`NAV_LEFT_RIGHT`, `SELECT`, `BACK` and `CANCEL`, all in
`HUD_FRONTEND_DEFAULT_SOUNDSET` (the freemode menus reuse the same names from
`HUD_FREEMODE_SOUNDSET`). Where this table says "no literal name verified",
the game's scripts simply never spell that sound out — it is enough to pick
something of the same size and feel from the same banks.

## How they will be played

- One SDL_mixer track per event, pooled (6 voices), so a tick never cuts a swell.
- A `focus`/`adjust` tick is rate-limited to one per 40 ms, so holding the stick
  does not buzz.
- `reduce_motion` does not silence them; a new **AUDIO** category in Settings
  gets Music volume, Effects volume and Menu sounds (on/off), and the music's
  muffling follows the menu, as it does now.
- The music keeps its own track and its low-pass; the effects are mixed in the
  same mixer *after* the filter would dull them, so the clicks stay crisp while
  the track behind them is muffled.
