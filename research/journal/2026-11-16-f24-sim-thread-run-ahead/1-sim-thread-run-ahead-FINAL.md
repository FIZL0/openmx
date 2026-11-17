# 1 — sim thread run ahead (FINAL)

Task F24. 2026-11-16.

## The latency: decided acceptable, on by default

The run-ahead costs one tick (about 33 ms) between a press and its picture and sound. I took it, and
made it the default for an engaged thread, because:

- The thread only engages under `auto` when a tick plus a picture overrun a picture's time, so the
  alternative there is 30 Hz motion steps on a 60 Hz display (or missed frames on one thread).
- The steps are visible all the time. One more tick of latency, in split-screen couch play where
  the machine is already struggling, is the smaller cost.
- `--run-ahead off` keeps N23a's held picture for anyone who wants the tick back. It costs nothing
  to keep: it is the old code path.

## What I did

- **`crates/openmx/src/pace.rs`**: two pacer reads.
  - `ahead(now)`: the blend a tick further behind, measured from the held tick's own due time.
  - `next_due(now)`: when a held tick lands: the next tick is due, or no tick after it ever will be
    (`--frames`, `--seconds`).
- **The ride loop** (`crates/openmx/src/main.rs`), nothing moved or reindented:
  - `Behind`: the before-copies (seats, AI riders, cameras, planes, helis, cars, quads, MX vs. ATV
    cars, plane surfaces, the cut tests, traffic time, path cars, props) one tick older.
  - At each tick's first half, the loop's copies are swapped into `Behind` before the fresh ones
    are taken (free: the vectors trade places). The cut tests are cloned, not swapped, because each
    one carries what it measured on the tick before.
  - At send-off, `Behind` is swapped in, so the pictures blend tick N-1 → N from the stand-in
    (end of N) while N+1 runs.
  - The finished tick is now held in `flight` until `next_due`, not landed at the next pass. The
    draw passes in between draw from the stand-in, with the rigs, cameras, particles and traffic
    clock still at tick N, since its second half has not run. That keeps every part of the
    picture a tick behind together, not just the seats.
  - Landing swaps `Behind` back, and the second half runs as before.
  - The picture's blend is `pacer.ahead` while a run-ahead tick is held.
  - A held tick is not run twice: `beside` only gets a tick that has not run yet.
  - `--seconds`' end waits one pass for a tick still on the thread to land. N23a's held mode could
    break out with the real world still in `flight` and the stand-in as `world`. This now covers
    both modes.
  - `--run-ahead on|off` (default on), and a line about it in `--help`.
- **`crates/openmx/src/sim_thread.rs`**: the module notes say what run-ahead does and what it costs.
- Nothing like this in `../opentennis` to take.

## Why the order stays the same ride

A tick still runs its first half, then its simulation, then its second half, before the next tick's
first half. The only thing that moved is how many pictures come between the simulation and the
second half. The pictures don't write the simulation (P205), and the pads and the menu are only
read in a tick's first half.

## How it was proved

- **Same ride, headless.** I hand-wrote a gas tape: 1800 ticks, Cross from tick 31 on seat 0. It
  lives in the session scratchpad, not committed. I played it on free06 with 2 players, using
  `--headless --steady-clock --trace-picture --trace-hash`, in three modes: `--sim-thread off`,
  `on --run-ahead off` and `on`.
  - All 1800 `tick N hash` lines (both seats) are byte-identical across the three.
  - The ride hash 0xb2308d03089f4763 and the plane hash 0x3fcc295c14e13abd are the same in all three.
- **Smooth, a tick behind.** From the `picture:` lines (3603 pictures, seat 0's drawn bike):
  - Off: median neighbour step ratio 1.002.
  - Run-ahead: median 1.002, the same.
  - Held (`--run-ahead off`): median 3.03, which is the 30 Hz judder.
  - Every run-ahead picture but the last equals the off-thread picture two pictures earlier
    (= one tick at 60 pictures a second), to within 0.005 ft. The two blends differ by 0.001
    because a tick is a PS2 frame pair, not exactly 1/30 s.
  - The last picture jumps a tick, because the tick that ends `--frames` runs on the loop by design.
- **Same ride, windowed.** I used a private headless weston (`--size 640x448 --draw-fps 240`).
  `auto` engaged: `sim thread on: slowest tick 17.70 ms + picture 287.91 ms against 4.17 ms`.
  - Weston's GL pictures took ~290 ms, so both windowed runs hit my 15-minute limit before 1800
    ticks.
  - The 1027 ticks the `auto` run reached hash the same as the `off` windowed run and the
    headless runs.
  - ALSA's POLLERR spam from the sound thread lands mid-line in that output, so I matched the
    hash lines anywhere in a line.
- **Tests:** `./kit check -- -p openmx --bin openmx -- pace::tests sim_thread::tests`: 23 passed.
  That includes the new `a_held_tick_lands_as_the_next_is_due`.

## Not verified / not 1:1

- **The latency was not measured on a device.** "One tick more" is from the schedule
  (`ahead` vs `blend`, and the 2-picture offset in the trace), not timed from a pad press to light
  on a screen. Not measured on a Deck either (R20 is still open for `auto` there).
- **Switching on or off mid-ride is a one-off jump.** When `auto` engages, or a backlog or reseat
  sends one tick back to the loop, the picture moves a tick back or forward once. `auto`'s gauge
  looks at 30 ticks, so it does not flap, but under a heavy load with backlogs this can happen often.
  The windowed weston run (mostly backlog) was not looked at picture by picture.
- **Sound comes a tick later too**, since a tick's sound is played in its second half. It stays
  in step with the picture. I did not listen to it.
- **What I watched:** I checked the bike's and the camera's drawn spots in the trace. The other
  copies in `Behind` (planes, helis, cars, quads, AI riders, props, path cars) are swapped the
  same way, but no picture of them was compared. The tape only rides the bike on free06.
- **A seat joining or leaving** right before a threaded tick leaves `Behind` with the new count.
  That's argued from where the join code runs (before the copies are taken), not run.
- **The tiled worlds and online rides** still never thread (F25 and as before), so they never run
  ahead.

## Next

Nothing; F24 lands with this.
