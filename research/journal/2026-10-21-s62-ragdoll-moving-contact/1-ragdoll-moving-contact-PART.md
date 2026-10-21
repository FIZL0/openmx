# 1 — ragdoll moving contact (PART)

Task S62. 2026-10-21.

## What I did

- **Ported the bone push-out against moving things** in `crates/game-sim/src/ragdoll_contact.rs`. This
  is purely additive: the old `push_out` now calls `push_out_against` with a still other.
  - `Against` is the contact's other side: `Still`, a `Mover`, or a collision `Other`.
  - `moving(at, against)` is that other side's velocity at the contact point. It reuses
    contact.rs's `Mover`/`Other` relative velocity, which the bone code shares.
  - `push_out_against` pushes the bone's velocity relative to that velocity.
  - The `Scene` trait gained two methods, `other(i)` and `velocity(other, at)`. Both default to "no
    other, still", so the existing scenes behave as before.
  - The solve hands each contact's other to the push-out. The sweep uses the last scene contact's
    other, as the original does.
  - `NodeBox` is bike_box_solve's +0x14 path: a box held in another actor's node frame. It keeps the
    original's quirk: the bone's velocity is set to the node turning the clamped *position*. The
    decompile reads it as the velocity, but the disassembly turns p.
- **Captured the wreck bike's world by chain.** `watch/s56-ragdoll.toml` gained these fields:
  - `bike_a` and `bike_b`: B-0x980..B+0xb80, where B is the request's +0x1ac. This holds the bike's
    rigid body and both of its collision objects.
  - Five absolute pools for the broad phase's list nodes. They are relinked every tick and no
    pointer chain reaches them.
- **`research/s62_wreck.py`** is S56's capture plus one late state (`--late`). `--hold N` moves the three
  early states to N ticks after the wreck.
  - Captures in context/capture/s62 (git-ignored): `front`, `side`, `back` and `tip`, 800 rows each, 0
    torn, 0 dropped. `tip` is S56's `tip-a.pad`.
  - `front3` is a second front wreck, 80 rows with states at 2630..2632. It came from loading front's
    2604 state with the same pad held, which does *not* ride the same wreck again.
  - Every state was checked against its own row: 0 lanes off.
  - Each state's RAM and scratchpad are written next to it as `.ee.bin`/`.spr.bin`. The scratch
    script that writes them is in the transcript; `./kit state ram` does the same job.
- **`crates/game-sim/tests/ragdoll_bike.rs`** is the oracle replay over the s62 captures. It is a new
  file rather than an edit to ragdoll_wrecks.rs, which S61 is editing in parallel. It reuses S56's
  method:
  - the original's `rider_body_update` runs in the Ee;
  - the request, the bike block and the pools are laid in each tick;
  - the rest of the game's rng draws before the update are found by trying each count;
  - every bone's six quads are compared.
- Tried 19 pad scripts for a *truly slow* tip-over (`research/s62/tip-*.pad`). None worked; see below.

## What the original does

- `./kit fn 0x001f6c40`: the push-out measures the bone's velocity against the contact's other
  (+0x70), in four cases:
  - **null**: the other is still.
  - **a mover** (+0x7c, class +0x1b0 == 2): the velocity comes from its vtable +0xbc (spin) and +0xb8
    (velocity), at the node +0x254 about the origin +0x190.
  - **a collision object with a body** (+0x80 → +0xf4): q, centre, rates and velocity come from the
    body.
  - **a collision object on a vehicle**: the chassis +0xf0 and node +0xf8.

  This is contact.rs's relative velocity line for line. It also writes the object's +0xa0.
- `./kit fn 0x001f5b40`, the +0x14 path:
  1. The bone is clamped in the other actor's node frame.
  2. The body's position is placed through the node.
  3. The body's velocity is `rotate(node, p)`, where p is the clamped local position. That is a bug
     in the original, and it is kept.
- The wreck bike's object hangs off the request's +0x1ac (B):
  - the rigid body is at B-0x780;
  - the collision objects are at B-0x580 and B+0x670.
  - S56's missed reads 0xdfd580/690/6d0 are that body's pos, quat and rates.
- A wreck lasts ~79 ticks to the put-back (wreck reason back to 0). The bones go still at the put-back.

