# 1 — start timeline cadence

Task S304. 2026-11-12.

## What I did

- `race::Countdown` gained the score manager's listener `interval` (`+0xc0`, 0.1 s — `Race::new` sets
  it, as the race's reset does) and accumulator `acc` (`+0xc8`, 0 from the countdown's start), with
  `Countdown::listen` / `Countdown::heard` doing the manager's cadence.
- `race::Race` gained `activity: Option<Supercross>`, the countdown's listener. With one, `Race::tick`
  calls `Supercross::start_timeline(elapsed, &mut [], false)` at the cadence, applies the intro's end
  (elapsed moved, accumulator zeroed) and hands the `StartStep` back on `Frame::start`. `None` (every
  existing race) behaves exactly as before: no listener, no accumulation.
- `Sim::race_frame` applies `Frame::start` to every rider's chassis with `StartStep::apply` after the
  caught riders' pushes and before the drop's release.
- Snapshot: `Countdown` wire carries `interval`, `acc`; `Race` wire carries `activity` (a full
  `Supercross` wire added). Hash: `countdown.interval`, `countdown.acc`, and the activity's timeline
  fields (cue, the four windows, intro seen/queued) behind an `activity` bool.
- Filed P331: the shell's `race_start.rs` builds its `Race` without the activity, so play does not run
  the hold yet (not this task's files).
- Nothing taken from opentennis: it has no counterpart of a listener countdown. The shape copies
  `event.rs`'s own countdown (the same manager code, already ported for the freestyle event).

## What the original does

`./kit fn 0x0025fcd0` (score_manager_tick), counting-down branch (flag 0x80): if `interval > 0` and a
listener is set (`+0x30`): when `interval <= acc` or `acc == 0`, take `interval` off `acc` (only in the
first case), call the listener's vt+0x5c with (length, elapsed, interval), then the listener chain on
`+0x30 +4` (none in a race); then `acc += dt` — inside the listener check, so no listener means no
accumulation. Then the end test `!(elapsed < length)` and `elapsed += dt` read the elapsed the
listener may have moved (race_intro_end writes the manager's elapsed and accumulator through the
activity's `+0x114`/`+0x118`).

## How it was proved

- Function oracle, `crates/game-sim/tests/race.rs`
  `the_countdowns_listener_cadence_matches_the_score_managers_tick`: score_manager_tick itself in the
  interpreter, 1,500 runs, 271,100 ticks, 68,551 listener calls (832 on a zero accumulator, 67,719 on
  the interval), 421 intro ends moving the clock, 919 warnings, 175 runs with no listener; intervals
  0.1, 1/30, 0.01, 0, −0, negative, denormal, random; dts 30 Hz, 60 Hz, random; lengths from a reset
  or random; stray start elapsed/accumulator words. Every tick: each call's three float arguments,
  the step, elapsed, accumulator, the 0x80 flag and the drop call, bit for bit.
- `the_start_timeline_holds_the_rider_until_the_warning`: a Sim race with sx01's 1.54 s start call
  and the intro seen: held (free 0,1,0) from tick 1, the clock started over by the intro's end, freed
  at elapsed 0.13332 inside the warning window (0.04..0.14) before the drop at tick 93; the snapshot
  round-trips and the accumulator and timeline change the hash.

## Not verified / not 1:1

- The oracle's listener is a hook running the port's `start_timeline` and writing what the intro's end
  writes, not the real `race_countdown_listener`: the listener itself was proven by S13f, this proves
  the manager's cadence around it. The two were not run together in one oracle.
- The listener chain (`+0x30 +4`, other listeners after the activity) is not ported: a race has the
  activity only, as far as S13f traced; not checked on a live race.
- Items: the port passes no `StartItem`s and applies the step to every rider's chassis. The original
  touches only `MXSupercrossInterface` items' bodies (on S196's sx01 image one item, the human's
  bike); whether computer riders' bikes are items was not checked (the port's riders are all players).
  The items' words and the intro camera argument (always false) are presentation and not kept.
- No live capture of a start with the hold: the warning tick on a real sx01 start (and that it lands
  at 30 Hz with the 0.1 s cadence) is shown only on the port. S290 reopens to test the held rider.
- The shell does not give its race the activity yet (P331), so play is unchanged by this task.

## Next

S290 reopens to test the held rider; P331 wires the activity into the shell's race.
