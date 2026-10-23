# 1 — limb hold (PART)

Task V61. 2026-10-23.

## What I did

- **Found the weight's writer.** The weight V10's "limb controller" reads off its bone at `+0x10` is
  the bone's inverse mass. The body's mass setter writes it beside the mass. The controller itself
  is not a limb solve: it is one of the riding ragdoll's four hold pins. Its solve is the pin's
  one-body solve, which S51 already ported (`game_sim::ragdoll::Pin::one_body`) and proved over
  10 000 generated cases (`crates/game-sim/tests/ragdoll.rs`,
  `the_pins_one_bone_solve_matches_the_original`). There is no clavicle or wrist solve to port: the
  original has no inverse kinematics on the drawn skeleton at all.
- **Found when the original holds.** While riding, the rider is drawn from the riding ragdoll
  (S70, S109), and its pins pull the hand and foot bodies to the grips and pegs. But the ragdoll is
  not placed, so not stepped and not drawn, whenever the body's mode machine is `Posed`. That is
  every gesture, every no-hander and a backflip's hold. Then the clip is drawn exactly as it was
  written, every limb included. This is the "hands don't always grab" difference: the port held the
  hands on through tricks, and the original lets go of everything.
- **Wired the gate into the shell's riding ragdoll.** `Riding::posed_off_the_table`
  (`crates/game-sim/src/ragdoll_riding.rs`) gives the actor the body machine's `Posed`, and
  `ride_ragdoll` (`crates/openmx/src/main.rs`) calls it every tick before the step. With
  `OPENMX_RIDING_RAGDOLL=1` a gesture or a backflip now draws its clip. This is S111's "Taunt's arm
  never comes out" and "a backflip folds the body".
- **Replaced the port's distance rule** in `crates/game-render/src/rider.rs`. I deleted `holding`,
  `HOLD_WITHIN` and `LET_GO_AT`. `hold_on`'s strength is now `Body::hold`, which `Body::ride` sets
  from the machine's mode through the new `holds_in`: 1 while `Riding`, 0 otherwise. The clip that
  the shell draws by default therefore lets go exactly when the original does. `chassis.rs`'s
  measure uses the same strength.
