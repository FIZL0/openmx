# 2 — online fliers (FINAL)

Task P318. 2026-11-09. The ride half on top of file 1's handshake and room half. The lobby side
(Garage tab, starting a ride in a pack's plane) is split off as P318a.

## What I did

A seat online can fly a flier pack's plane, picked mid-ride, where every seat's machine holds the
pack. No files go between players.

- **`online_fliers.rs` (new).** The ride's flier packs are one machine's packs of kind `machine`, in
  key order: the host's in a lobby ride, the lineup's first seat's machine on `--peer`s. So every
  machine numbers them alike without a list of known packs.
  - A pick is one pad byte, `(slot+1)<<4 | skin`, where skin is the place among that pack's flyable
    plane skins in manifest order (the same order on every machine that holds the same files). 0 is
    the ride's own biplane.
  - `offer()` lists this machine's skins of each ride pack, greyed with `needs <name>, missing for 2P`
    (the room's own wording) where some seat's machine lacks the pack or holds other files under it.
  - A pack this machine lacks gets one greyed row.
  - A pack held here with no plane (the quads) gets no row but keeps its slot.
- **The pick in the input stream.** `net::FLIER` (byte 21; `WIRE` 21→22) sits beside N17's switch
  byte and is a level, like it. `Rides::step`'s switch onto the plane takes
  `template_for(byte, own, fliers)`: the flier's template where one has that byte, else the ride's
  own biplane. So the pick is applied on the tick the switch lands, on every machine, under rollback.
  - The pick takes effect at the seat's next switch onto the plane. A seat already up keeps its
    plane, as offline P315g does.
- **Where the skin lives.** It is `Flight::skin` in the `Sim`, so rollback keeps it.
  - It is not in the hash walk or on the snapshot's wire, so MX Unleashed rides hash and encode as
    before: `size_of::<Sim>()` is unchanged at 12248, and the game-sim hash and snapshot tests pass.
  - `snapshot::decode` keeps the plane's skin.
  - `seat_bytes` (joins and resyncs) writes a `0x80`-flagged preset byte and the skin before the
    snapshot for a flying seat with a skin. `probe` takes the decoded plane's rays and `mxvatv` from
    its skin's template.
  - `remodify` rebuilds each template on the new modifiers (`flier_on`), and replanes a flying seat
    off its skin's stock file.
- **Templates are preloaded.** At an online ride's start, `main.rs` `online_local_fliers` reads every
  flyable plane skin of every machine pack here, as offline P315g reads a pick (`PlaneFiles`). It
  hands them to `Online::fliers_here`.
  - Once the offer is known, `Online` builds one template per open row: at once from the room
    (lobby, drop-in), or for `--peer`s once every peer's packs are in, before tick 0.
  - The template set never changes after that, so no machine's templates change mid-ride.
  - The models are loaded beside the ride's liveries in `plane_drawn`. A seat is drawn in its skin's
    model, picked by its plane's skin byte after `Garage::follow`.
- **How `--peer`s learn each other's packs.** There is no room or handshake on `--peer`s, so each
  machine's packs ride in its saved-markers blob (`online_fliers::wrap`: `P318`, marks length, marks,
  packs as JSON). The session already waits for that blob before tick 0 (P105). An unwrapped blob
  from an older build reads as markers with no packs: that machine lacks every pack.
- **Lobby rides.**
  - `Args::room_machines` gives the mods folder to read flier packs from. The ride's own `mods` is
    `None` on a disc track, so it could not be used.
  - The offer is made again whenever the room changes. This is the brief's narrowing, a joiner
    lacking an unridden pack.
  - The host sends `Cmd::InUse` with the packs its seats fly whenever that set changes
    (`Online::say_in_use`). Through file 1's handshake, a joiner lacking a ridden pack is then
    refused.
- **The pause picker online.** `Menu::set_online_fliers` makes the pause's flier picker work online
  for the first player.
  - Its rows are this machine's, each pack plane greyed with the ride's reason. Helicopters are
    greyed with "Online flies no helicopter" (P216).
  - `Act::Switch` online turns the pauser's pick into the pad byte, or 0 with a message if it is no
    longer offered.
- **`--online-script`** accepts `biplane@pack/skin` (P315g's tape word). If the ride does not offer
  that flier, it prints `fliers: <word> is not offered online: <why>` and does not switch.
- **Last lines.** `online: flier <pack>/<skin> open|greyed: <why>` for each row, and
  `online: fliers 1P bike, 2P mxvatv_fliers/biplane3` before the summary.

## What the original does

Nothing: online play and mods are openmx's own. MX vs. ATV's planes fly on SLUS-21104's steps as
offline (MD80e).

## How it was proved

- `tests/online_fliers.rs` runs two headless `--peer` shells on free09, each with its own `--mods`:
  - (a) Host with `mods/` (MX vs. ATV fliers pack), guest with `--mods off`, both scripted
    `40:biplane@mxvatv_fliers/biplane3`. Both list every pack plane `greyed: needs MX vs. ATV
    Unleashed biplane and Kiowa 1.0.0, missing for 2P`, and both refuse with that reason. Both end
    `1P bike, 2P bike`, with 0 desyncs and the same state hash before tick 300.
  - (b) Both with the pack. The guest picks `biplane3` (model two, the Spitfire) at tick 40 and opens
    the throttle. Both machines switch it at one tick (`vehicle: biplane`) and end
    `2P mxvatv_fliers/biplane3`, with 0 desyncs over 420 ticks and the same hash before tick 300.
  - Also in that file: unit tests of the byte, the offer and who lacks what (key order, other files,
    a machine that says no packs), and wrap/unwrap.
- `tests/netplay.rs` `a_flier_pick_takes_its_template_and_its_skin_rides_the_seat_bytes`: the byte
  takes the template and an unknown byte takes the ride's own. The skin survives a join's seat bytes
  and a resync's decode, with the hash unchanged.
- Re-ran, all passing: netplay, markers, markers_online, online_put_back, online_wreck_sound,
  machines_everywhere, menu, online_car, and game-sim hash/snapshot.

## Not verified / not 1:1

- **Lobby rides are not run end to end.** No two-shell lobby test was run: the brief asked for
  `--peer` shells, and lobby tests need a relay. That covers the offer from `room.packs`, the
  `Cmd::InUse` send, a joiner refused for a ridden pack, and the narrowing on a joiner lacking an
  unridden one. All of it is code reading plus file 1's unit tests.
- **The pause picker online is not run on screen.** It was not driven over AT-SPI. That includes its
  greyed rows and the pick reaching the pad.
- **The pack plane's model and its seat online are not seen.** No screenshot. The loading code is the
  offline P315g/P316 path.
- **Narrowing is not tick-synchronised.** Each machine greys the picker when its copy of the room
  changes. Templates never narrow, so a pick sent just before a lacking joiner lands would fly the
  pack on the old machines and the ride's own plane on the joiner's: a desync. Filed as P318a.
- **Host handover.** Under N11 the reference machine becomes another one. The templates and slots
  stay as built, but a recomputed offer reads the new host's packs. If the two machines' pack sets
  differ, slots could renumber in the picker while templates keep the old numbers. Not tested.
- **Packs are hashed whole.** A machine missing one skin's texture but holding the pack under other
  files is refused for the whole pack, not drawn in another skin.
- **Pack planes online fly without hull or bike mesh**, as the ride's own online plane does (`plane_on`).
  Offline flights have both.
- **Every plane skin's model and `.phy` is preloaded** at ride start, not on first use. Load time and
  memory with large packs were not measured.
- **MX Unleashed's second livery** row online picks byte 0, the ride's own biplane in the seat's
  livery, not that livery.
- **Out of scope here, filed as P318a:** the Garage's Fliers tab in the lobby (greyed by
  `room.offer()`), starting a lobby ride in a pack's plane, and `Pick::packs` (left empty because no
  seat starts on a pack's plane yet).
- The pack name in the reason carries the version (`… Kiowa 1.0.0`), because `installed_packs`
  names packs with it. The brief's example was written without it.
