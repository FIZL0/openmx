# 1 — the wreck entry a tick late (FINAL)

Task B19. 2026-11-03.

## The proof first

- `tests/replay.rs b19_riding` reads the human's body actor out of an R14-kind RAM image as the
  port's `Riding`:
  - the actor is vtable 0x00458eb0 with 18 parts, and the one whose main bone is nearest the bike;
  - it reads the bodies, joints, pins, collisions and flags as `s273_doll` does, plus the actor's
    mode, pose and hold words;
  - it builds `Holds` from the request (actor +0x488: frame +0x50, hands +0xb0/+0xc0, feet
    +0xd0/+0xe0) in the bike model's frame.
- `b19_ride(dir, name, ticks)` seeds that body and those holds into S211's ride (through the new
  `s211_ride_seeded`), then records each tick's bones. On R14's south-bones image the seeded bodies
  equal row 0 exactly.
- Mean / worst bone position off the csv, and worst velocity off. Port tick t is row t + 1; the
  verdict (reason 4) is tick 2.

  | entry | tick 2 | tick 3 (entry row) | tick 11 |
  |---|---|---|---|
  | HEAD: entry on the verdict's tick from the tick before's bodies | 4.35 ft | — | 41 ft |
  | H1: the verdict's tick rides, entry at the end of tick 3, no step | 0.26 / 0.49 | 4.12 | ~4.0 |
  | H2: H1 plus one more riding step before the entry | 0.26 | 1.27 | 8.2 |
  | H2, with the riding bodies' velocities kept | 0.26 | 1.27 | 3.2 |
  | **H1 plus the wreck's first step after the entry (landed)** | 0.26 / 0.49, 12.2 ft/s | 0.40 / 0.73, 9.1 ft/s | 1.59 / 2.22, 10.1 ft/s |

- The landed order is S110's: run2's hook on `rider_body_wreck_enter` shows the update going on past
  the entry (its first wreck step) before the row is written. The entry's pose reads the nodes the
  verdict's riding step left (S70). On the verdict's tick the port's main bone moves at
  (6.24, 15.07, 106.70) ft/s against the original's (6.14, 15.08, 106.70), which is the 0.8 carry
  B16 saw.

## The change (crates/game-sim/src/rider.rs)

- In the verdict branch, a rider with holds and a placed riding body (and no put-back this tick)
  gets no ragdoll yet. Its riding body is kept, and `riding_tick` still steps it on that tick
  (`entry_waits` counts as up).
- At the end of the next tick (step 9, where the riding step would be), `enter_late`:
  1. calls `Riding::hand_over`, then `Down::enter_from` against the wreck body as that tick's physics
     left it;
  2. calls `voice_entry` (both draw from the idle pool's generator, as before);
  3. calls `step_posed_among` once, on the tick's skeleton, the bike in, and the lent props.

  `out.separated` is raised if that step lets go of the bars.
- Riders without holds or a riding body (fixtures, online sessions, the riderless S211/R14/B15 runs)
  still enter on the verdict's tick, off the skeleton.

## Tests updated

- `the_windmill_wrecks_the_bones_with_the_bike` (R14) now proves the late entry with the riding body
  seeded:
  - on the throw's tick the port's main bone is carried within 0.5 ft/s of the original's;
  - on tick 3 it is within 0.5 ft of the original's (0.27).

  The riderless ride's entry at rest is kept as the stand-in.
- `the_windmill_strikes_the_ports_bones_left_at_its_face` (B15) keeps its riderless strike, then
  rides B15's image with the riding body seeded:
  - the bones are down exactly on the rows where the original's riding word is 0 (from row 5);
  - no bone goes over 200 ft/s in 40 ticks (fastest ~140), and nothing is put back;
  - the worst bone is under 1.5/2.0/2.0 ft while riding and 4 ft on the entry's row (3.36).

  From there the bones part at ~2.2 ft a tick (filed as B22).
- `tests/ragdoll.rs the_wreck_enters_from_the_riding_bodies` (S110's port side):
  - the verdict's tick now leaves the held rider riding, with no bones;
  - on the next tick its bones equal, bit for bit (position and velocity), an entry rebuilt from
    the riding bodies the verdict's tick left, stepped once;
  - the bare rider still enters on the verdict's tick from the skeleton.

## Checked

- `./kit check -- -p game-sim --no-fail-fast`: all green but two:
  - `fly_and_collide the_drawn_rider_leans_as_the_original_through_the_turns` ((1952, 282) hard-turn
    ticks against 280). It fails the same way at HEAD 1189b61e in a scratch worktree; noted on B18.
  - `trick the_humans_tricks`, the known red on C6's capture.

  `replay`, `ragdoll`, `ragdoll_riding` (S110's bit-exact entry against the original) and
  `ragdoll_entry` pass.
- `./kit check -- --workspace --exclude game-sim --no-fail-fast`: 3288 tests passed. This includes
  game-render's whip_tricks and body tests and openmx's snapshot and netplay hashes. The pending
  entry is derived from the encoded riding body and wreck state, so snapshots needed no new field.

## Filed

- B22: from B15's south3 image the port's bones leave the late entry at the bike's velocity,
  (8, 16, 133) ft/s against the original's (41, −39, 122), and part from it at ~2.2 ft a tick.

## Not verified / not 1:1

- Riders without a riding body (no holds: fixtures, online sessions) still enter on the verdict's
  tick, from the skeleton. This is a stand-in.
- On the tick the entry waits, `Rider::main_bone` and the wreck voice's frame read the crash, not
  the bones (there are none yet). Not checked against the original.
- The seeded holds are the image's request, constant over the ride. The original's request moves
  with the drawn chassis each tick, and the test has none to follow it.
- The bones still drift from the original's on R14 (~0.15 ft a tick) and on B15 (~2.2 ft a tick,
  B22). The throw draws cannot be checked: the csvs carry no generator state.
- Not looked at on screen.
