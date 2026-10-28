# 1 — online wreck sound (FINAL)

Task P184. 2026-10-28.

## What I did

- **The bug (from P180):** the online tick set `world.seats[0].said` to `rider::Frame::default()` on every tick. The other seats' `said` were never set at all. So `Sound::frame` (and the shell's `wreck:` / `landed:` lines) never saw a wreck entry or a landed trick online.
- **`audio::Said`** (crates/openmx/src/audio.rs) reads both off the ride *as it is drawn*. The session runs the riders' own frames inside ggrs, re-runs some on a rollback, and keeps none of them, so `Said` keeps each room seat's rider from the last drawn frame: wrecked, off the ground, and the trick in flight.
  - A **wreck entry** is `state.wreck` going from none to some between two drawn frames.
  - A **landed trick** is the bike coming down between two drawn frames: in the air with a trick asked for before, on the ground after, and no wreck. This is the same edge `Rider::frame` takes `Frame::landed` on.
  - **Rollback-safe:** a tick re-run to the same wreck is drawn wrecked both times, so it is heard once. A re-run that takes the wreck back clears it, and the wreck it does have is heard when it is drawn. It only reads; nothing it does reaches the simulation or its hash.
  - The memory is kept by room seat (`Online::drawn()[d]`), not shell seat, so a join or leave that reorders the shell's seats does not hand one rider's memory to another. A seat's first drawn frame says nothing.
- **main.rs:** the online tick sets every seat's `said` from `Said::of`. Seats past `drawn()` get the default frame. Online wrecks and landings now print their lines too, as offline ones do.
- **game-sim `Rider::in_flight()`** is a read-only getter for `(airborne, flight)`, the two `pub(crate)` fields `Frame::landed` is computed from. No simulation code changed.
- **From opentennis:** I looked at `hst-play/src/play/heard.rs`, which plays sounds inside the tick and de-duplicates them by (tick, kind, count), with a tick counter in the snapshot. I did not take it. That design needs sound calls inside the simulated step, and here the rollback runs `Rides` in `net/`, which has no sound. An edge on the drawn state does the same job for one-shot entries without touching the session.

## What the original does

Nothing new. The sounds themselves are P180's and P144's (the entry cry by reason, `[IMPACT]`/`[Fall]`, the bike's `[Wreck]` clank at `MinImpulse`) and the crowd's cheer on a landed trick. This task only gets them their inputs online.

## How it was proved

- `crates/openmx/tests/online_wreck_sound.rs`, `a_headless_online_wreck_plays_its_voice_and_clank`:
  - It runs the real session (`Netplay` over `Rides`, as the shell steps online) on two machines in one thread, linked three rounds late.
  - The guest swings its stick every 4 ticks, so the host keeps guessing its pad wrong: 62 ticks re-run in 120.
  - Both seats are P180's drop-and-wreck bike (game-sim's ridden test bike with the drop wreck limits).
  - The host's `Sound` gets each drawn frame's `SeatFrame`, with `wreck`/`landed_trick` from `Said`.
  - Result: each seat's wreck cry played exactly once, on the round its wreck was first drawn (75), with that seat's own clank on the same round. The host had no desync.
  - Before this change the same frames carried `wreck: None`, and no cry played.

## Not verified / not 1:1

- **Landed-trick cheer online: not tested.** The code path is the same edge as `Rider::frame`'s `landed`, read through `in_flight()`, but no test rides a trick to a landing online. A fixture with a jump and a trick on the flat test world would be needed, and I did not build one.
- **No full shell run.** The online shell binary was not run headless here. The test drives `Said` and `Sound` the way main.rs does, but not main.rs itself. main.rs's change is one loop over `drawn()`.
- **Several ticks between two drawn frames.** If a whole wreck-and-put-back happens between two drawn frames (catch-up after a stall: the wreck clock is ≥ 2 s), the wreck is never drawn and is not heard. A takeoff and landing between two drawn frames is missed the same way. This is presentation only, and the original never skips frames this way.
- **A misprediction that moves a wreck's tick.** If the predicted ride wrecks and the corrected ride wrecks a tick or two later, the wreck stays drawn throughout, so it is heard once, at the predicted time. If the correction takes the wreck away entirely, the cry has already played. This is the usual rollback trade-off; opentennis accepts the same for its one-shots.
- `respawned`, `started` and `separated` stay at none online (the brief asked only for wreck entries and landings). So the biplane pilot's lean reset and the online "put back" / "trick:" lines still do not fire online.

## Next

Nothing for P184. Follow-up filed: P202 (online `respawned` / `started`).