- **Kept `aim`, against the brief.** It is the stand-in solve for the default draw, which is the clip
  and not the ragdoll: the riding ragdoll is off by default (user, 2026-10-23: "our guy is super
  floppy"), and turning it on is S111's call. Deleting `aim` now would put the hands back off the
  bars on every riding frame of the default draw. So I filed **V63**: delete `hold_on`, `aim`,
  `Body::hold` and the chassis measure once S111 turns the riding ragdoll on. I also added a note to
  S111 with what V61 found.
- **Captured recording2 twice** on the original (PCSX2 copy 3, `capture/play.sh` with
  `capture/holds.toml` and `capture/gate.toml`, S109's pause-before-Play recipe). Both runs cover
  frame_counter 2374–4023: 1650 rows each, one per tick, no gaps, no torn rows. They are in
  `context/capture/v61/` (git-ignored).
- **Tests.**
  - New: `crates/game-sim/tests/limb_hold.rs`, three tests.
  - `crates/game-render/tests/body.rs`:
    - the V10 sweep is now `the_stand_in_solve_puts_a_limb_in_reach_on_its_anchor`, which no longer
      encodes the distance window;
    - `a_one_hander_keeps_the_other_hand_on_the_bars` is gone, because it tested the port's invented
      rule;
    - new: `a_gesture_is_drawn_as_the_clip_wrote_it`.

## What the original does

- The four holds are the actor's pins at `+0x5d0`, `+0x5d4`, `+0x5d8` and `+0x5dc`: hand, hand,
  foot, foot, on ragdoll bones 11, 7, 17 and 14.
  - Each pin's bone is a ragdoll body, `*(actor+0x2f4) + i*0xb0 + 0xa4`.
  - The solve is `./kit fn 0x001f4550`, reached through the pin's table (`./kit fn 0x001f49b0`).
  - Its gains at `+0x100` are (1/m of the bone, 0, z, 0).
- The mass setter is `./kit fn 0x0015acc0`. It writes `+0xc = m` and `+0x10 = 1/m`, and is called
  through the body's vtable.
  - The riding entry sets each bone to its part record's ride mass (`Mass`).
  - The wreck's entry sets the ragdoll mass (`RagdollMass`).
  - Leaving the wreck runs neither mode's entry (the quirk `Actor::mode` already has), so the
    wreck's masses stay until the next riding entry.
- The ragdoll is placed only when it is riding:
  - "placed" (`+0x2ac`) is the riding entry's;
  - entering `Posed`, `Gate` or `Held` clears it;
  - the physics update (`./kit fn 0x0030f6e0`) steps the world only when it is placed, active and
    not reposing;
  - the drawn skeleton is the ragdoll's nodes only after a step.
- The mode chain is the same in the actor and in the port's `rider_anim::Body::mode`. `Posed` is
  `from_table` (`+0x680`) or `holding` (`+0x684`).
  - At that point of the tick, `from_table` still holds what the previous tick's vehicle animation
    wrote.
  - `holding` has just been raised if a backflip is running.

## How it was proved

- **The solve**: the existing 10 000-case function oracle of the pin's one-body solve
  (`tests/ragdoll.rs`). I did not touch it.
- **The weights**: `limb_hold.rs` `a_holds_weight_is_its_bones_riding_inverse_mass`. On 1131
  captured ticks where the ragdoll is placed while riding, all four hold bones carry the mass and the
  inverse mass that the port's `ride_enter` sets off the disc's `mrider.phy`, to the bit: hands 0.2
  (inverse 5.0), feet 0.15 (inverse 6.667). On every tick a pin is on (hands 1131 and 1131, feet
  1028 and 1051), the pin's first gain is that inverse mass and its last gain is 0. The hands' third
  gain is 1.
- **The limb ends**: `a_holds_anchor_is_the_kits`. Each pin's anchor on its bone equals the port's
  kit's, to the bit, on all 1650 rows.
- **The gate**: `the_ragdoll_lets_go_while_a_pose_is_off_the_table`. On every riding tick (1499 of
  1499), "placed" is exactly not (`from_table` of the tick before, or `holding` of this tick). 368 of
  those ticks are posed: the capture's tricks include a taunt, a cancan, a lazyboy, a stalelin and two
  backflips.
- **The port follows the same rule**: `body.rs` `a_gesture_is_drawn_as_the_clip_wrote_it`. A
  gesture made in the air through `Body::ride` is posed for 80 frames. On each of them `Body::hold`
  is 0, and the palette the draw uses is the clip's own world matrices, unchanged. On every frame,
  the hold is `holds_in` of the machine's mode. The run also showed the same phase as the original:
  on the tick the gesture starts, `from_table` goes up but the mode is still `Riding`, and it is
  `Posed` from the next tick.
- `./kit check -- -p game-sim --test limb_hold` and `-p game-render --test body` both pass.

## Not verified / not 1:1

- **The default draw's hold is not the original's solve.** While riding, the clip is held on by
  `aim`, a two-turn closed form that this port wrote. The original holds through its ragdoll's pins.
  Only *when* the stand-in holds is the original's. Where the hands end on a riding frame, and how
  the body gives, is the original's only with `OPENMX_RIDING_RAGDOLL=1`, and that draw is S111's (V63
  then deletes the stand-in).
- **The stand-in's switch is a step, and was not compared.** It goes from 0 to 1 on the tick a
  gesture ends. The original re-places its ragdoll from the clip pose on that tick and its pins then
  pull. Whether its hands reach the grips within that one tick was not measured, so the hold coming
  back on can pop differently.
- **Look-around and Gate are not fed to the riding ragdoll.** The body machine's `Gate` (the start
  line, a look-around idle) also leaves the original's ragdoll unplaced. The stand-in follows it
  through `holds_in`, but the shell's riding actor is not told (`actor.idle` is not fed), so with the
  riding ragdoll on, a look-around is still held. I did not prove that `idle` is the body's
  `looking`. That was added to S111's note.
- **The feet's hold weight (gain z, the foot fade) during a look-around** was captured but not
  compared. The port's `Actor::feet` is proved by S109's tick test only on a ride with no
  look-around.
- **The gate's capture is the original's**, and so are the port's mode chain and its tests. The
  port's `from_table` and `holding` writers were proved by earlier tasks (`tests/motion.rs`). The
  port's own trick run was not compared tick by tick against recording2's tricks.
- **One recording** (recording2, free09, one bike and rider).
- **Not looked at in the shell.** The task asks for proof by code. The shell's gesture and backflip
  draws with this change were not inspected on screen, with or without the riding ragdoll.

## Next

Done as briefed, apart from deleting `aim`, which waits on S111 (V63).
