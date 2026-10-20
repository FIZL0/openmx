# 2 — p35 seats and pads (FINAL)

Task P35. 2026-10-20. Continues 1 (the seats, `PadSlots`, `controls.txt`), which was blocked on S54.

## What I did

By the time S54 landed, V22 had already given every seat its own player in the shell: `World::seats`
is one `Seat` (pad reports, `Sim`, garage) per local player, and main.rs's loop feeds `Seats::frame`'s
seat `i` to `world.seats[i]`. The shell rides one `Sim` per seat rather than `Sim::others`, so the
"pause only the waiting seat's player" the brief asks for lives in `Seat`, not in game-sim:

- `Seat::waiting` (main.rs), set each frame from `SeatInput::Waiting`. `Seat::pass` returns at once for
  a waiting player: not polled and not stepped, so the input manager's held buttons and timers stay
  where the pull left them and the ride carries on from there when the pad is back. Every other player
  rides on.
- `engine.frozen` is no longer set by the shell. It froze the whole frame for any seat's pulled pad.
- `--record`: a tape is the first player's, so a pass where 1P waits is not written to it — the tape
  holds the passes 1P rode, with no hole, which is what the old whole-frame freeze guaranteed.
- The body and chassis pose (`moving`), the overlay's frozen flag and the per-second line read
  `waiting` too; the per-second line says `(waiting for the pad)`.

## What the original does

Nothing to port: host pads and seats are the shell's (see 1).

## How it was proved

- `./kit check`: TESTS PASSED, including pad.rs's seat tests from 1 (join order, kept across an
  unplug and replug, the keyboard as a seat).
- An unattended run of the real shell on :0, `--players 2 --pad <fifo> --seconds 8`, a script
  (scratchpad, not committed) holding Cross on the fifo and after 4 s closing it and removing the node,
  as the kernel removes `jsN`: 1P printed `the pad is out — paused`, the per-second line read
  `40.7 mph (waiting for the pad)` for four seconds without moving, and at the end **player 1: 239
  ticks, player 2: 479 ticks** — 1P stopped, 2P rode on, the shell did not crash. In a first run that
  left the fifo node in place the rescan reopened it each 0.5 s (`is back` / `is out`), and 1P stepped
  a frame or so each time: an artefact of a fifo, whose node outlives its writer, not of a js device.

## Not verified / not 1:1

- **No test of `Seat::pass`'s pause in `./kit check`.** main.rs has no test module and a `Seat` needs
  the disc's tables and `.phy`; the proof is the unattended run above, not a gate.
- **Online (N2) does not pause.** `Online::frame` steps the local seat's `Sim` itself, outside
  `Seat::pass`; with 1P's pad out it is fed `Report::missing()` (nobody's hands) and the session runs on
  — the same input every peer sees, so it stays in sync. Pausing one rider in a rollback session would
  be a session-level pause N2 does not have; the old `engine.frozen` did not reach it either.
- **A waiting 1P under the front end's pause menu:** the menu reads 1P's report, which is
  `Report::missing()` while it waits, so Start on the pulled pad cannot open it (Escape still does).
- Everything in 1's list still stands: no real pad was plugged or pulled (no js device, `/dev/uinput`
  root-only), Steam's filter not seen against Steam, `controls.txt` beside the working directory not
  the disc, bindings not on tapes. 1's "seats past 1P do not ride" and "freezes the whole frame" are
  resolved here.

## Next

none. P38 builds the join screen over `PadSlots` (`forget`, `waiting`, `keys`).
