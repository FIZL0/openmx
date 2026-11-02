# B8 — the struck HasPhysics truck, measured already done

2026-11-02. Task: `./kit plan B8`.

B8 was filed before S186 and S186a landed, and those two tasks have already done all of it. S186 added
the struck truck itself: `game_sim::physics_car` wakes the body, stands the meshes down and puts the
truck back after `ResetTime`. S186a fixed the one-tick-early contact: the `HasPhysics` cars now update
after the bikes (`Traffic::frame_late`), where the path cars update before them, and the body rays'
struck listener wakes the truck. The 'not here' comment in `src/traffic.rs` that the brief points at
is gone. No code changed in this task.

## What I measured

I copied S219's captures and RAM images (flank, slow, rear) from slot s2 into this slot, then ran
`cargo test --release -p game-sim --test replay -- --nocapture a_path_car_into_a_parked_bike
a_bike_meets_a_moving_truck`. Both tests passed, and all 8 S207/S219 rides ran with none skipped:

- **S207 ctruck**: parts on tick 36, past the brief's 35. The port's wreck is `None`, the same as
  the original's (it was `(172, 1)`). The row it parts on is the strike's push: vel.x 9.53 in the
  port against 2.97 in the original.
- **S219 rear**: every lane agrees to tick 41. Both bikes wreck `(42, 4)`; the brief had the port
  wrecking on 41. It parts on tick 42, again on the push: vel.z 15.48 against 24.73.

The tables in `S207_RIDES` and `S219_RIDES` already hold these numbers.

## Not verified / not 1:1

- The push on the strike tick still parts in both rides. The original's bike meets the woken truck's
  f.col as a body during the rest of that frame, and the port does not file it there. That work is
  S251, which is still open, so I left it there.
