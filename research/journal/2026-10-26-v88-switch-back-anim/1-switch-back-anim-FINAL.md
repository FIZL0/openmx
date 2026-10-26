# 1 — switch back anim (FINAL)

Task V88. 2026-10-26.

## What I did

- **Found what the body plays.** I logged the shell's two calls a tick for a seat: `Garage::step`, then
  `Body::ride` while the bike is up. Each tick I printed `Rider::body_frame`'s mode, want/state, the
  request's cut flag, the look timer, the clock's cross, the blend and `Body::posed`. That ran on the
  disc's biplane, the stock bike and thq1's body.
  - **Bike first, then plane, then bike** (throttle opened before the switch): the body rides from tick 0
    after the swap. It is in mode `Riding`, on a `Four` grid blend with ramp 0. The cut flag was
    lowered for good the first time the throttle opened. Nothing in the switch (`teleport_to` →
    `place_on`, `teleported`) raises it again or touches the body. No fade from the pose the plane left
    was found either: the body is simply not stepped while the plane is up, and it carries on from
    there.
  - **Session started in the plane** (`--vehicle biplane`, or a ride setup that picks the plane): after
    the swap the body sits in mode `Gate` with `cut` up. It plays state 53, a get-ready idle (hands off
    the bars), through the `Free` blend's player path. The rider is fresh off `Rider::new`, which raises
    `cut` ("a rider is put on the bike at the gate"). Only the throttle opened during a get-ready idle
    or a start pose lowers it. Once the idle ends, the gate breathes for 3.5 s and the throttle cannot
    cut that short. So the user's "first few seconds" is the gate: how long depends on when they open
    the throttle.
- **The fix is the lead's first half.** I added `Rider::no_start_line` in game-sim (it lowers
  `body_request.cut`). The shell's `new_rider` calls it for every local seat, and online seats are
  clones of seat 0's start sim, as are seats that join mid-ride. `same_ride.rs`'s `start()` and
  `peer::sims` call it too, so a headless peer's state matches a shell's. `Rider::new` is left raising
  the flag: game-sim's gate tests and the snapshot bytes of every fixture start from it.
- **I did not take the lead's second half** (the main checkout's uncommitted `unfaded` first pose in
  game-render's `Body::posed_by`). With the gate gone, a fresh body's first frame is the riding grid at
  ramp 0. Its default state is already `TURN_RIDING`, so there is no restart and nothing fades out of
  the bind pose. Running the tests with and without that change gave identical results. It stays that
  other session's work.
- Tests in `crates/openmx/tests/switch_back_anim.rs`:
  - `a_ride_started_in_the_plane_hands_over_a_riding_body`: plane first, two seconds, swap, then five
    seconds idle and five seconds on the throttle. Every tick has mode `Riding`, the flag down, a grid
    blend, `posed`, and a ramp that never rises and ends at 0 (ramp 0 from the first tick). The rolling
    body moves on more than half the ticks. A rider built without `no_start_line` is still at `Gate`.
  - `a_round_trip_through_the_plane_draws_the_bike_that_never_left`: two seats on the same bike, both
    set down. One takes the plane for two seconds and comes back, the other stays. For five seconds after
    the swap both have the same mode, flag and blend kind, and every drawn joint (bike space) is within
    0.1 in of the other; the worst seen was 0.0024 in.
  - Proved red without the fix: with `no_start_line` dropped from the test's seat, the first test fails
    "idle, tick 0 after the swap: the body is at a start line", and the second fails at 0.5958 in on
    tick 0.

- opentennis: nothing to take; its players have no start-gate body machine.

## What the original does

Not looked at. The publisher of the request block's cut flag in the original was never identified
(S25/S27). Whether its free ride clears the flag or never raises it is not known, and neither is
whether its gate sequence plays on a free ride at all.

## How it was proved

Port-side only: the two tests above, on the disc's files, plus the red run without the fix. Gate:
`./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **The original's free ride with no start line is the port's own assumption.** Nobody has seen
  whether the original's body plays the get-ready sequence when a free ride starts. I filmed nothing
  and did not read the request block's owner (it is not identified). The fix follows the lead and the
  user's report that the body should ride.
- **Not run in the real shell.** Nothing launched the shell windowed or with `--shot` through a switch.
  The tests drive `Garage` and `Body::ride` in the shell's order, but not main.rs's loop itself (its
  `moving`/`bike_up` gating, interpolation, the GPU skinning). The fix is one call in `new_rider`, which
  builds every local seat, so the split is covered by construction rather than by a two-seat shell run.
- **Online:** seats are clones of seat 0's start sim, so they carry the lowered flag. No netplay test
  was run beyond the gate's (`same_ride` uses the same `start()` on both sides).
- **The look-around idle** still fires after about 30 s of standing still (the look timer). That is the
  original's selection and unchanged here; I note it because it also plays a get-ready idle.
- The lead's `unfaded` first-pose change was not taken (see above). If a case turns up where a fresh
  body's first frame restarts with ramp 1, that is where it would matter.

## Next

Nothing for this task.