## How it was proved

- **Function oracle on generated cases** (`tests/ragdoll_contact.rs`, 3 new tests). The port is
  compared against the original's push-out, solve and bike-box functions in the Ee, word for word.
  - `the_push_out_against_moving_things_matches_the_original`: every kind of other, >1000 each
    (1318..1959), plus 2562 swept.
  - `the_solve_against_moving_things_matches_the_original`: 11 413 moving contacts, and 2865 sweeps
    after one.
  - `the_bike_box_in_another_frame_matches_the_original`: 7197 held, 2159 free, 644 off.
  - The counts were checked by raising the asserts until they failed.
- **Save-state capture + oracle replay** (`tests/ragdoll_bike.rs`,
  `the_wrecks_replay_through_the_bike`). Bit-exact on every lane of 18 bones × 6 quads:

  | wreck | replay | first tick | exact through | bike body read |
  |---|---|---|---|---|
  | side | whole wreck and the put-back | 2607 | 3146 | 2615 |
  | back | to the capture's end | 2477 | 3179 | — |
  | tip | to the capture's end | 2472 | 3179 | — |
  | front | 25 ticks into the wreck | 2602 | 2627 | 2620..2627 |
  | front3 | through the put-back | 2630 | 2687 | 2631 |

  - For side, back and tip, the run goes through the bike-contact ticks where S56 parted (2613, 2491,
    2482), through the put-back, and on.
  - At side 3146 the riding body's rows start to alternate ±0.11 between ticks.
  - S56's replays parted at front 2608, side 2613, back 2491 and tip 2482.
  - Checked that the test does not skip: an impossible expectation on the last wreck made it fail
    there.

## Not verified / not 1:1

- **The port does not replay the wrecks yet.** The task's "then the port the same" needs the port's
  whole per-tick body update, which is S61's mode machine. S61 is in progress in another slot, editing
  ragdoll.rs, ragdoll_ride.rs and ragdoll_wrecks.rs. The new contact paths are proved only function
  by function on generated cases, not in a whole tick of the port.
- **Front parts at 2628, 26 ticks into the wreck, cause not found.** Every lane is off; bones 0 and
  15–17 are 0.4–0.7 lower in the original. What was ruled out:
  - There is no push-out in the tick.
  - Nothing new is read that the tick did not write or the capture did not lay in.
  - The request, the bike or the pools from the previous row do not mend it.
  - Running the update twice does not mend it.
  - Its rng draws (29) are ordinary.

  A front3 wreck replays through its end. A state saved at front's 2627/2628 would settle it, but
  front cannot be ridden again: the pad is time-based, and loading a state with the same pad does not
  repeat the ride.
- **The moving push-out paths are barely reached in the replays.** In all five wrecks the scene
  push-out ran once, against other = 0. The bones reach the bike by reading its body (front 9 ticks,
  side and front3 1 tick each), not by a push-out against it. The `Mover`/`Other` paths are proved on
  generated cases only, not on a captured tick.
- **No truly slow tip-over.** 19 pad scripts tried standing, crawling, braking and leaning, on flat
  ground and on the slope.
  - The game holds a slow or stationary bike upright. There is no ground roll limit, and the ground
    pitch limits were not met even at ~60° on the slope.
  - Only two pads wrecked: m (front, 33 ft/s) and t (reason 4, 57 ft/s).
  - The tip capture is S56's tip-a: reason 3, entered at 32 ft/s.
- The other actors (AI riders, their bikes) are not captured. None of these wrecks reads them; a wreck
  into another rider would.
- The bike_box +0x14 path (another actor's node frame) never runs in these wrecks: +0x14 is 0 live.
  It is proved on generated cases only.

## Next

- After S61 lands: drive the port's ragdoll tick from the s62 captures (`tests/ragdoll_bike.rs`'s
  `lay_in` gives the world) and assert it equals the Ee tick by tick over the same spans. The port's
  `Scene` must answer `other`/`velocity` from the bike block (B-0x780 body), which `NodeBox`/
  `Against::Other` take.
- Front 2628: capture a new front wreck with `--hold` near where it parts, and diff the RAM the tick
  reads against the state's.
