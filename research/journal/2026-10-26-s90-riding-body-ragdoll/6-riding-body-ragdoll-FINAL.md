# 6 — riding body ragdoll (FINAL)

Task S90. 2026-11-21.

## What I did

- Took the last step entry 5 left open: PCSX2 shots beside the shell's, after a put-back and through a
  landing. No product code changed; the ragdoll stays on by default as entry 5 left it.
- PCSX2 side (copy 2): `capture/pcsx2_ride_shots.py` loads `recording2.p2m2_SaveState.p2s` (free09, the
  spawn every motocross state shares) into scratch slot 8. It plays S66's 'ride' pad script in 30 Hz
  ticks through `./kit pad` (gas at tick 45, the put-back chord at 195 and 330). At each shot tick it pauses
  over PINE and presses System → Screenshot over AT-SPI.
- Shell side: `capture/make_tape.py` writes the same schedule as a shell tape
  (`context/tapes/s90/ride.tape`). `capture/shell_shots.sh` runs `openmx --track free09 --play … --headless
  --shot-frame T` once per tick, with the ragdoll on and again with `OPENMX_RIDING_RAGDOLL=0` (the clip).
- Shots, all in `context/shots/s90/` (git-ignored):
  - `side-<T>.jpg`: PCSX2 | shell with the ragdoll | shell with the clip, at 17 ticks from 60 to 410.
    The put-back ones are 200–240 and 335–350.
  - `landing/landing-a.jpg` and `landing-b.jpg`: the same three columns, cropped round the rider, at ticks
    138–163. The PCSX2 log has the bike's vertical speed going from −59 to −23 ft/s at about tick 145.
- What they show:
  - Same spot, same moment: tick 96 lines up on the same rise and road, and the bike heights agree to
    about 1.5 ft at tick 100.
  - Put-back (206, 212, 225, 350): the ghosted rider fades in with the same posture as the original's.
    At 212 it is twisted with the arm out; at 225 it is upright and centred; at 350 both arms are out.
    No over-swing.
  - Landing (138–163): the ragdoll rider stays upright and centred. At the touchdown it dips a little,
    about as much as the original's. It does not fold or wobble after. The clip column is stiffer, with
    no dip.
  - The 10-23 "super floppy" look is gone, and the ragdoll rider is closer to the original than the clip.
- How a kit copy is shot while the desktop is locked: grim gives the lock screen; `./kit shot` falls back
  to the pad hotkey L3+R3, but the game reads L3/R3. System → Screenshot over AT-SPI works while paused.
  The file is written only on resume, so it lands after the 5 s wait; `capture/collect2.py` files them by
  time order. The copy runs at NominalScalar 0.3, which doesn't matter because the shots go by tick.
- S74's and S201's node-row captures (`context/s74/`, `context/s201/`) are gone from the shared context,
  and omarchy-desktop doesn't have them either. So `the_riding_body_gives_as_the_originals` now prints
  "no context/s74/…" and skips. It passed on 2026-11-02 (entry 5). Filed C47 to re-record them.

## What the original does

Nothing new: see entries 1 and 5. Riding, the drawn body is the ragdoll bones pulled toward the animated
skeleton by the riding pose spring.

## How it was proved

- Looked right beside a screenshot: 25 PCSX2 shots of the original against 25 shell shots of the same
  tape from the same spawn, with the ragdoll on and off (files above).
- The numeric swing proof is still entry 5's: RMS within 10%, peak within 0.07 ft, settle within 3 ticks,
  over 13 put-backs and 12 jolts. It is not re-run here, because its captures are missing (C47).

## Not verified / not 1:1

- The pictures are not frame-exact. In the first set, the PCSX2 shot was paused 2 ticks after the asked
  tick (logged as "at tick T+2"), while the shell shot is at T. The landing burst uses the paused ticks
  on both sides. The PCSX2 picture is drawn on resume, so it may be one tick later still.
- The rides drift apart after the first put-back: the chord lands on slightly different ground, so the
  put-back spot and heading differ a little. By tick 410 the two riders are in different places. The
  recording2 state also has three AI riders, which the shell's free ride doesn't.
- Camera and render differ (the shell's chase camera sits higher and farther, so the rider is smaller).
  That is presentation, not this task, and not chased.
- The shell is not seeded from the PCSX2 state. It starts from `--track free09`'s own spawn, the same spawn
  recording2 (and every motocross state) starts from. So the start spot matches, but the bike's speed and
  pose at load are the shell's, not the state's. Tick 96 shows the two lined up.
- The swing proof skips until C47 re-records S74/S201's rows. Its tolerance figures are from 2026-11-02.
  I tick S90 anyway (decided, unattended). The proof passed on main on 2026-11-02. Since then
  `crates/game-sim/src/ragdoll.rs` has been touched (S280, MX vs. ATV's rider world), so a regression in
  between is possible and unchecked. Today's shots show none. C47 puts the check back.
- Entry 5's other items still stand: a wreck on the very first tick enters from the skeleton, and online
  sessions draw the stand-in (V63/P133).

## Next

Nothing for S90. C47 brings back the captures the swing proof needs.
