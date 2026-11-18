# 1 — outro stops the flashes (FINAL)

Task S334. 2026-11-18.

## What I did

- `crates/game-sim/src/race.rs`:
  - `Race::tip_words`: the "preload and clutch" popup has a word to show. The front end sets it.
  - `Race::outro`: the menu's fade-in over the race, in seconds run. Starts at 0 on the tip's tick.
  - `OUTRO_FADE`: 0.25 s with words, 0.75 s without.
  - `Race::tick`: at the end of every tick from the tip on, the fade runs `+= dt`. Once
    `min(run / length, 1)` reaches 1 (`Race::outro_up`), the tick returns at once and does nothing:
    no clocks, no laps, no flashes, no draws.
- `snapshot.rs`: the two fields are on the race's wire (2 bytes more per race).
- `hash.rs`: they are walked only once the tip has shown, so every race before it hashes as before.
- `tests/camera_flash.rs`: rows now keep the activity's bytes. Two new tests (below).
- `research/oracle.txt`: `race_tip_fade` and `race_prop_raiser`.
- Nothing taken from opentennis: it has no race or outro.

## What the original does

- The race manager's tick (`./kit fn 0x002fcff0`) runs the tip clock (`+0x410`). The race's end arms it
  from the outro scene's length (Race::tick's `stage` path, ported in S13b).
- When it runs out, it pushes the "PreloadAndClutchTipPopup" menu and starts the menu's fade
  (`./kit fn 0x001c8670` → `./kit fn 0x001c8e80`). The fade-in is 250 ms if the popup's `+0xb4` or
  `+0xb8` is set (some player missed the preload or the clutch, `tip_popup_arm`), else 750 ms.
- The fader's tick (`./kit dis 0x001c8a80`): while its share is under 1, `run += dt`,
  `share = min(run / length, 1)`.
- In S13d's capture the tip clock reads 0 on row 2762. The activity then ticks on rows 2763–2769 and
  never again: its own clock (`+0x148`), the score manager's clock (`+0xf8`) and the flash object all
  freeze to the capture's end (row 2843). 7 more ticks = the 0.25 s fade at 1/30 s.
- So the flashes stop because the whole race activity stops, not because of anything in the flash
  object. The port does the same: the whole `Race::tick` stops.
- **The other child (`+0x3a8`, ctor `./kit fn 0x002fde40`, tick `./kit fn 0x002feb40`)**: it exists on
  sx01 (activity `+0x3a4` = 1). It raises two named groups of scene props by 50 ft over a time, once a
  card passes each of two spots. Its tick and its two callees call no `rand()`, so it does not move the
  shared generator. Not ported; filed V134.

## How it was proved

- **Live capture** (`context/capture/s13d/sx01-finish.jsonl`), new test
  `the_tips_popup_fading_in_stops_the_race_and_its_flashes_as_s13d_ends`:
  - Starts a `Race` on the row the tip is armed (2568), its tip clock from the row, `tip_words` on,
    and the flashes resumed on the first batch after it.
  - Every row to the capture's end: the race ticks exactly when the activity's clock moved. While it
    ticks, the tip clock and the flash words match bit for bit, and the tip fires on the row the
    capture's clock reaches 0. Once stopped, the tick does nothing and the race (generator included)
    stays the same, as the capture's flash words do.
  - It stops on the tip's row + 8, the first frozen row.
- `the_race_stops_when_the_tips_fade_has_run`: 7 more ticks with words, 22 without, then frozen.
- Mutant: fade 0.25 → 0.2 fails both tests. Restored.
- `./kit check -- -p game-sim --test race --test hash --test camera_flash`: 101 passed.

## Not verified / not 1:1

- **What stops the activity is not traced.** The capture shows it stop after the fade's time, and the
  fader's own code is read. The code that reads the fader (or the pushed menu) and stops ticking the
  activity was not found. The port says "once the fade is full, no more race ticks", which fits the
  capture's 7 ticks exactly.
- **The fade's order inside a frame is not proved.** The port runs the fade after the race's work, on
  the tip's own tick too. Running it first in the next frame gives the same count at a steady dt, so
  the capture cannot tell them apart. With an uneven dt they could differ by one tick.
- **The 0.75 s fade (popup with no words) has no capture.** Read from the code only. That S13d had words
  is inferred from the 7 ticks (a 0.75 s fade would be 22).
- **The fader is assumed idle at the tip.** If it were mid-fade, the original starts part way in
  (`+0x4c`) or skips the fade (`+0x48`). Not modelled.
- **The stop does not happen in a ride yet.** The Sim never sets `Race::stage`, so the tip never fires
  there. Arming it from the outro scene's length is S480. The shell still leaves the ride on its own
  7 s wait.
- **Only the race stops.** In the original the whole activity stops (its other objects too). Riders,
  AI and the rest of the Sim keep going in the port.
- **The `+0x3a8` object's props call (its vt+0xe4) was not followed** for `rand()`. Its own tick and
  its two callees have none. Not ported (V134).
- **The wire is 2 bytes longer per race.** Netplay tests that count a race's bytes could move.

## Next

Nothing. Follow-ups: S480 (arm the outro in play), V134 (the prop raiser).
