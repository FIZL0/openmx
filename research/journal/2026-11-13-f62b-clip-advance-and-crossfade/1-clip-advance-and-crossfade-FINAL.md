# 1 — clip advance and crossfade (FINAL)

Task F62b. 2026-11-13.

## What I did

- `crates/game-sim/src/handling/anim.rs`: added `set_clip` (the controller's vt `+0x90`), `start` (the
  wreck states' clip start with its crossfade setup), and `advance` (the controller's vt `+0x88`: run the
  clip on by `dt`, end/loop it, sample it into the bones, and while fading mix it with the saved pose or
  with the old clip). Helpers `sample`, `fade_from_saved`, `fade_from_clip`, `cross_position`,
  `store_rotation`, `counts`, `own`, `raw`.
- `crates/game-sim/tests/mxvatv_bike_ride.rs`: the rider leaf now serves `Call::ClipStart` with
  `anim::start` (through a `LeafAnim` that answers the saved-pose allocation with `Call::SavedPose`) and
  `Call::ClipAdvance` with `anim::advance`. Both arms first assert that the controller's vtable holds the
  original's entries. The flip replay's census moves both to `=`. The anim oracle
  (`the_anim_controller_calls_are_the_originals`) gains Advance and Start cases.
- The rider.rs and anim.rs module docs now say where these calls went. `research/oracle.txt` `# F62b`.

## What the original does

`./kit fn 0x0013c7b0` (advance), `./kit fn 0x0013db90` / `./kit fn 0x0013d350` (the two fades),
`./kit fn 0x0013ea90` (start), `./kit fn 0x0013e960` (set clip). Words and arg orders: oracle.txt `# F62b`.

- **Advance.** Time += dt. The end test is `length < t`; it either loops or marks the clip ended (+0x24c).
  - Not fading: sample every channel into the bones.
  - Fading: the clock +0x68 runs on, u = ((clock − start) − dt)/span, w = smoothstep(u).
    - With no old clip (+0x40 == 0), each bone is slerped from its saved rotation to the sampled one by w.
      Positions become p + (saved − p)(1 − w).
    - With an old clip, the old clip is sampled at the clock, then slerp(old, new, w) and the same
      position mix. Channel k of the new clip stands in where the old clip has fewer channels.
    - When the clock passes the end, fading clears.
  - Rotations go into the node (+0x2d bit5) or the bone's +0x20 slot. Positions are placed when bit4 is set.
- **Start.**
  - A span of 0 only sets the clip.
  - Otherwise: start = clock = now, end = now + span, the old clip is remembered. If there was no clip, or
    a fade was already running, the old clip is cleared and the pose saved (`anim::stop`). Then the clip is
    set and fading is set.

## How it was proved

- **Function oracle:** `the_anim_controller_calls_are_the_originals` in tests/mxvatv_bike_ride.rs. It ran
  14,000 generated cases across all the anim calls, against the original's code in the Ee interpreter, with
  RAM compared after each.
  - Advance cases cover: plain, ended, looped, fade from saved, fade from clip, fade done, node turn and node
    place.
  - Start cases cover: span 0, stopped (no clip), stopped (fading), and from a clip.
  - Every path tag is required to be hit. It passes.
  - It caught one difference: the original keeps a denormal quaternion word raw when |cos| ≥ 1. Fixed with
    `raw()` rather than `Vf::from_bits`.
- **Replay:** C15 flip-1 (465 frames) with the rider leaf on the ports: `flip_replays_with_the_rider_leaf_on_the_ports` passes ("TESTS PASSED: 1 tests"; the C15 RAM image,
  flip-1.csv, nat05.res and mx_race250p.phy were all present, so it did not skip). The leaf serves "= clip
  advance" 68 and "= clip start" 2. Of the advances, 8 fade from the saved pose, 8 from the old clip and 52
  are plain. `leaf_census` holds: the tail's rider ran in the interpreter only the leaf's
  remaining outside calls, which are rand 8, cue busy/playing 1 each and sound busy/play 1 each. `leaf_off`
  is only `0x851fec..0x851ff0` (the cue's write and the ground query's ring), as before F62b.

## Not verified / not 1:1

- **The per-bone list branch (ctrl +0x2c bit7) is not ported.** The rider's +0x2c is 0x60 on every C15
  frame. `advance` has a `debug_assert!` on it, and the oracle clears that bit.
- **Advance's mirror, mask, alternate-pose and events arguments are not ported, nor start's keep-time
  (a2 ≠ 0) path.** C15 always passes 0, and the ports take no such arguments.
- **Node turn/place (+0x2d bits 5/4) are proven by the oracle only.** On C15 the rider has bits 2/3 (0xd/0xf),
  so on the replay rotations go to bone +0x20.
- **The stale stack quaternion starts at zero in the port.** As with F43a's play/blend: a rotation channel
  that is off or keyless reuses the previous channel's value, and the first one reads stack garbage in the
  original. The oracle and C15 never hit this with a garbage-dependent result.
- **The replay never reaches the saved-pose allocation (`Call::SavedPose`), start's span-0 path, or the
  ended/loop paths at a fade boundary.** All of them are covered by the oracle.
- Nothing was taken from opentennis.

## Next

Nothing left for F62b.
