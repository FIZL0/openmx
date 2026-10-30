# 1 — online frames count session ticks (FINAL)

Task K10. 2026-10-29.

## What I did

`two_online_machines_start_their_seats_in_the_plane_on_a_race_track` (tests/machines_everywhere.rs)
starts two `--online` shells at once, each with `--frames 400`, and wants both to print
`state before tick 300` with the same hash. What it raced: **the other shell's load time, on the
wall clock.** Online, every pass of the loop the pacer owes counted toward `--frames`, including the
passes spent waiting for the peer to finish loading (no markers yet, ggrs not synchronised) and the
passes ggrs stalls for a late peer's pads. On a busy gate the two shells load nat19 seconds apart, so
the first one up spent much of its 400 frames (6.7 s of wall time) waiting and quit with fewer than 300
settled ticks: no `state before tick 300` line, or (when it left early enough) a mismatch.

Reproduced with the pre-fix binary by starting the guest 10 s after the host:
`online: 84 ticks, 78 settled` / `online: 124 ticks, 84 settled`, no tick-300 hash.

Fix (main.rs, online.rs):
- `--online`'s `--frames N` now ends the ride after the session's Nth tick (`Online::ticks`, the
  netplay's `tick()`), not the loop's Nth frame; the pacer is not capped by `--frames` on `--online`
  (it would stop stepping the session before it got there). Offline `--frames` and a lobby ride opened
  from the menu are unchanged. `--frames` was documented as "the same run on any machine"; online it
  now is.
- The test waits on both shells together: since a shell now waits for its peer's pads rather than
  quitting on wall time, one that fails kills the other instead of leaving it waiting for ever, and
  each stdout is read on its own thread so neither blocks on a full pipe.

Nothing taken from opentennis: its netplay tests run in-process, not as two binaries with a frame count.

## What the original does

Not applicable: test-gate and shell flag work, no simulation change.

## How it was proved

- The same 10 s staggered start on the fixed binary: both shells `online: 400 ticks, 399 settled,
  0 desyncs, state before tick 300 0x70245ba2dc5b2f87`. Started together: the same two lines.
- The test itself in `./kit check --all` (machines_everywhere 98 passed). That run also redded
  web.rs `the_touch_pad_rides_on_a_phone` (TypeError in touch.mjs under load; passed alone at once) and
  original_mode.rs (no DISPLAY exported in the agent's shell); neither touches this change.

## Not verified / not 1:1

- The free ports are still taken by binding port 0 and letting go, so another process could take one in
  the seconds before the shell binds it. Not seen fail; left as it is because the shell has to bind its
  own address and the peer must know it beforehand. A failure would read "Address already in use" in the
  shell's output.
- If a peer shell dies without the test noticing (it is killed from outside), the other now waits for
  ever instead of ending on its frame count; the test kills it when it sees the first one fail, which
  covers every failure that exits.
- Not run under an artificially loaded machine beyond the staggered start (load was ~18–30 from other
  slots during the runs).

## Next

none
