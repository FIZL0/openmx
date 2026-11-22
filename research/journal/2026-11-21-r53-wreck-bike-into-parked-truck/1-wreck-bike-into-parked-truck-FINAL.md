# 1 — wreck bike into parked truck (FINAL)

Task R53. 2026-11-21.

## What I did

- Took the route the user named: force P2's wreck by a poke near the truck, plane parked far off.
- Booted R48's free09 for two players (research/r53/start.sh, now honouring `KIT_PCSX2`), on copy 1.
  - Copy 1 had been left at `NominalScalar = 0.05` (5% speed). Boot then stalled ("never in game") and
    the frame counter sat at 0. Set back to 1.0 in the copy's PCSX2.ini.
- tele_p2.py stood P2 18 ft south of the truck, facing it (slot 12).
- New research/r53/wreck_p2.py, in one paused moment:
  - P2's bike velocity set to 25 ft/s at the truck (the words tele_p2.py zeroes);
  - P2's forward limit (rider +0x758, 22.0) set to -1e30.
  - The original's wreck test (`./kit fn 0x2522c0`, first clause `+0x758 < +0x558`) then enters the
    wreck itself on the next tick, reason 4, through vt +0xfc. The limit is put back after.
- Recorded research/r53/r53-wreck-truck.toml from the wreck state and from the poked state. The
  wreck-state capture was recorded twice and kept once. The second recording added the contact
  columns and overwrote the first; both gave the same wake on row 358.
- Nobody rides the plane: it stays parked where the boot put it, 391 ft south of the truck.

## What the original does

- Tick 348 enters the wreck (reason 4). The wreck body starts with the bike's velocity
  (-1.75, -0.01, 24.96), 16.6 ft from the truck's body.
- Ticks 349-357: the wreck bike tumbles toward the truck, slowing 25 → 16.9 ft/s. The truck is asleep.
- Row 358: the truck wakes. Its contact list has one own entry (kind 0) whose other object is
  0x96c060, the wreck body 0x96b020's collision object (+0x1040). So P2's wreck bike woke it.
  - Wreck body to truck body: 10.6 ft at the wake.
  - From 359 the wreck bike bounces back at -2.3 ft/s. The truck barely moves (0.02 ft/s).
- The truck sleeps at 364 and wakes again at 374, when the wreck bike touches it again. It then
  flickers awake/asleep through ~410, creeping -z at ~0.27 ft/s, and sleeps from 449.
- P2 respawns at 445 (reason back to 0).

## How it was proved

- Live PINE recordings (`./kit record`, 0 torn reads, 0 counter gaps):
  - `context/capture/r53/wreck-truck.csv`: from slot 13 (frame 348), rows 348..497.
  - `context/capture/r53/wreck-entry.csv`: from the poked slot 14 (frame 347), rows 347..426.
  - The two agree on every column from row 350 to 426. So the run repeats.
- State images in `context/states/r53/` (each `.ram`, `.spr` and the slot's `.p2s`):
  - `r53-348`: the wreck entered, limit restored.
  - `r53-347-poked`: the next tick enters the wreck; limit still -1e30.
  - `r53-staged`: P2 standing, 18 ft away.
  - `r53-boot`: the two-player boot.
- `research/r53/ram_peek.py r53-348`: the image's wreck body is exactly row 349's.
  - So a row N holds the state at the start of tick N, and image 348 is the state before tick 349.

## Not verified / not 1:1

- The wreck is forced, not ridden.
  - The bike's velocity is poked, not reached by throttle.
  - The reason is 4 (pitched forward), forced by the limit poke. The test asks for 7 (head-on contact),
    whose clause also writes a throw to +0x560. No run reached 7 away from a pinned rider.
  - The rider/ragdoll words were not reset by the poke. Only velocity words were set.
- First rows are not clean.
  - Row 348 of wreck-truck.csv is a pre-load read: it holds the previous run's end state.
  - Row 349 of wreck-entry.csv (3822.13) differs from wreck-truck's (3821.31): a read at another phase.
  - Row 358 is torn in both. The wreck columns repeat 357's in wreck-truck; the truck has 2 contacts in
    wreck-entry against 1. Use rows 350-357 and 359+.
- `p2_wrecked` (rider +0xb7c) stays 0 through the wreck. The oracle notes say +0xb7c = 1 for the
  vehicle, so this offset is not that flag on the rider object. Not looked into.
- wreck-entry.csv runs its whole length with the limit still poked. The wreck test's clauses only run
  while the reason is 0 (`./kit fn 0x2522c0`: the clauses follow `+0x748 != 0 → skip`). The capture ends at 426,
  before P2's respawn at 445, so the poke cannot fire again inside it. The match with wreck-truck.csv
  from row 350 agrees with this.
- Contact entry layout (+0 kind, +0x74 other object) is taken from S489's notes. The second entry is
  read at +0x80, assuming the entries sit back to back. That is unchecked, so the truck_contact1
  columns, and the contact count on torn row 358, are not trusted.
- The 10.6 ft gap is body origin to body origin. Whether the Sim's wreck-bike origin is the same point
  as the original's wreck body origin is not checked. S506 must compare like with like.
- The c16_fly test was not run against this capture. R53 is research: no test changed. It is a
  different scene from the test's (C16 image, truck put beside the wreck). At the wake the
  original's gap is 10.6 ft (body to body), past the test's `< 10.0`. The test measures from the
  Sim's wreck-bike origin, which may not be this body origin. Filed as S506.

## Next

- S506: a c16_fly test that seeds from `states/r53/r53-348.ram` and holds the Sim to
  `capture/r53/wreck-truck.csv`. Use the wake row (358), the plane's distance, and the wreck bike
  before and after the bounce.
