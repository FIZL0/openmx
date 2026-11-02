# 1 — phone preset (PART)

Task P215. 2026-11-02.

## What I found

The brief (filed from P205's journal, 2026-10-29) asks for two things.

1. **A Phone preset row in Settings that turns every speed-up on at once.** Done already, by P223
   (`ed6dcbc0`, merged the same day; its plan line says "Folds in P215's Phone preset"). Later changes
   kept the preset and re-tuned it:
   - `8fd3f293`: a lower phone ladder that keeps shadows and fog; the original plant detail was removed.
   - `d46ed680`: Shadows split into Off/On/All plus a Shadow Quality of Original/Low/High.

   `video::Preset::Phone` is now one choice on the Graphics tab's Preset row (High, Medium, Low, Steam
   Deck, Phone, plus Auto and Custom). It sets:
   - Fog and draw distance: `Graphics::Far`, four times the original's fog. This is the brief's "nearer
     draw distance".
   - Render scale: 75 %.
   - Shadows: the movers' only (`Shadows::On`), at `ShadowQuality::Low`.
   - Filtering: 4x.
   - Detail: `Detail::Reduced`, coarser models at half the distance.
   - Frame limit: 60 fps.

   Auto on a touch device picks Phone, or rungs below it by model, screen or memory (`video::pick`). The
   page's own default before the player picks anything (`menu::shadows_default`,
   `shadow_quality_default`) is the movers' shadows at Low on a phone.

   Two differences from the brief's wording. Both are the user's later decisions, not gaps:
   - **Shadows are "Low", not "Original".** The 2026-10-30 split gave the original's per-racer shadow
     its own quality, Original. The preset's doc measures Low at about the same cost (free09 at
     1920x1080, 0.2 ms a frame over Original).
   - **There is no "fewer plants".** The plants row was removed on 2026-10-29: the original's plant LOD
     fades in bushes that are not there, and the plants are always openmx's Smooth. The doc on
     `video::Look` says so.

2. **Measure the ride on a real phone.** Not done. It cannot be done unattended:
   - There is no phone attached, and no GUI or remote control of the user's iPhone (tailscale
     `iphone-15-pro-1`). That phone only listens for LocalSend, and only while that app is open.
   - CDP cannot throttle the worker's CPU (memory: cdp-cannot-throttle-a-worker), so a desktop chromium
     cannot stand in for one.

   The page already shows its rate (Settings → Graphics → Show FPS, `NN FPS` top right), and
   beta.openmx.dev serves `92da46e` (2026-10-31), which has the preset. A person can therefore read the
   numbers off the phone in about five minutes. The steps are in `plan/HUMAN.md` under "The ride on a
   real phone (P215)".

## How it was checked

- `./kit check -- -p openmx --test menu video::` passed 3 tests: `the_preset_row`,
  `auto_picks_per_device`, `auto_timing`. They pin Phone's rows, its place on the row, Auto's pick
  on a phone, and the steps below it.
- `curl https://beta.openmx.dev/version.txt` gave `92da46e`.
- `git merge-base --is-ancestor` confirmed that build has `d46ed680` and `8fd3f293`.

## Not verified / not 1:1

- **No real phone measured.** That is the open half of this task, and it waits on the HUMAN.md item.
- **No code changed here**, so no product test was added. The preset's tests are P223's.
- **beta.openmx.dev is two days behind main.** P208's change (the HUD moved clear of the touch pad) is
  not on it. That does not change the frame rate this measures.

## Next

The person runs the HUMAN.md item and puts the numbers in this task (`./kit plan P215`). The next agent
then writes them up here, retunes `video::Preset::Phone` or `video::ios_tier` if the phone misses
60 fps, and finishes.
