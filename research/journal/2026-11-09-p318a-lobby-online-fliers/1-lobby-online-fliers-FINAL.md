# 1 — lobby online fliers (FINAL)

Task P318a. 2026-11-09. This is the lobby side of P318's online fliers
(research/journal/2026-11-09-p318-online-pack-offer/2-online-fliers-FINAL.md).

## What I did

- **The Garage's Fliers tab in a room** (menu.rs `lobby_fliers`, `Menu::garage_fliers`).
  - While this machine is in a lobby room, the tab lists the fliers on offer with each pack's skins
    greyed where `room.offer()` does not open the pack. The reason is the room's `why()` (`needs …,
    missing for 2P`). A pack the room copy has not heard of reads `Not every machine in the room holds
    this pack`.
  - The greyed list is rebuilt when the room changes (`Event::Room`/`Start`/`DropIn`) and when the
    fliers on offer change (`set_fliers`). Every Garage reader on the Fliers/Machines tab in
    menu_pages.rs reads it, so a greyed skin is stepped over, as P315 steps over one it cannot fly.
  - A seat whose saved pick is greyed rides MX Unleashed's in its place (`flier_of`'s fallback). The
    row's note now leads with the greyed pick's name and its why.
  - `Menu::fliers()` (the offer itself) does not change, so the pause and offline picks are as before.
- **Starting a lobby ride in a pack's plane.**
  - `openmx_net::Pick::flier` (new, `pack/skin`, serde default, not in the handshake, like `machine`).
  - `Lobby::set_flier` (menu.rs) fills `flier` and `Pick::packs` (`machine/<id>`, the key
    `handshake::pack_key` makes). It only does that where the pick starts on the plane, is a pack's
    and not MX Unleashed's, and `room.offer()` opens the pack. Otherwise both are empty. So
    `Room::needs()` holds every seat and every drop-in to that pack through file 1's handshake check.
  - main.rs feeds it 1P's Garage pick of the plane kind each frame, beside `set_machine`.
  - `Online::start_planes` puts each flying seat in the template its pick's byte names
    (`start_flier`: `online_fliers::byte_of` on the ride's offer, else 0) through
    `rides::template_to`. A pick the offer greys starts in the ride's own biplane.
  - main.rs starts this machine's pad `FLIER` byte at its seat's start byte, so the seat's next switch
    onto the plane takes the same skin.
  - A drop-in that picked a pack's plane rides in flying in it too (`online.rs` `Event::Arrived` arm,
    `spawn_flying_on` with that template).
- **Narrowing on a drop-in, tick-synchronised** (net/rides.rs).
  - `rides::Flier::closed`: a closed flier answers no switch. `template_to` gives the ride's own plane
    in its place. A seat already up in it flies on in it: `probe` and `remodify` still find it by
    `template_for`.
  - The host closes the fliers a joiner's machine lacks at the join. In the `Arrived` arm it makes the
    offer on the room with the joiner in it (`offer_on_room`), and every template not open there goes
    in the join's spawn (`rides::spawn_closing`: the preset count's top bit, then a count and the
    bytes; `spawn_on` is `spawn_closing` with none).
  - `Rides::seat` closes them in the host's `grow`. `Rides::encode` appends the closed bytes after the
    seats (count, bytes; nothing when none, so every existing state is byte-for-byte as before), and
    `Rides::decode` sets exactly that set closed. So every machine closes them on the restart's tick,
    and every later resync carries the set.
  - The picker rows (`offer_fliers`) grey a row whose template is closed (`Closed when a machine
    without its pack joined the ride`), and are made again when a decode changes the closed count.
    So the pause picker stays honest after the lacking seat leaves, because the templates never
    reopen.
  - The old per-machine narrowing of the picker on a room change stays, for display. Only the
    templates decide what a switch flies.

Opentennis has no mods or fliers, so there was nothing to take. The shape follows this crate's own
N13 preset bytes in the spawn and P318's skin flag in `seat_bytes`.

## What the original does

Nothing: online play, the lobby and mods are openmx's own.

## How it was proved

- `tests/netplay.rs` `a_joiner_lacking_a_fliers_pack_closes_it_at_the_join_on_every_machine`:
  - Seat 1 goes up in the pack's plane, then a join's spawn closes byte 0x12 on the host.
  - The encoded restart state, decoded on a second machine, closes it there.
  - Over 10 more ticks on both machines, seat 2's switch on 0x12 takes the ride's own (skin 0, not
    mxvatv), and seat 1 flies on in the flier. Both machines end on the same hash.
  - A spawn closing nothing is byte-equal to `spawn_on`'s, and a state without the trailer reopens it.
  - Shown to run to its end with the disc (temporary panic probe).
- `tests/lobby.rs` `a_lobby_greys_a_fliers_pack_some_seat_lacks_and_starts_only_in_an_open_one`:
  - The greyed why is exactly `needs MX vs. ATV fliers 1.0.0, missing for 2P`. MX Unleashed's stay
    open and `fliers()` is untouched.
  - `set_flier` is refused while the pack is greyed, and taken once open, with `Pick::packs` =
    `machine/mxvatv_fliers` and `room.needs()` naming it. It is cleared for MX Unleashed's own or on
    the bike.
- `tests/lobby.rs` `a_lobby_seat_that_picked_a_packs_plane_starts_the_ride_in_it`:
  - An `Online::lobby` ride on the disc's `biplane.phy` with a pack skin's template.
  - 2P's pick of `mxvatv_fliers/biplane3` starts it in that plane (`fliers_flown` says
    `2P mxvatv_fliers/biplane3`, `start_flier(1)` = 0x10). When the guest lacks the pack it starts in
    the ride's own (`2P biplane`, byte 0).
  - Shown to run to its end with the disc (temporary panic probe).
- Re-ran, all passing: netplay, online_fliers, menu, lobby, machines_everywhere, the openmx-net lib.
  Full gate in `./kit done`.

## Not verified / not 1:1

- **No two-shell lobby ride run end to end.** No two headless shells were started in a room with a
  pack on one and not the other, and no relayed drop-in was done. The closing is proved on `Rides`
  directly, by a host `seat` → `encode` → another machine's `decode`, not through `admit::grow` and
  `recover::restarted` over a network. The lobby start is proved on one `Online::lobby` with a
  hand-made room, not two. Reason: lobby rides need two networked shells and a Garage pick made
  through the menu. That is a much larger harness than the tests here, and file 2 noted the same gap.
- **The Garage tab in a room is not seen on screen**, not driven over AT-SPI and not screenshotted.
  It is proved through `garage_fliers()` and the shared `flier_rows` code.
- **A pick sent before a lacking joiner's handshake can still race `Room::in_use`.** The host's
  `say_in_use` reads the last tick run. A switch onto a pack's plane that lands after the host last
  said it, but before the joiner's handshake is checked, lets a joiner in that lacks a ridden pack.
  That joiner then decodes a seat flying a skin it has no template for and probes the ride's own
  rays, while the others keep theirs. That is a possible desync, and it was not tested. Closing at the
  join does not cover a seat already up. Reason: the handshake is checked on the network thread
  before the join's tick exists. Holding it to the settled ticks would need the ride to answer the
  handshake.
- **Closed fliers never reopen** when the seat that lacked the pack leaves. A leave is
  tick-synchronised too, so reopening could be done the same way. It was left out so the set only
  ever narrows. The picker says why the row is greyed.
- **Host handover (N11)**: the new host closes by its own room copy and templates. That was not
  tested, the same as file 2's note on handover.
- **Only 1P's pick goes in the lobby pick.** A second local player on the same machine starts on the
  bike or the ride's own plane, as N18's machine pick already does for them.
- **The lobby's flier pick is re-checked only when this machine's room copy changes.** If a lacking
  peer joins the room, the pick is cleared on the next frame and sent again. A Start pressed in that
  frame would carry the old `packs`, and the handshake would refuse the lacking peer. That is a
  refusal, not a desync.
