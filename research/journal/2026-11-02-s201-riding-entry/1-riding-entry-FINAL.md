# 1 — riding entry (FINAL)

Task S201. 2026-11-02.

## What I did

- **Captured the entry again.** `capture/entry_watch.py` is S74's `bones_watch.py` with more regions
  added:
  - the ragdoll words `body+0x2a4..+0x2b7` (riding, repose, placed, active, stiff);
  - the holds, the carried velocity, the bike box and the drawn bike frame, `body+0x4d0..+0x5df`;
  - each hold pin's target and velocity (`*(body+0x5d0+4k)` +0x40..+0x5f);
  - each part body's `+0x50..+0x1bf` (last position, position, velocity … rates);
  - each part body's start position and velocity, `+0x30..+0x4f`.

  This machine has no uinput pad, so `capture/make_tape.py` turns S74's ride script into a generated
  `.p2m2` (P21's layout, two fields a tick). `capture/play_entry.sh` plays it with C6's AT-SPI recipe
  and runs the watcher in `--attach` mode. PCSX2 copy 3, state slot 7 (free09). I set copy 3's
  `NominalScalar` to 1: it was 0.25, while S74 captured at full speed.
- **Captures** (git-ignored, `context/s201/`):
  - `free09-ride2.txt`: 520 ticks, 2937..3456, put-backs at 3137 and 3267, torn 0, gaps 0.
  - `free09-ride3.txt`: the same tape with the start/hold/pin regions added. Same frames, same
    put-backs, torn 0, gaps 0.
  - `free09-ride.txt`: an earlier run with no input. No put-backs, so unused.
- **Port** (`crates/game-sim/src/ragdoll_riding.rs`). Both changes are additive; nothing is renamed.
  - `Riding::step_moved(kit, posed, moved, …)`. `Riding::step` is `step_moved` with `moved = None`.
    A held tick's repose puts the bones at `moved`: the skeleton under the bike node as this tick
    leaves it. Those nodes become the tick's drawn matrices.
  - The riding entry after a held tick (`actor.mode == Held` going in) poses at those nodes, i.e. last
    tick's. It measures velocity from the drawn matrices that a still pose of the same nodes writes, so
    each bone's start velocity is `-turn(q, cofg) / dt`. No new state: the nodes come out of
    `Riding::drawn`, so the wire and hash are unchanged.
- **Test** (`crates/game-sim/tests/riding_spring.rs`).
  - `ride()` now starts a put-back stretch with `actor.prev = Wrecked`, so the held entry clears the
    placement as the original's does. Before this, the fresh `Riding` started `prev = Held,
    entered = true`. It stayed placed through the held ticks and never ran the riding entry's pose,
    which the original does.
  - `ride()` feeds `moved` (the skeleton under this tick's bike node).
  - Entry bounds: `ENTRY_AT` 0.3 → 0.06, `ENTRY_TURN` 0.15 → 0.1. New `ENTRY_MEAN_AT` 0.01 (was
    unbounded).
  - New test `the_riding_entry_starts_from_the_last_held_tick` on `context/s201/free09-ride3.txt`:
    - at both entries the port's pose from T+1's nodes equals the original's 18 start positions and
      velocities bit for bit;
    - from the put-back on, the riding body stays within the entry and settled bounds.

    It skips without the capture.

## What the original does

Put-back at T (`mode` 0 at T and T+1, 1 from T+2). From `free09-ride3`, put-back 3137:

- **Held ticks.** `placed` (`+0x2ac`) is 0. Every part body has zero velocity and rates, and sits where
  the pose puts it under **this** tick's bike node: within 0.004 ft, against 0.3–4 ft under last
  tick's. The still repose (`rider_ragdoll_pose` a1 = 1, then `rider_ragdoll_reset`) writes
  `part+0x60` = `mat4_basis(position + turn(q, cofg), …)`. That is the body centre, not the node (see
  oracle.txt, `rider_ragdoll_pose`).
- **T+2, the entry.** Each body's start position (`+0x30`) equals the position T+1 left it at, bit for
  bit. The skeleton the entry poses from is still T+1's: the entry runs in the animation update, before
  this tick's nodes are written. The start velocity (`+0x40`) is `(node − part+0x90) / dt`. Since
  `part+0x90` is the T+1 body centre, that comes to `−turn(q, cofg) / dt`: −0.18, 0.29, −0.12 ft/s for
  the pelvis and 13.5 ft/s up for a thigh. The reset copies the starts in, and the step runs from them.
  The pelvis rises 0.13 ft at T+2 while the bike sinks 0.09 ft. That rise is the "0.15 ft behind" that
  S135 saw.
- **The holds and the box are this tick's.** The hand pins' targets and velocities at T+2 are the
  request's holds and `0.8 × bike velocity` of that tick. The foot pins are off (stale targets from
  before the wreck, never read). The bike box is (0, 4.3, −1.5) ± (0.45, 0.6, 1.75) in the drawn
  frame. Nothing there is a tick stale.
- **Check.** Starting the port's T+2 step from T+1's positions with the captured start velocities
  (the experiment, before the port computed them) brought the entry from 0.23 ft to 0.010 ft worst.
  The port's own computation gives the same starts, and the same numbers.

## Results (`cargo test -p game-sim --test riding_spring -- --nocapture`)

| stretch | ticks | worst | worst turn | mean |
|---|---|---|---|---|
| S74 entries, T+2..T+5, nine put-backs, before (S135) | 36 | 0.246 ft | 0.082 | 0.105 ft |
| S74 entries, after | 36 | 0.044 ft (free09-down 2671, foot) | 0.078 | 0.0043 ft |
| — the seven not off a stick-held `down` run | 28 | ≤ 0.006 ft | ≤ 0.009 | ≈ 0.0015 ft |
| S74 settled, after | 2,297 | 0.149 ft | 0.1995 | 0.0041 ft |
| free09-ride3 entries (2 put-backs) | 8 | 0.010 ft | 0.013 | 0.0021 ft |
| free09-ride3 settled | 308 | 0.053 ft | 0.084 | 0.0032 ft |

The two `down` entries off 0.044 ft are the same two put-backs whose settled arms are worst (0.15 ft,
S135: straight off a wreck, a fresh body here).

## Not verified / not 1:1

- The entry's bones are read back out of the drawn matrices (`basis` rows: normalised forward and up,
  the node position). The original re-reads the nodes themselves. The capture test matches bit for bit
  on free09-ride3, but other poses could differ by an ulp.
- Only the put-back's Held → Riding entry was measured. The Posed → Riding entry, the opening and the
  wreck entry are unchanged. Whether the Posed → Riding entry also poses from last tick's nodes is not
  known.
- The start rates (`+0x140`) were not captured. The port's come out ~0, from the same orientation on
  both sides. The match without them suggests they are near zero.
- The test harness, not the shell, feeds `moved`. The shell calls `Riding::step` (`moved = None`), so in
  the game a held tick's repose and the drawn nodes are the update's skeleton, as before S201. Only
  the entry's start (last tick's nodes and the centre-of-mass velocity) reaches the game. Not watched
  in the shell. The riding ragdoll stays off by default (`OPENMX_RIDING_RAGDOLL`).
- Not run against a second track, or with the stick held through the entry, beyond S74's `down` runs.
