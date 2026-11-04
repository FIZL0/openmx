# 1 — the struck ball's launch on a seeded ride (FINAL)

Task R27. 2026-11-03.

## What I did

- Copied s3's f27a1 capture (`context/capture/s195/f27a1.csv`) and the RAM images
  (`context/states/s195-f27a1-{2903..2906,2919}.eeMemory.bin`) into this slot.
- New `crates/game-sim/tests/knock_launch.rs`. It seeds the whole ride off an f27a1 image: the
  rider with S174's `Image` seeder (copied from tests/two_p_rider.rs), on Race125P with the bike's
  collision files as the shell loads them, and the woken balls with S195's `seed` (copied from
  tests/prop_knock.rs), `bike_frame` set so the props hear the bike. Then `Sim::step` runs on an idle
  pad (aim.py's ride has no pad) through free09's ground and scene.

## The images against the capture

Image `s195-f27a1-N` is the csv's row N+1: the image's chassis velocity and cofg equal row N+1's.
The strike is between rows 2903 and 2904, so **every f27a1 image is already after the strike**. No
image of the frame before it was saved (2902; B28 asks for one too). The launch takes three ticks of
contact:

| image → next | ball speed (ft/s) | ball vy | contacts in the next image |
|---|---|---|---|
| 2903 (after the strike) | 27.49 | 11.39 | — |
| 2903 → 2904 | 33.43 | 19.31 | bike ray → ball and its mirror, **ball probe → bike frame and its mirror** |
| 2904 → 2905 | 35.23 | 19.64 | bike ray → ball and its mirror |
| 2905 → 2906 | 34.64 | 18.57 | none (gravity only) |

## What the port does

- **2904 → 2905 and 2905 → 2906 match bit for bit**: ball 2's velocity and position, and the bike's
  chassis velocity (`the_launch_goes_on_as_the_original`). The bike's ray entry and the ball's mirror
  agree on from, on_body, normal and point.
- **2903 → 2904 parts.** The original's ball list holds a second entry: its own probe on the bike's
  frame collidable (kind 0, from 0, on_body (0, −4.35, −4.35), normal (−0.061, 0.995, −0.084),
  point (5043.06, 127.64, 3552.56)). The bike's list holds the mirror (kind 1). That pushes the ball up
  off the bike. The port's ball probe meets the bike too, but only as a sound: the ball's `heard` is
  `[(0, Bike(0))]` on that tick, because F27a2 made those hits heard only ("no contacts, wakes or
  activation follow"). So the port's ball leaves at 31.95 ft/s, vy 13.88, against 33.43 and 19.31,
  and the bike's vy is 2.3076 against 2.2470. Filed as **B29**. The ignored test
  `the_balls_probe_pushes_it_off_the_bike` holds it.
- The resolve arms on that tick differ too (the ball's mirror arm (−2.530, −4.131, −3.674) against
  (−2.521, −4.175, −3.652)). That is expected, because the second entry is missing from the resolve.

## The 60-tick ride from image 2903 (`the_ball_flies_clear_of_the_bike`)

The bike's speed stays within 0.01 ft/s of the capture's all the way (18.77 → 1.47). The ball, low
from the first tick, peaks at y 134.89 on +16 instead of 137.58 on +20. It lands on +23 instead of +29
(the capture's first bounce, row 2933). After +2 the bike never strikes the ball again: the gap only
grows (8.6 → 23.5 ft, the capture's 8.7 → 25.5). The ball's probe hears nothing after +1, and its
landings are heard as nothing either.

So, **seeded from the original's own state, the port's bike does not catch the ball again.** The
second bbhit F27a1 heard about 20 ticks after the strike in `research/f27a1/ours.sh`'s shell run is
not reproduced here. That run reaches the ball on its own tape run-up, not from the original's state,
so its contact geometry and speeds differ. That the port's ball lands about 20 ticks after the strike
(+23 here) is a coincidence I could not rule out: the shell's ball landing near the bike might be
what was heard. Since B27 the shell's knock plays no bbhit at all (B28), so a fresh `ours.sh` run
cannot tell the two apart until B28 lands.

## How it was proved

Save-state RAM images against the port's whole `Sim::step`, bit for bit on three ticks (two equal,
one parting, with the missing entry read off the image), and 60 ticks against the capture's rows.
`cargo test --release -p game-sim --test knock_launch` (2 pass, 1 ignored; the ignored one fails as
described when run with `--ignored`).

## Not verified / not 1:1

- **The strike tick itself** (row 2903 → 2904's image 2903) is not compared. No image before it
  exists in f27a1. S197's `prop_knock.rs` proves the same strike on knock-2882 → 2883 bit for bit,
  but in that capture no probe-on-bike-frame entry was compared on the following tick.
- **The shell's second bbhit** was not re-heard. `ours.sh` was not rerun: since B27 it is silent
  (B28). Whether it was a re-strike or the ball's landing is open. B29's fix changes the launch
  anyway, so recheck it after B29 and B28.
- **The 60-tick ride after +1** is not bit-exact. It inherits the low launch, so only its shape was
  compared (no re-strike, the bike's speed).
- **The seeder** is S174's `Image`, built for the 2p state. On these images it gave a bit-exact
  bike on 2904 and 2905, but its fields (the trick machine, motion) were not checked one by one here.
- **What a contact from a prop's probe on a bike frame does in the original's resolve**: whether
  the bike's side is the chassis with the frame's arm, and whether a wake or activation follows. That
  is B29's to read.

## Next

B29: file the contact pair for a prop probe's hit on a ridden bike's frame, and turn on
`the_balls_probe_pushes_it_off_the_bike`. After that and B28, rerun `research/f27a1/ours.sh` with
`match.py --pair` to hear whether the shell's knock is one bbhit, as in PCSX2.
