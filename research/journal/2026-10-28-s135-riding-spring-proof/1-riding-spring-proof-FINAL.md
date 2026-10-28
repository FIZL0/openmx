# 1 — riding spring proof (FINAL)

Task S135. 2026-10-28.

## What I did

- `crates/game-sim/tests/riding_spring.rs` (new, test only). For each of S74's nine put-backs it takes a
  fresh `ragdoll_riding::Riding`, steps it from the first held tick on the capture's request, `dt` and
  animated skeleton, and compares its 18 part nodes against the original's node world rows
  (`bones_watch.py` node<j> +0x80..+0xbf). The comparison runs every tick from T+2 through the 3 s ghost
  and on into plain riding, up to the capture's end or the next wreck. The five captures' opening
  plain-riding stretches are compared the same way after a 30-tick settle.
- No change to `ragdoll_body.rs` or anything drawn: the port already matched once it was fed the right
  targets. The drawn rider stays the clip pose (user, 2026-10-24), and `OPENMX_RIDING_RAGDOLL` stays
  opt-in.
- The captures are copied from s3's `context/s74` (git-ignored). The test skips without them, or
  without the iso for free09's terrain.

## What the original does

- **Skeleton from S74's joints.** The node hierarchy (parent of node 0..21) is
  `-1,0,0,0,1,4,4,4,6,8,9,10,7,12,13,14,2,16,17,3,19,20`. It was found by best fit on the held ticks;
  every held tick's 22 composed nodes are within 1e-2 ft of the captured world rows, and the test asserts
  this.
  - Each node's local rotation is its joint quaternion.
  - Only the root takes the joint's translation. Every other node has a fixed offset under its parent.
  - The root hangs under the bike node's rows (`bike.node` +0). That is not the request's frame (+0x50),
    which differs by about 0.07 ft.
  - Nodes 11, 15, 18 and 21 are not parts.
- **In mode 1 the world rows are still the composed local rows (within 0.006 ft).** So the riding
  write-back writes the nodes' local rows, and the hierarchy composes them as usual.
- **The spring's targets hang under last tick's bike node.** The part sync and the animation pose the
  skeleton before this tick's bike update moves the bike node. Composing the targets under the current
  bike node puts the fast rides 0.33–0.75 ft off on average. Composing under the previous tick's node
  brings the mean to 0.004 ft.
- **The two words the mode machine reads come from last tick.** `body+0x680` (posed) and `+0x684`
  (latched) are written by the vehicle animation, which runs after the body's update inside the physics
  update. The test feeds them and the clip word (`body+0x8f0`) from the previous row.
- **The ground is free09's.** S66's states are all free09, including the ones named free06 (memory:
  motocross-states-are-all-free09). Riding the free06 captures on free06's terrain put the feet 20–30 ft
  off. On free09's terrain (`Tracks\free09.res` → `.ps2.trn`, `world::Ground`) they match like the
  others.

## How it was proved

This is a save-state capture compared tick by tick, using S74's five bones captures in `context/s74`:
2,333 compared riding ticks × 18 part nodes = 41,994 node comparisons. Run it with
`./kit check -- -p game-sim --test riding_spring -- --nocapture` to print per-stretch lines.

| stretch | ticks | worst position | worst turn (per row lane) | mean |
|---|---|---|---|---|
| entry, T+2..T+5, all nine put-backs | 36 | 0.246 ft (free06-ride 3049, part 8) | 0.082 | 0.105 ft |
| settled, the seven clean put-backs | 1,554 | 0.079 ft | 0.093 | ≈0.003 ft |
| settled, the two put-backs straight off a wreck (free06-down 3060, free09-down 2666) | 88 | 0.157 ft | 0.213 (arm, node 14) | ≈0.03 ft |
| openings after a 30-tick settle | 655 | 0.056 ft | 0.042 | 0.004 ft |
| all settled | 2,297 | 0.157 ft | 0.213 | 0.0043 ft |

The bounds in the test:

| window | position | turn | mean |
|---|---|---|---|
| entry | 0.3 ft | 0.15 | — |
| settled | 0.2 ft | 0.25 | 0.01 ft |

For scale, steady riding sits around 0.002 ft, about 0.03 in. The body's mode (held/riding) matches the
original's on every stepped tick, and the test asserts this.

## Not verified / not 1:1

- **The riding entry (T+2..T+5) is not matched.** At T+2 the original's bones sit about 0.13–0.15 ft
  behind the animated target, while the port's sit about 0.02 ft from it. The gap closes over three or
  four ticks. Composing the held ticks under the current bike node instead did not help. The cause is
  not found: possibly the entry's measured velocity or a tick of the old pose that the original keeps.
  The test only bounds it, at 0.3 ft.
- **The two put-backs that come straight off a wreck start from a fresh `Riding`.** The original carries
  the wrecked ragdoll's state into them, and the arms stay up to 0.21 per lane / 0.16 ft off for about a
  second. Not proved; the settled bound (0.2 ft / 0.25) is set by them.
- **Not bit-exact.** Every comparison has a tolerance. The port is fed through f64 glue (the request
  frame inverse, `under`), so float-for-float is not expected.
- **World:** terrain only. There are no props or scene meshes, and the request's bike collider is a
  placeholder: riding only reads that it is there. A foot on a prop would not be caught.
- **The openings start mid-ride with no body state,** so they are compared only after a 30-tick settle.
- **posed / latched / clip are taken from the capture's previous row,** not from the port's vehicle
  animation. This proves the spring, not the words' source.
- **The free06-named captures are free09 states,** so free06 itself is not covered.
- **No draw change,** by the user's ruling. The proof stays a test only.

## Next

- If the entry gap matters, compare the original's per-part velocity at T+2 with the port's
  `outside.pose` measurement. That needs a capture of the part bodies' velocities (RAM, not nodes),
  which S74 does not have. Filed as S200.
