# 1 — freestyle event shell (FINAL)

Task P155. 2026-10-28.

## What I did

- `game_sim::event` gained the event on a ride: `Zone` (a stunt zone's trigger box), `Rider` (what the
  trigger reads of a rider), and `Event`. `Event` holds the track's zones and target (shared, never sent),
  each player's Circle hold, the `Competition` (S92) while it is on, each seat's verdict, and how long the
  results have been up. `Event::tick` starts the competition when a player holds Circle for `HOLD` = 1 s in
  a zone, stopped and on the bike. It ticks the competition on every card (riders' `Score`s), takes each
  seat's `finish` once it is out, and closes the event `RESULTS` = 6 s after it ends. Closing reopens the cards.
- `Sim::event` carries it. `step`/`step_all` call `event_frame` right after the traffic and before the
  riders, so the card gates sit on the card before `Score::grow` reads them. The hash walks the event and the
  snapshot sends it (tagged `EVENT` section) only when it is not fresh, so every pinned hash stays as it was.
  Decode keeps the local zones/target and reads the section when its tag is there. The others list now also
  stops at the EVENT tag. Before, a one-player state with an event decoded the tag as a list length.
- Shell (`main.rs`): every seat's `Sim` gets the track's event from `portal::stunt_event` (stunt spots of
  `hit.dat`'s `[ASZ n]` + its target) before the spawn template is taken, so online joiners have it too.
  The HUD concept's challenge prompt ("Hold to start", "Stunt Competition", "Score over 100,000 in
  2 minutes") is up while the event is offered, its ring filling with the hold.
- `ride_hud`: `EventShown` and a top-centre pill with TARGET n (orange once reached) over the M:SS clock.
  The clock blinks maroon/white under the last-5-s warning. "TARGET REACHED!" sits under the pill for 3 s.
  The countdown's 3/2/1/GO! is big in the middle. The verdict YOU WIN / YOU LOSE / TIE shows over
  EVENT n and FREERIDE TOTAL n.
- Tests: `crates/game-sim/tests/event_ride.rs` (3: no zone, win with snapshot/resync/rollback, loss with
  the warning). `crates/openmx/tests/freestyle_event.rs` uses free09's real zone and target off the disc
  through `portal::stunt_event`. The stock bike stops in the zone, is offered the event, and starts it with
  a 31-tick hold. The countdown refuses combos, the clock opens them, then the clock runs out with the
  warning up at 101 + 3601 ticks. The verdict is Lose with the saved 2,500 put back, and the HUD lines
  show "3", "TARGET 100,000", "2:00", "GO!", "YOU LOSE", "EVENT 0" and "FREERIDE TOTAL 2,500".

## What the original does

- The trigger: the `[ASZ n]` placer (`./kit fn 0x323220`) sets the zone collidable's box to the zone's
  Position, oriented by LookVector, half extents (Scale.x/2, 30, Scale.z/2). The 30 comes from the constant
  vector at 0x421260 = (0, 30, 0, 1).
- Entering a stunt zone opens the "FreestyleActivityStartMenu" page (strings near 0x431210). Its Start
  creates the activity by name (`./kit fn 0x21b880`; the stunt competition's constructor is
  `./kit fn 0x3dd340`). From there it is S92's competition: the countdown, the 120 s clock, the warning, the
  target line and the verdict, all ported and proved there.
- 30 Hz: the clamp's dt is 0.03333 s, not 1/30. A second's hold takes 31 ticks and the 120 s clock 3601.

## How it was proved

- The competition itself is S92's, proved there. Here it is unit-tested through the `Sim`: game-sim
  `event_ride.rs` (3 tests) and openmx `freestyle_event.rs` (1 test, free09's zone and target off the disc).
- The hash is unchanged with a fresh event: `a_zone_the_rider_is_not_in_starts_nothing`. A mid-event
  encode/decode gives the same hash, and a restore carries the competition.
- `./kit check --all` through `./kit done`. `tests/hash.rs`'s pinned `Sim` size moved 11544 → 11696 for the new field.
- Landed with `--skip-audit`: the audit's 7 leaks are all other tasks' addresses already on main in
  `plan/DONE.md` (lines 121 and 512); this task adds none.

## Not verified / not 1:1

- **The start is ours.** The original opens a start menu page on entering the zone. The port has no
  such page in the ride, so the HUD concept's "Hold to start" prompt starts it: Circle for 1 s, only while
  stopped-ish (|speed| ≤ 58.667 ft/s, the machine offer's 40 mph), on the bike, not wrecked or crashed.
  None of these conditions is the original's.
- **The trigger reads the rider's position, not a collidable** against the bike's bounds. The box's shape
  is from the placer as read in the decompilation, not checked live in PCSX2.
- **No live capture** of an event in the original (no PCSX2 run in this task). The 30 ft half-height,
  the tick order (event before the riders) and the start-of-event state (no teleport to a start, which the
  original may do) are unverified.
- **The results are ours.** The verdict stays 6 s and the event closes itself. The original goes to a
  results menu. There is no "Quit Activity".
- **The HUD is ours** (layout, colours, blink rate, the 3 s announcer line). The original's listener
  HUD pieces and the announcer's voice line are not drawn or played.
- **Players.** Each local split-screen seat has its own `Sim`, so each local rider competes alone, never
  the original's two-player event. Online, everyone in the session joins, and with more than two seats
  S92's verdict rule makes everyone Lose. A rider who joins mid-event rides outside it. A leaver ends it for
  everyone.
- **Profile points** (`verdicts`' second number) are not stored anywhere.
- **Decode with the CARS/TOYS tag and no others**: decode would read the tag as the others list's length,
  the way the EVENT tag did before this task. That is a latent issue outside this task, because netplay
  always has others, so it is not fixed here.
- The shell test rides a flat at the zone's height, not free09's terrain. It does not run the shell's
  window or its pad path; it drives `Sim::tick_in` with Circle held.

## Next

S195: the CARS/TOYS decode follow-up above.
