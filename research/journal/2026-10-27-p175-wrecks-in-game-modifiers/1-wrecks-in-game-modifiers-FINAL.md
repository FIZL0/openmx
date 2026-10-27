# 1 — wrecks in game modifiers (FINAL)

Task P175. 2026-10-27.

## What I did

- The Ride tab's Wrecks and Air Wrecks rows (P138) are gone from `ride_rows`; the Game Modifiers page
  has them under a **Wrecks** heading after the modifier sliders and before Presets
  (`menu_pages.rs` `wrecks_rows`). Key::Wrecks / Key::AirWrecks and their toggles are unchanged.
- Kept: Air Wrecks dimmed "Wrecks are off" while Wrecks is off; under the pause its note is "Changes
  take when the pause closes."; `air_held()` still holds it (the online room / a played tape).
- The Ride tab's Game Modifiers row counts Wrecks off and Air Wrecks off as changes ("2 Changed"), and
  its help says wrecks live there.
- **Decision: the world's, not per-player.** Both rows carry the World tag and a "The world's: one value
  for every player" note, like gravity / air drag / time scale, and are held ("The world's: only the host
  sets it.") on a page that is not the host's (an online guest, or a 2P+ pauser). Why: the switches are
  one bool each in `Settings`, the online room carries one `wrecks` / `air_wrecks` for the whole ride,
  the shell writes `wrecks_enabled` / `air_wrecks` to every seat from that one value, and a tape records
  one Air Wrecks switch. Per-seat wrecks would need the room wire, the tape header and the shell's seat
  setup changed — a task of its own, not a menu move. Behaviour change worth knowing: a local 2P who
  opens the pause can no longer flip Wrecks (they could from the Ride tab), the same as the world's
  sliders.
- **Decision: not in saved presets.** Presets are `Mods` (the u16 knob table) and their file lines;
  loading one leaves the wrecks as they are (tested). Reset to Original does put both back on, but only
  the host's All Players reset ("Every modifier back to the original game, Wrecks and Air Wrecks on"),
  and Air Wrecks only when not held; one player's reset leaves the world's, as it already did for the
  world sliders.
- settings.txt is unchanged: still `wrecks = off` / `air_wrecks = off`, so old files read (tested with
  `Settings::parse`).
- Tests (`crates/openmx/tests/menu.rs`): the P138 air-wrecks test now walks Ride → Game Modifiers;
  the pad-settings test turns Wrecks off on the modifiers page; the tab-first-row test expects Bays as
  the Ride tab's first row; the modifiers page row count grows by 3; new `wrecks_are_game_modifiers`.
- opentennis: nothing to take — it has no wrecks and its modifiers page is not this one's shape.

## What the original does

Not applicable: the menus are ours to design (AGENT.md). The wreck rules themselves are untouched.

## How it was proved

Menu unit tests through `./kit check -- -p openmx --test menu` (175 passed), then the gate in `./kit done`.

## Not verified / not 1:1

- The page was not looked at on screen (no shot taken): the Wrecks heading and the two rows go through
  the same row drawing as every other page row, World tag included, but where the list scrolls with
  the extra three rows was not checked by eye.
- The local 2P-pauser hold on the wrecks rows is tested only through the online-guest path
  (`mods_own()` = Some), not by a real 2P pause; both go through the same `mods_own()` check.
- The lobby's strip chips ("Wrecks On/Off", Triangle toggles Wrecks) and its "Modifiers N changed" chip
  were left as they are; that chip's count does not include the wrecks (it sits beside its own Wrecks chip).

## Next

Nothing.
