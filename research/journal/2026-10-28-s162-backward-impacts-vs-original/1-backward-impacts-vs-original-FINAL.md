# 1 — backward impacts vs original (FINAL)

Task S162. 2026-10-28.

S153's idle sweep found two free06 spots where the port's idle bike wrecks, and I captured the original
at both of them. Nothing from opentennis applies here.

## What I did

- **Checked the port at both spots first** (release build, `sweep/sweep.sh`, race125p, 450 idle ticks):
  - 4000,4000 at heading 180 still bails at tick 179, at −40 ft/s.
  - 2500,3100 at heading 90 no longer wrecks on this main. S153's second pass saw the same.
- **Made two free06 states from S89's along state** (copied from s3's `context/s89` into
  `context/s162`), using `research/s162/turn.py`, which is new. It:
  - teleports the bike onto the spot (S76's teleport);
  - rides it round to the asked heading with a little gas and the stick full over, then the brakes;
  - teleports it back onto the spot at the height it first settled at;
  - settles it braked and saves it.

  First I tried turning it with a yaw rate written in the air (S150's spin). That does not work: the
  game's air control replaces the chassis' angular velocity on the next tick, and spinning into the
  landing wrecked it (reasons 1 and 3). The final settle on the slope swings the bike, so the states
  face a little off the sweep's headings:
  - **bump**: 3999.8, 247.7, 4000.3, facing 167.4°, saved at frame 12722.
  - **prop**: 2500.6, 295.5, 3099.7, facing 72.2°, saved at frame 14262.
- **Recorded both at a quarter speed** with `watch/s89-rails.toml`, nothing on the pad
  (`research/s162/idle.pad`), 450 rows each. Each capture had 0 torn reads and 0 gaps, and
  rider_ctl_src was 1 on every row. The files are:
  - `context/capture/s162/{bump,prop}.csv`;
  - the RAM images, `context/states/s162-{bump,prop}.eeMemory.bin`;
  - the states, `context/s162/{bump,prop}.p2s`.

  All of these are in s1's context only. `research/s162/look.py NAME` summarises a capture, and
  `research/s162/slots.py` moves the states and sets the copy's speed.
- **Wrote the test** `idle_roll_offs_on_free06_wreck_as_the_original_does` in `tests/replay.rs`. Each
  ride runs from the original's state:
  - S129's free06 world, with the path cars seeded from the image;
  - the state's whole 250 (race250p on 250_motoxxx), with wrecks on and the body rays carried;
  - the original's pad.

  It holds three things to the capture:
  - the first wreck: the same reason within a tick, or none (S138's rule);
  - the original's own first wreck, against `S162_RIDES`;
  - how far the two centres of mass get apart while both ride: 2.0 ft for bump, 0.01 ft for prop.

## What the original does

- **bump: the original wrecks too, Back (4), at tick 206.** It rolls back down the slope tail first,
  reaches −41 ft/s, and is at −39.3 ft/s along its heading when it meets the bump at about 3939, 4097.
  This is the port's backward-impact wreck, so it is not a port bug. The original resets the rider
  about 90 ticks later.
- **prop: the original does not wreck.** It rolls off forwards to 7 ft/s, passes the prop's corner at
  about 2510, 3104, slows, stops 32 ft on, and starts to creep back at 0.3 ft/s.

## How it was proved

Two save-state captures (450 rows each), replayed tick by tick from their own RAM images by
`idle_roll_offs_on_free06_wreck_as_the_original_does`:

- **bump:** the original wrecks Back (4) at tick 206, the port Back (4) at 205. The two are at most
  1.53 ft apart before the wreck.
- **prop:** neither wrecks. The port stays within 0.001 ft of the original on every one of the 449
  ticks.

## Not verified / not 1:1

- **The bump replay is not bit-exact.** It parts at tick 2, by more than a thousandth of a foot, and
  drifts to 1.5 ft by tick 203. The port wrecks a tick earlier (205 against 206). That is inside
  S138's ±1 rule but not exact. I did not look for the cause. Filed as S204.
- **The headings are not the sweep's.** The final settle swung the bikes to 167.4° (the sweep used 180)
  and 72.2° (the sweep used 90). The port's own `--trace-at` at 167.5 still wrecks at the same bump
  (tick 208), so the bump case is the same impact. The prop case is not S153's 0.5 ft/s head-ray
  graze:
  - At heading 72 the bike never comes back onto the prop.
  - At heading 90 the port no longer wrecks at all on this main.

  So the original's verdict on a head ray grazing a prop at 0.5 ft/s (`air::hit_wreck` has no speed
  gate) is still unproven.
- **The `--trace-at` drop and the state replay disagree at the prop spot.** `--trace-at
  2500,3100,72.2` (race125p, and the 250 too) reaches 12 ft/s by tick 56 and wrecks against the prop.
  From the state, the bike only reaches 7 ft/s and does not wreck, in the original and the port alike.
  The drop starts a fresh rider, and I think its clutch launch ramp is what makes the difference (3×
  torque for 1.5 s, P154). I did not prove that. Whether the original wrecks against that prop at 12
  ft/s was not captured.
- **Only the 250 was tested.** The states come from S89's 250 (race250p). The sweep rode race125p. The
  125 was not captured at either spot.
- **After the wreck the two sides do different things.** The port's test stops comparing at the first
  wreck. The original's respawn (about 90 ticks later, at 3950, 4083) is not compared.
- **The states were made by hand and live in s1 only.** They are teleported states, not ridden into
  place. The tests skip in other slots.

## Next

None for S162. S204 is the bump replay's drift.
