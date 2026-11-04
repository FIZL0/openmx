# 1 — knock heard on the strike tick (FINAL)

Task B28. 2026-11-03.

## What the original does

The props node's update (`./kit dis 0x0026f800 0x0026f980`) clears every filed object, then walks
the filed list (+0x30) calling each object's collide (vt+0x7c). It reads the count again after every
call (`lw $2, 48($16)` at 0x26f874), so an object filed during the walk is collided in the same walk.

The bike's record is first on free09's node (S197). Its body rays strike ball 2, and the struck
listener wakes the ball. The wake is the enable (`./kit fn 0x0015b790`). For a body that was asleep,
with a node, collision on (+0x54) and a collision object (+0x120), it files that object on the node
(`./kit fn 0x0026f680`, an append to +0x30). With detect-asleep (+0x6c) it also sets the probe to
mode 1 with an empty category list. So in the strike's own tick the walk reaches the ball after the
bike and collides its probe, which can see every category. The material listener on that probe hears
the bike's frame collidable. That is the ball → bike entry (material 16 → 21) that f27a1's image 2903
has queued, at the ball's shock peak (B27).

A ball the track has just loaded is asleep and not filed (the port's free09 set: `filed false`,
`detect_when_disabled true`), and f27a1's ball 2 had not been woken before. So the port's collide
pass (`PropSet::search`, run before the bike's strike) never searched its probe in the strike tick.
It first heard the bike on the next tick, when the shock peak is 0 again. Since B27 that means no
bbhit at all.

## Ruled out first

A throwaway test seeded the whole `Sim` from knock-2881, knock-2882 and f27a1-2903/2904 and stepped
it once:

- `ride.transform()` is bit-equal to the bike record's node (`rec + 0xf8`) on all four images, so
  `Sim::hear_ridden` puts the bike's frame where the original's search finds it.
- Wherever the ball was already filed (all four images are after a strike), the port hears the ball
  on the bike on the same ticks as the original: on knock-2881 → 2882 and f27a1 2903 → 2904, and
  on neither knock-2882 → 2883 nor 2904 → 2905. So the gap is only the tick that files the ball.

## What I did

- `Collection::hear_filed_by_strike(k, rays)` (prop_world.rs): the object's ray probe against the
  ridden bikes' frames (`flight_search::bikes_struck`), at its node and with its categories as the
  wake left them. Any hit goes on `heard` as `(k, Struck::Bike(j))`. It only hears: no contact, wake
  or activation, the same as F27a2's hearing.
- `PropSet::strike` (prop_contact.rs) calls it for each struck object that the strike filed, that is,
  one not filed before `struck`. An object that was already filed was searched in the collide pass
  and has been heard there, so it is not heard twice.

## How it was proved

- `tests/knock_launch.rs a_struck_ball_is_heard_in_its_strike_tick`. No image before the strike
  exists, so the test builds a stand-in: image 2903's seeded ride, with ball 2 put back as free09
  loads it (asleep, unfiled, at its spawn place). Then one `Sim::step`:
  - the bike's rays strike the ball and file it;
  - its heard list is `[(0, Bike(0))]` in that tick (with the call switched off it was `[]`, checked
    by hand);
  - the same step with no bike frame heard gives the same ball position, velocity, shock peak and
    bike velocity, bit for bit. The hearing moves nothing.
- `./kit check -- -p game-sim --test knock_launch` and the full `-p game-sim` run: see below.
- The shell's ride, F27a1's line: `research/f27a1/ours.sh` (one run, kept in the scratchpad and not
  committed). The 48 kHz stream was resampled to 22050 Hz stereo with ffmpeg, and
  `Impact\ball.vag` decoded by the shell's `audio::Library` in a throwaway test (deleted). Then
  `match.py --pair`: bbhit at 2.493 s, NCC 0.938, gain 0.481 / 0.483, one voice (the one-copy fit
  leaves 12%, two copies do no better). Nothing else is above NCC 0.23. The original's capture had
  one voice at gain 0.494 / 0.489 (F27a1). After B27 the port's best was NCC 0.145, i.e. no knock.

## Not verified / not 1:1

- **The strike tick itself, against the original.** There is still no RAM image or capture from
  before f27a1's strike (2902), so the original's queue on that tick is read off the stale queue in
  image 2903 (B27), not off a node update the oracle ran. The test's strike tick is a stand-in: the
  ball at its spawn place under the next tick's bike.
- **Only the bike's body-ray strike files and hears.** The other wakes that happen inside the
  original's collide walk also file an object that the walk then collides: a prop's rays on another
  prop (`prop_props`) and a wreck bike's (`prop_wreck`). They do not hear the ridden bikes in the
  tick they wake. Wakes after the walk (the wheels standing on a prop, the ragdoll) are right to
  wait for the next tick.
- **The just-filed probe's own contacts** (terrain, scene, the bike's frame) are still left to the
  next pass. The original files them in the strike tick, after the bike's mirror. That is a
  physics change. It goes with B29 (the probe's contact on the bike's frame) and is filed as
  B32, not done here.
- **Order in the queue.** The new entry goes at the end of the struck collection's `heard`. In the
  original it is at the end of the whole walk, behind every other filed object's entries. The shell
  joins collection by collection, so with another collection filed after this one the order differs.
  Order only matters for `drop_pairs` on a tie.
- **The level in the shell.** The match gives the same gain as the capture (0.48 against 0.49), but
  the port's level on the strike tick (its own ride's shock peak) was not logged next to the
  original's 67. One run only; the stream is wall-clock, so a voice two ticks apart could merge.
- **Split seats and `step_all`** strike through the same `PropSet::strike`, but no multi-rider strike
  was run.

## Next

Nothing. The physics side is B32.
