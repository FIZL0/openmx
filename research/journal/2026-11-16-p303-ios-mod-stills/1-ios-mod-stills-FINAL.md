# 1 — ios mod stills (FINAL)

Task P303. 2026-11-16.

## What I did

- `track_previews.rs`: new `SHOOTS` (false on wasm and iOS). Where it is false, `gather` asks for no still,
  so `still_job` never spawns the shell on iOS — the 38 `Operation not permitted` lines a launch are gone.
- On iOS a mod with no `previews/track.png` now shows the first other `previews/*.png` in its folder (a
  store zip's picture), as the browser already did; otherwise its card has no picture.
- The mods left with no picture are one log line, said once a launch (a static flag), not one per track.
- Chose not to take the still in-process: a still is a whole second world loaded beside the menu, which
  P282 stopped doing on the Deck for memory; a phone has less. The store's picture or none is the
  option the task offers.
- Test `a_build_that_takes_no_stills_names_the_bare_ones_once` (in the `track_mods` test binary) runs the
  no-stills path on Linux through `gather_on(.., shoots=false, ..)`.
- opentennis: has no mod track pictures; nothing taken.

## What the original does

Not applicable: front end, ours to design.

## How it was proved

Unit test above (38 bare tracks → one line, no takes; store picture read; once-flag silences the second
call). `./kit check -- -p openmx --test track_mods`: 22 passed.

## Not verified / not 1:1

- Not run on an iPhone or the iOS simulator: no device in the run. The change there is the `cfg!(target_os
  = "ios")` constant only; the same code path is tested on Linux with `shoots = false`.
- The iOS build was not compiled in this run (needs the hst-ios SDK and an upstream toolchain); the edit
  uses no iOS-only API.

## Next

None.
