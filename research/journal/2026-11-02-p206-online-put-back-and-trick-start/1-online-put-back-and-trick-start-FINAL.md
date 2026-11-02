# 1 — online put back and trick start (FINAL)

Task P206. 2026-11-02.

## What I did

- `crates/openmx/src/audio.rs` `Said::of` takes the seat's `Sim::plane` too and keeps a `Seen` per room
  seat at each drawn frame. Besides the wreck entry and the landed trick, it now reports:
  - **respawned**: the wreck reason going from some to none; or the bike's ghost clock going up, because
    every bike put-back calls `Ghost::start`, including the chord on a rider who is still riding; or the
    plane's crash reason going to 0; or the plane's body coming back within 3 ft of its spawn after
    being more than 10 ft away. Nothing else on a drawn frame shows the chord's put-back of an unwrecked
    plane on its wheels.
  - **started**: `tricks.in_progress` going from false to true, which gives `tricks.current`. A queued
    trick that follows on is not reported, the same as offline (`restart && !before`). Failing that, a
    pack's front flip coming round (`front_watch` reaching 3), or a newly shown flip's points
    (`score.shown`), in the same order the rider reports them.
- `crates/openmx/src/main.rs`: the online tick passes `seat.sim.plane` to it.
- `tests/online_wreck_sound.rs`: the new argument.
- New `tests/online_put_back.rs`:
  - `a_headless_online_put_back_resets_the_lean` uses two in-thread `Netplay` machines over `Rides`, with
    the link 3 rounds late. The guest swings its stick, so the host re-runs 76 ticks. Both seats start in
    the disc's biplane on flat ground. The host's plane taxis 77 ft on the throttle and is put back with
    L1+R1 at tick 100. On the host's drawn frames, the put-back is reported once, on round 131, which is
    the frame the plane is first drawn back in its bay. It is never reported for the guest. A
    `PilotLean` stepped as main.rs steps it snaps there, and one never told of the put-back does not.
    The test skips without the disc.
  - `a_put_back_and_a_trick_start_are_read_off_the_drawn_ride` uses hand-set riders. It checks: a wreck
    going away, the ghost starting, and the ghost starting over are each a put-back, and the ghost running
    down is not. A trick coming into progress is a start once, and a queued trick that follows on is not.
    A flip's shown points are a start.

## What the original does

Nothing new: this is presentation on the shell side. The rider's own `Frame` (game-sim `rider.rs`) is
what the offline path already reports, and these readings match it.

## How it was proved

`./kit check -- -p openmx --test online_put_back --test online_wreck_sound --test netplay`: 100 tests
passed. Both new tests are in `crates/openmx/tests/online_put_back.rs`. `cargo build -p openmx` passes.

## Not verified / not 1:1

- MX vs. ATV's 360 coming round (`spin.came_round`) is not reported online. The flag is cleared in the
  same frame that sets it, so a drawn frame never sees it.
- Several of the same event between two drawn frames are reported once: two put-backs, or a trick that
  starts and ends with no frame drawn in between. A drawn frame cannot tell them apart.
- A plane chorded back within 10 ft of its bay is not reported as put back, because the 10 ft rule
  cannot see it. Its lean does not snap.
- The bike's put-back and the `trick:` line are tested on hand-set riders, not on a live online ride. The
  `put back` / `trick:` println lines in main.rs were not run online here.
- The browser build was not checked (`tools/web.sh --check`). The change is one argument in main.rs's
  online tick.

## Next

none
