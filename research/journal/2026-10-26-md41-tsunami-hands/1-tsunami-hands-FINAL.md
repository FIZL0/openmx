# 1 — tsunami hands (FINAL)

Task MD41. 2026-10-26.

## What I did

- **Measured first.** A probe (now `crates/openmx/tests/flip_tricks.rs`'s harness) rode `game-sim/tests/ride.rs`'s
  backflip and front flip off the whip tests' ramp on the 250sx with mods/tricks/mxvatv_tricks. It threw a
  Tsunami (slot 16) and an Indian Air (slot 9) in each flip, ran the body and the chassis in the shell's order, and
  logged the body's state, the bike's state, the hold, and the drawn hands against the grips in the bike's space.
  Before the change:
  - **Backflip.** The body ran BACKFLIP (38) and the bike AIR (35): the bike had no backflip clip and
    `Chassis::want` never asks for 38. The clip's hands were 0.51–0.88 ft off the grips at entry, then 0.15–0.17,
    drawn as written (hold 0, V61's rule).
  - **Indian Air.** No bike clip exists for it on the disc or in the pack. The bike went to slot 9 and drew its rest
    shape (a still bike), and the hands were 0.25–0.41 ft off, unheld, through the trick.
  - **Tsunami.** The 250sx's own archive has `bike_tsunami`, so the bike moved, the hands were 0.11–0.26 ft off
    during the trick and back on the grips one fade after it. At the trick's end the bike left slot 16 one tick
    before the body did: `Tricks::current` lags the body's request by a frame.
  - **The pack's bike clips were never used for the 24 slots.** That is what the shell's "7 of 24" says.
    `bike_members` asked only for `MX_Bike_<stunt>`; the pack ships `bike_<stunt>` and `MXF_Bike_<stunt>`.
    `PackClips::fill` only looked for extras and 360s.
- **Filled the holes** (`anim.rs` `PackClips::fill_empty`, called from `Chassis::add_pack`):
  - Every gesture slot the disc's `bike.mcf` leaves empty, and the backflip (state 38), is filled from the pack.
  - `bike_members` now tries `MX_Bike_`, `bike_` and `MXF_Bike_`.
  - On the 250sx the disc already has all five stunts the pack has a bike clip for (taunt, nacnac, flatliner, kod,
    tsunami), so no gesture slot gains a clip. The backflip does: `bike_backflip.mot`.
  - A bike whose archive lacks one of the five now gets it from the pack. That may be the user's case: a different
    bike, whose Tsunami was a still bike.
- **The bike follows the body** (`vehicle.rs` `Chassis::want_under`, used by the shell):
  - The body's gesture slot is the bike's on the same tick, which fixes the one-tick lag.
  - The body's BACKFLIP is the bike's when the bike has a clip for it.
  - A trick the bike has no clip for (`Chassis::lacks`) is not run at all. The bike stays where it would be without
    the trick: the backflip it is turning (the body's own `backflip` test, front flips excluded), else the airborne
    row, else the riding grid. So it keeps turning and crossing up under the rider instead of drawing its rest shape.
  - The shell logs the slots still without a bike clip: `chassis: no bike clip on the disc or in the pack for ...`.
  - On the 250sx with the pack that is 17 slots: lazyboy, cancan, nohandedlander, pendulum, hartattack, indianair,
    barhop, supercan, superman, saranwrap, stalelin, onehandindianair, shoalin, cordova, deadbody, rigamortis and
    stripper. The extras with no bike clip are cliffhanger, rocksolid, stalefish, mcmetz, switchblade and nofooter.
- **The hold for those tricks** (`rider.rs` `Body::bare`, `Body::holds_on`, `bare_reach`, `BARE_NEAR` 0.8 ft and
  `BARE_FAR` 1.6 ft):
  - While the body is in a trick the bike has no clip for, or fading out of one (`Chassis::bare_under`), each hand
    is held on its grip with at least `bare_reach` of how far the clip put it.
  - The gate comes from all 24 disc clips thrown off a jump, in a backflip and in a front flip (72 rides). A hand a
    clip keeps on the bars is at most 0.74 ft off its grip (stalelin in a backflip; Indian Air in a backflip 0.64).
    A hand it takes off the bars reaches at least 2.06 ft (super can Indian air; lazy boy 4.5, no-handed lander 2.9,
    saran wrap 2.9). So 0.8 and 1.6 put a clean gap between the two kinds of hand.
  - V61's rule (a posed clip is drawn as written) is untouched for tricks the bike has a clip for and for the
    backflip itself. `a_gesture_is_drawn_as_the_clip_wrote_it` still holds.
- **The front flip** (brief: "check the frontflip shows the backflip's bike clip"): it does not, and now says so.
  - Since P159 the body does not play the backflip in a front flip. It holds the airborne lean full forward, and
    the bike follows the body, so the bike is on its airborne row.
  - The backflip's bike clip was written under a rider pulling back; under a rider leaning forward it would put the
    grips somewhere the hands are not.
  - The trick-pack log line, which claimed the front flip "shows the backflip's", was wrong and is corrected.

After (flip_tricks.rs, printed by the tests):

| ride | bike | bike nodes off the plain flip's | hands after the trick |
|---|---|---|---|
| Tsunami in a front flip | slot 16 every tick | 4.16 ft | ≤ 0.15 ft for 1 tick, then held (0.00) |
| Tsunami, no flip | slot 16 | 4.16 ft | the same |
| Tsunami in a backflip | slot 16 | 4.19 ft | ≤ 0.34 ft: the backflip's clip as written, then the landing |
| Indian Air in a front flip | airborne row, held | 0.00 | 0.00 throughout |
| Indian Air in a backflip | backflip clip, held | 0.00 | ≤ 0.28 ft: the backflip and its landing as written |

## What the original does

Not read in the decompilation for this task. What the data says:

- MX vs. ATV's `MX_Rider_Stunts.ini` marks `UsedForBike = T` on exactly the stunts it ships a `bike_*.mot` for.
  That is the rule this port's `anim.rs` documents for MX Unleashed's disc, and it holds on the pack's 45 rows with
  one exception: the backflip says F but ships `bike_backflip.mot`. The backflip is not a gesture slot; it is the
  vehicle machine's own state 38.
- Indian Air is F in MX vs. ATV too. The original game does not move the bike for it either, so the bike's pose under
  an Indian Air cannot differ from a plain flip's and stay 1:1 with either game.

## How it was proved

`crates/openmx/tests/flip_tricks.rs`, four tests, run in the shell's frame order (ride, body, `want_under`, chassis,
`bare_under`, palette):

- `a_tsunami_ends_with_the_hands_on_the_grips`: a front flip into a Tsunami, and a Tsunami alone.
- `an_indian_air_off_a_front_flip_ends_with_the_hands_on_the_grips`.
- `tricks_in_a_backflip_move_the_bike_and_keep_the_hands`: Tsunami and Indian Air.
- `the_pack_fills_the_bikes_backflip_and_names_what_it_cannot`.

Each trick test asserts:

- a trick with a bike clip: the bike's state equals the body's on every trick tick, and some bike node is drawn more
  than `POSE_APART` (0.1 ft) off a plain flip's at the same tick;
- a trick without one: the bike is never in the empty slot, and the hands are held within `ON_THE_GRIPS` (0.05 ft)
  on every trick tick;
- on every tick after the trick ends: a held hand is within `ON_THE_GRIPS`, and an unheld one is within `AS_WRITTEN`
  (0.4 ft);
- outside a backflip, at most `UNHELD_TICKS` (8) unheld ticks after the trick;
- the ride ends riding and held.

The tests skip without mods/tricks or the disc.

## Not verified / not 1:1

- **The original MX vs. ATV was not filmed.** The data answers which tricks move the bike (`UsedForBike` and the
  shipped clips). The drawn hands in the original during a trick inside a flip were not checked: no pad script
  exists for a flip with a trick thrown in it in the original, and building one was out of proportion here.
- **The user's exact ride was not reproduced.** On the 250sx in this harness, a Tsunami off a front flip already
  ended with the hands held one fade after the trick before this change. The fixes cover the two ways it could have
  failed: a bike whose archive lacks `bike_tsunami` drew a still bike, now the pack's clip; and a trick with no bike
  clip at all, now the hold. Which one the user's bike hit is not known.
- **The Indian Air's bike pose does not differ from a plain flip's**, against the brief's ask. Neither game ships a
  bike clip for it (MX vs. ATV marks it not `UsedForBike`). The bike keeps the flip's turn under the rider and the
  hands are held, which is the brief's own fallback.
- **The front flip shows the bike's airborne row, not the backflip's clip.** The reasons are above; it is a choice,
  not the original's.
- **What the bike draws under a trick it has no clip for is the port's own.** It is the backflip, airborne or riding
  row it would run anyway. MX Unleashed's loader binds nothing in such a slot, and what the original draws for an
  empty bike slot was not read. Before this change the port drew the rest shape.
- **`BARE_NEAR`/`BARE_FAR` and the hold under clipless tricks are the port's own.** They also apply with no trick
  pack (MX Unleashed's disc clips leave the same 17 slots empty on the 250sx). In MX Unleashed's terms that is a
  presentation change: V61 measured posed clips drawn as written there.
- **The pack's `bike_backflip` is paired with the disc's rider backflip**, not MX vs. ATV's own rider backflip clip.
  The hands at backflip entry are 0.51–0.88 ft off, as they were with the airborne row under it; in the flip they
  are 0.17 (was 0.15). They were not compared with the original.
- **Backflips still draw the hands off the grips** (up to 0.34 ft after a Tsunami's fade, 0.28 ft in the landing):
  that is V61's measured rule for a pose off the clip table, kept.

## Next

Nothing for this task.
