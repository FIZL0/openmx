# 1 — host changes modifiers online (FINAL)

Task P100. 2026-10-22.

## What I did

Online, the host can now change the game modifiers from the pause. Every machine takes the change at
one agreed tick. The mechanism is the one N10's joins already use: the host takes its settled state,
sends it as a session restart, and every machine decodes it and starts a new ggrs session at that tick.

- **The session** (`net/`):
  - `Netplay::remodify(text)` is host-only and queues the change. A second change before it is taken
    replaces the first, and a change back to the current set cancels it.
  - `admit::run` now fires for a pending change as well as for heard joiners. It sends a new
    `Msg::Mods { round, was, to }` to every peer, ahead of the `Restart` pieces. The link delivers in
    order, so the guests have it before the restart is complete.
  - `recover::restarted` decodes the state, then calls `Game::remodify(was, to)` on it, and only then
    starts the new session. So `was` is the set the state was ridden on, and ticks from the restart
    tick on use the new set. No rollback can reach back before that tick, because the session starts
    there.
  - `Netplay::mods` is the agreed text. `Netplay::remods` lists every change taken, as (tick, set).
  - `Msg::Mods` goes with **every** restart the host's `admit::run` sends, joins included. A joiner's
    machine built its bikes on the room's `modifiers`, which may be older than the ride's. When
    `was == to`, a machine already on that set changes nothing.
  - `Game::remodify` defaults to "only no change" (Tally, and a bare `Rides`).
- **`rides::Modded`** (net/rides.rs) wraps `Rides` with each seat's stock `.phy` and the current
  `Mods`. Its `remodify` runs `Mods::resim(was, 0, stock[seat], sim)` on every seat. That is P98's
  offline path, and it uses 1P's row, which is what every online seat is built on. It then sets the
  time scale for the ticks after. `Rides` itself is unchanged, so its struct literals elsewhere still
  compile.
- **`Online`** (online.rs):
  - `modifiers(mods, stock)` replaces the old `time_scale =` assignments. It normalises the set to
    1P's for every player (`first_players`, which also keeps `Msg::Mods` small).
  - `remodify`, `mods`, `host` and `remodified_at` are new.
  - `frame` steps through `Modded`.
- **The shell** (main.rs):
  - `mods_lock` now takes `Option<host>`. The host of an online ride (lobby, opened freeride) can
    edit the pause's modifier rows. A guest's rows say "Online only the host changes the modifiers:
    every rider takes them at the same moment". A played tape is held as before.
  - When the host's pause closes on a changed set, the shell calls `on.remodify` instead of the local
    `remodify`.
  - After each online frame, if `on.mods()` moved, every machine updates the biplane (`replane`), the
    HUD's modified marks, `front.menu.modified` and `mods`, and prints the tick.
  - Each seat's stock `.phy` is handed over for lobby rides (each seat its pick's), drop-ins, opened
    rides and `--online`.
- **tests/menu.rs**: `the_pauses_modifiers_take_when_it_closes_unless_held` failed on HEAD before
  this task. It used `Knob::ALL` positions, but since P95 the menu lists `Knob::MENU` (P98 and P95
  crossed at the merge). I changed its two `Knob::ALL` uses to `Knob::MENU` so the gate passes.

Nothing taken from opentennis: it has no mid-match change of game parameters (grepped its crates).

## What the original does

Nothing: the modifiers (P55) and online play are both the port's own. The original-game guarantee
still holds. With no change made, the only new traffic is a `Msg::Mods` with `was == to` alongside a
join, which changes nothing, and every N10 join test passes unchanged.

## How it was proved

`crates/openmx/tests/netplay.rs`:
- `the_host_changes_gravity_mid_ride_and_both_peers_stay_in_sync`: two peers on a link with 5% loss
  and a lag of 3 rounds, mashed pads, on the real `Rides` (stock bike, flat floor). At round 300 the
  host asks for `gravity=50`; the guest's own request is refused.
  - Both machines took the change at one tick (269 on this run): the same `remods`, round 1, no
    desync.
  - Every seat's `.phy` on both machines equals `Mods::bike` of the stock file at half gravity.
  - Both machines had the same state hash before every settled tick and ran the same pads, from tick
    0 to the last (829 ticks).
  - An offline replay of the host's pads, with `resim` put on before the change tick, has the same
    hash as the session before every one of those ticks. So the online change is P98's offline change
    at that tick.
  - A run with no change matches up to the change tick, and its state first differs within 30 ticks
    after it.
  - The host never stepped back more than the rollback window.
- `a_peer_that_joins_after_a_change_rides_the_changed_set`: the host rides alone and changes
  gravity at round 100, which it takes at once (tick 100 to 109). `g` drops in at round 300, its
  machine built on the original set. `g`'s `remods` is exactly (its join tick, `gravity=50`), every
  seat on both machines is on half gravity, and hashes and pads agree on every settled tick from the
  join on (over 300).
- The whole netplay file (32 tests, N10's join tests included) and the full `./kit check` pass.

## Not verified / not 1:1

- **The shell path was not run live.** The pause edge, `on.remodify`, and the HUD and menu updates
  after a change were only compiled, not driven. Nothing here presses a pause unattended, and
  `--online` (which can run headless on localhost) has no front end, so it has no pause to change
  modifiers from. The session path they call is the one the tests above prove.
- **The host's ride holds during the change.** Like N10's join, a restart makes ggrs sync again
  (about 5 round trips) before the ride advances. With others in, the ride also steps back to the
  settled tick. The change lands at the settled tick, not the tick the pause closed on.
- **Online every seat rides 1P's modifiers.** That was already the rule (P55/N3), and a change is
  1P's too. `--online` builds seat *i* on row *i* (`mods.bike(i, …)`), so a `--modifiers` with
  per-player values there would be normalised to 1P's at the first change. `--online` has no pause,
  so in practice it never changes.
- **The room listing's `modifiers` is not updated mid-ride.** `openmx-net` has no command for it.
  A joiner gets the right set from the join's restart, but Find and the board still show the set the
  room opened on.
- **A guest's pause shows the guest's own Settings modifiers** (held), not the ride's set. The HUD
  mark and the "modifiers: … (the host's, from tick T)" line say the ride's.
- **The biplane is not ridden online** (online rides the bike), so the `replane` call after a change
  is not exercised.
- **A game whose `remodify` fails ends the session** ("the host's modifiers did not take") rather
  than refusing the change up front. The shell's `Modded` can only fail on an unparseable set, and
  `Online::remodify` sends `Mods::text()`, which always parses.

## Next

Nothing for P100.
