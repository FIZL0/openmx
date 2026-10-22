# 1 — markers seen in the ride (FINAL)

Task P94. 2026-10-22.

## What I did

- **Only riders are picks.** `game_sim::markers` gained `Markers::others(me, seats, riding)` (the
  other seats in the ride, in seat order), `Markers::pick_among(me, riding)` and
  `markers::frame_among(sim, me, spots, riding, hands)`. The selection's places are the five slots
  plus the other *riders*; a seat not riding is stepped past and Up can never land on it. A
  selection left on a seat that stops riding moves to the next rider, or to the first slot when there
  is none past it. The old `pick(me, players)` / `places(players)` / `frame` stay, as "everybody
  riding", so nobody else's call sites moved.
- **Who is riding.** Offline every shell seat is somebody's (join.rs skips open seats; a seat whose pad
  was pulled is still a rider on the track, P35), so nothing changes there. Online a seat nobody
  sits in rides on with the missing pad — the contract `net::Game::step` already documents — so
  `net::rides::Rides::step` counts a seat as riding when its pad's status byte is not the missing
  pad's. That keeps it deterministic: the pads are the session's, so every machine counts the same on
  every tick and every re-run, and no field was added to `Rides` (17 literal sites in
  tests/netplay.rs would have had to move). The display side (main.rs) counts off the lineup
  (`Online::names()`), which is the same seats.
- **The slot display lists the riders.** `hud::draw_markers` takes `others`: after the five slot boxes,
  a `P2`… box for each other rider, the selected one ringed. Filled slots are drawn in their slot's
  colour (was the HUD's green), so the banner matches the posts and the radar.
- **Posts in the world.** `overlay::marker_post` projects a marker's spot and a point
  `POST_HEIGHT` (15 ft) above it through the view's camera; `hud::draw_marker_posts` draws a dark-edged
  pole in the slot's colour and a flag at its top with the slot's number, the selected one's flag
  bigger and ringed white. Drawn over the picture (both renderers: in the GPU path inside the HUD's
  layer patch, in the software path over the view's frame), far ones first. Only the view's own
  player's markers, as the faint ghost bikes (P88) — which stay.
- **Radar.** `Icon::Marker { slot, selected }` (was `{ facing, selected }` on the original's
  `map_icon_gate`): a disc in the slot's colour with the number on it in the label font, a white ring
  round the selected one, a dark edge round the rest. `overlay::marker_blips` makes them;
  `map_icon_gate` is no longer loaded. `MARKER_COLOURS` are cyan, magenta, lime, coral and
  lavender — none the original's icon or rider colours (test holds them to that list).
- opentennis has nothing like session markers; nothing taken.
- **Not mine, fixed to pass the gate:** `tests/menu.rs`'s
  `the_pauses_modifiers_take_when_it_closes_unless_held` was red on main (b63486a, checked in a scratch
  worktree): P98's test counted the modifiers' rows with `Knob::ALL`, but P95 took PlaneSpeed off the
  menu (`Knob::MENU`). Two words changed to `Knob::MENU`; the menu code was not touched.

## What the original does

Nothing: session markers are the port's own (P88). The radar icons they must stay clear of are the
original's set (P92).

## How it was proved

- `crates/openmx/tests/markers.rs`:
  - `a_seat_not_riding_is_never_a_pick` — three riders, seat 2 out: Right walks slots 1–5, seat 3,
    round; Left from slot 1 is seat 3; Up lands on seat 3; a selection on seat 2 moves to seat 3 / slot 1.
  - `online_an_empty_seat_is_never_a_pick` — `net::rides::Rides::step` with seat 2 on the missing pad:
    seven Right presses wrap one place sooner than with everybody in.
  - `a_filled_slot_has_a_radar_mark` — filled slots 1 and 3 give a radar mark each (the selected one
    ringed, in its own colour, at the bike), an empty slot none; the post stands up in a view looking
    at it and is `None` behind the eye.
- `crates/openmx/tests/overlay.rs` updated: the marker icon kind check, `a_markers_icon_is_its_slots`
  (five distinct colours, none of the original's), and the drawn radar's sprites carry a disc per
  marker and one white ring.
- Looked at: a headless `--play` tape on free06 (place slot 1, ride, place slot 2, ride back to 1,
  select 2) shot at tick 165, GPU and software, one and three players: the magenta "2" post stands at
  slot 2 ahead, ringed; banner and radar match; three players' banner reads `P2 P3`.

## Not verified / not 1:1

- Nothing here is 1:1 with the original: the markers, their posts and radar discs are the port's own.
- Posts are drawn over the picture, not depth-tested: a marker behind a hill still shows through it.
  Chosen so it can be seen from anywhere (the task's "from a distance"); not a world model.
- An online seated player whose pad reads as the missing pad (status 0xff) would be counted as not
  riding for those ticks while the display (lineup) still lists them. Not seen: the shell sends an
  idle report for a seat with no pad, never the missing one; not tested against a real unplug online.
- No online two-machine run was ridden to look at the banner over the network; the online rule is
  proved through `Rides::step` alone.
- Posts make the HUD layer patch span the screen where flags are far apart (the patch covers what was
  touched), an upload cost on the GPU path while markers are placed; not measured.

## Next

Nothing.
