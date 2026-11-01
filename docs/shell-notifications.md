# How the shell speaks: notifications

*Set down 2026-11-01, when the help line (`Ui::alert`) landed, so the next
occasion is a one-liner in the right channel instead of a new mechanism.
The aim, in the owner's words: balance simplicity with room to bend.*

## The five channels

| Channel | Code | Shape | Lifetime | For |
|---|---|---|---|---|
| Warning pop-up | `ui/popup.rs` | Modal box, whole screen dims | Until answered | Asks that block: delete, restore |
| Notifications (toasts) | `Ui::notify`, `notify_mod` | Social Club cards (title/name/line) | Queue of 3, 3.5 s each | What *other people* did: friends, mods |
| Help line (hints) | `Ui::alert` | GTA help-text strap (black α186, margins 7/5/4/12) | Reading time: base + 333 ms/word, +30% | What *I* did, done: copies landed, waits ended |
| Settings note | `Ui::note` | One sticky line in Settings | Until replaced | What may need re-reading; refusals with reasons |
| Signs | stream chip, start-page problem card | Small, state-shaped | While the state lasts | Continuous states, never events |

Pick by asking, in order: **Must it be answered?** pop-up. **Did someone
else do it?** toast. **Did I do it, and is it done?** help line. **Must it
be there to re-read or act on later?** note. **Still true in a minute?**
sign. Errors the player can act on are notes (they wait); errors that are
just "that didn't work" about a press just made may be help lines, but so
far every error stays a note — a refusal you can't re-read is a dead end.

## Stage rules (shared, already in force)

- One placement pass for toasts + help line + chip: the same `safe`,
  `below` and `taken` list (`toast_place`), so nothing covers the touch
  controls, the gear or each other. The pop-up is the exception: it owns
  the whole screen.
- One sound per channel (`Notify` for toasts and help lines; the pop-up
  has its own answers). Fades: 350 ms toasts, 100 ms help lines.
- Toasts queue (3, one after another); a help line **replaces** the one
  showing, and the *same* line again only extends it (no blink, no second
  tick). The pop-up is singular and takes the keys.
- Completions must be **polled into the app loop** to be spoken of
  (`imports::install_ready` is the house pattern). Background work with no
  way back gets no help line; give it a poll or leave it silent.

## Occasions (audited 2026-11-01)

| Occasion | Today | Verdict |
|---|---|---|
| Diagnostics copied; log capture requested/started/finished; debug marker | Help line | Wired (success was silent, or a settings-only note) |
| Save/resources imported (`install_ready`) | Help line | Wired (was a settings-only note) |
| Graphics restored | Help line | Wired (closes the pop-up loop) |
| Controller connected | Help line | Wired (disconnected: no — the game/pauses own it) |
| Local copy deleted | Nothing (background, no signal back) | Roadmap: completion poll, like imports |
| Stream download complete ("the whole game") | Pages change under you | Roadmap: edge-detect in App |
| Server back after a drop | Chip just clears | Roadmap: edge-detect in App |
| Device thermal serious/critical (iOS) | Log line only | Roadmap |
| Game folder chosen/found | The page itself changes | Not a help line |
| No game (refused/not found) | Problem card + note | Not a help line (must be re-read) |
| GPU driver added ("from next launch") | Note | Stays a note (future-tense, re-readable) |
| "Applies at next start" settings | Note | Stays a note |

## Growth, only when a consumer hurts

- A duration override on `alert` (the game's help text has
  `OVERRIDE_DURATION`) — when something must outstay the formula.
- The help line for mods too, beside their `ui.notify`.
- Button glyphs inside a line (`~INPUT_…~`-style), once fonts/glyphs land.
- A one-deep queue if replacements start eating confirmations in practice.

Resist: a general notification centre, priorities, history, per-channel
settings. The game we mirror has none of those; five small channels with
one rule each beat one framework with flags.
