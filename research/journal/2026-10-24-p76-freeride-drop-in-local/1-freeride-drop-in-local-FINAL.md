# 1 — freeride drop in local

Task P76. 2026-10-24.

## What I did

- **The seat change is one value.** `join::Change` (`Join(pad seat)` / `Leave(player)`) with
  `Change::apply(&mut Vec<T>, make)`: a joiner is pushed on the end, a leaver is removed and the
  players after them move up one. main.rs's ride loop runs every per-player list through it — seats,
  rigs, kept markers, idle pads, marked portals, `seated`, per-view triangle counts, cuts, banners,
  bay answers, starts, bike words, stock `.phy`s, the Start readers, the last tick's cameras — so
  each staying player keeps all that is theirs. `Change::devices` keeps the pad-seat list
  (`args.pad_of`) in step. No reload: the ride runs on.
- **Joining.** Every pad seat has a reader now (`idle_readers`), fed every frame so that a
  controller that just left does not rejoin on the press that left. A Cross from a pad seat with no
  player in the ride, while riding or over the pause, is `Menu::step_idle` → `Act::Join(pad seat)`
  (full at `join::IN_RIDE` = the join screen's 4: a note). The joiner is a clone of the ride's spawn
  `Sim` (the track's start, the first player's bike and rider), `ghost.start()`ed (P57), wrecks as
  the settings say, on markers loaded from `"{n}P"`'s file, in a garage with the lot's plane for the
  new seat after `Lot::resize(players)` (P87 note). Its rig loads the first player's body and chassis
  again (each animation set reads its own machine), shares the GPU models, and in software copies the
  bike and body models under `@join{k}` names. The pause shows a chip "✕ on a free controller joins"
  while seats are open and the ride is not full.
- **Leaving.** The pause gains **Leave Session** (before Quit, under the online rows) when the ride's
  seats are open. The pauser's Cross on it is `Act::Leave(pauser)` and the pause closes; the last
  player gets the Main Menu confirm (P110) and goes back to the menu. The leaver's engine is silenced
  (`Sound::seat_left`).
- **Re-layout.** `Menu::reseated(change)` makes the ride a `Party` of the new devices (so a Restart
  starts with them), shifts `up`/`switchable`, notes "Player N joined/left the ride.". The shell then
  takes `Layout::for_players(n, split)`, rebuilds the view rects/frames (`views_of` now takes the
  layout and count), and `set_display`s every rig onto the presets of the new view shape. A
  `players: … ; views: …` line says it.
- **Open to Online** is offered again once the ride is back to one local player, and not while
  there are more (it was computed once from `seats.len() == 1`).
- **Where seats are open**: a local freeride with the front end — not online, not a played or
  recorded tape (`Menu::seats_open`, set by the shell). Online rides one local seat (N10/N11's
  mid-session path is per machine); see Next.
- opentennis checked first (AGENT.md): its matches have fixed line-ups, nothing to reuse.

## What the original does

The original has no drop-in: a freeride's players are fixed when it starts. This is port-only
behaviour; the split layouts and preset suffixes it re-lays out to are the original's (V22).

## How it was proved

`crates/openmx/tests/menu.rs::players_drop_into_a_running_freeride_and_leave_it_and_the_others_ride_on`:
pad input alone on four pad seats — Free Ride from the main menu; pad 1's Cross joins (Full →
Stacked, a see-through joiner with a plane parked, player 1's bay unmoved); pad 1's Cross again joins
nobody; pad 2's Cross over pad 0's pause joins (Quarters, 3 views, the pause stays); pad 1 leaves from
their own pause through Leave Session (pad 2 moves up, Stacked again, party devices `[0, 2]`); pad 1
rejoins as player 3; then each staying player's `hash::of(ride)` on every tick (>40 each) equals a ride
stepped alone on that pad's frames with the same freezes; last, everyone leaves and the last one out
is asked and lands on the main menu. The harness applies the same `Change::apply` as main.rs.
A mutation (Leave popping the last player instead of the leaver) fails it. `./kit check -- -p openmx
--test menu`: 137 pass.

## Not verified / not 1:1

- **The shell's own join/leave path was not run end to end.** The binary reads only `/dev/input/js*`
  and skips uinput pads (memory: the shell skips uinput pads), so a second live controller could not
  be made here; the main.rs block is compiled and its lists are the ones the test's harness changes,
  but its rig loading, model copies, sound seat, set_display and the views line were never seen on a
  screen.
- **The ride-on proof steps each rider alone on a flat floor.** Riders touching each other (S63) is
  not in it: a joiner is a ghost at the spawn, but once solid it can meet a staying rider, which then
  rides differently — as it would with any rider there.
- **A joiner rides the first player's bike, rider and look**, not a pick of their own (no garage
  from the pause), with the number plates of the first player when numbers are on, and the name
  texture is not re-made for them.
- **A joiner's bike has the modifiers the ride started with** (the spawn `Sim`'s `.phy`); a change
  made in the pause after the ride started reaches them only through the next remodify.
- **After a middle leave, the players who move up keep their machine and plane, but their bay
  (`Lot::bay` by index) and home slot are the ones of their new index**, so a moved-up player's
  parked plane / start-home is the departed one's from then on.
- **The joiner's markers are kept as `"{n}P"`** (their seat number when they joined), not a profile
  name; a moved-up player keeps the profile they started with (ClearMarkers now reads the kept one).
- **If player 1 leaves**, the keyboard's pause follows the new player 1 (seat 0), and the front end's
  reader is the old one.
- **Online and taped rides do not open their seats** (`seats_open` false): left for a follow-up.
- Software model copies of a player who left stay in `assets` until the ride ends.

## Next

Follow-up filed for online multi-local drop-in. If a live second pad is ever available, ride a
freeride with two pads and join/leave from the pause to see the views re-lay out.
