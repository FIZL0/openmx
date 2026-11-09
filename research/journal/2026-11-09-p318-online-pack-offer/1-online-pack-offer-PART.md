# 1 — online pack offer (PART)

Task P318. 2026-11-09.

## What I did

The handshake and room half, which needs nothing from P315g (the offline Switch To Machine picker).
P315g's merge was still under test when this session began, and this branch sits on the main before it,
so the in-ride half (Garage and pause pickers greyed, the pick in the input stream) waits for P318 to
reopen on a main that has it. Blocked `--after P315g`.

Nothing here knows which packs or kinds there are (the user's "all of these should be dynamic"):

- **`openmx_net::Pack`** (handshake.rs): one installed pack, by its `mod.json` kind and id
  (`Pack::key`, `machine/mxvatv_fliers`), a hash of its files, its name and version, and what makes
  one (`how`, its author less the version: `openmx-mods tools/import-atv fliers`). `Disc::packs()`
  (default none) says every pack a machine holds.
- **Hello carries the joiner's packs**, the host puts its own in at hosting, and the room keeps them
  per peer (`Room::packs`, freed with the peer). `Room::offer()` is every pack a seated machine holds,
  each with the seats whose machine lacks it or holds other files under its key (the host's files are
  the reference, else the first holder's); `Offered::why()` reads `needs MX vs. ATV Unleashed biplane
  and Kiowa 1.0.0, missing for 2P`. A lacking peer leaving opens it again; a peer from an older build
  (no packs) lacks every one.
- **What a ride needs**: `Pick::packs` (the packs a seat's picks ride, comma-joined keys) and
  `Room::in_use` (what seats ride now, set by the host's ride with the new `Cmd::InUse`, since a
  mid-ride pick is not in the seats' picks). `Room::needs()` is their union, and the handshake carries
  `packs` and `needs`: `check` refuses a peer missing a needed pack, or holding other files or another
  version under it, naming the pack and the import tool. A pack nobody rides is never a reason to refuse.
  This one check covers both the lobby's Start and a peer dropping in mid-ride, (4) in the brief, once
  the ride keeps `in_use` up to date.
- **The shell's reader** (lobby.rs `installed_packs`): every folder two levels down the mods folder that
  has a `mod.json` (`mods/<anything>/<pack>/`). Kind and id come from the manifest, or from the folder
  names when it doesn't say. The hash covers every file under the folder, path then bytes, in the same
  form as `mod_track_hash`. It reads through `crate::files`, so the page's OPFS mods count too. A pack's
  hash is kept per folder in `DiscFiles` while its `mod.json` bytes and file names are unchanged.
  `DiscFiles` implements `Disc::packs` from the Mods row's folder, and gives none while that row is off.

Opentennis has no mods, so there was nothing there to take; the shape copies this crate's own
trick-pack and track-mod checks (MD19, MD6).

Tests: `openmx-net` lib `every_seat_holds_the_packs_a_seat_rides` (a kind no code knows, `weather/storms`,
held like a machine pack; the missing, other-files and other-version refusals; a host lacking a needed
pack; a handshake from before P318), `the_offer_is_what_every_seat_holds`, `needs_are_the_picks_and_the_ride`;
`openmx` tests/lobby.rs `every_installed_pack_is_said_by_kind_id_and_hash`.

## What the original does

Nothing: online play and mods are openmx's own.

## How it was proved

Unit tests (above). No two-shell run yet: that comes with the ride half.

## Not verified / not 1:1

- **No online ride uses any of this yet.** `main.rs` still sets `a.fliers = Some(false)` online, no
  pick fills `Pick::packs`, and no ride sends `Cmd::InUse`, so `needs` is always empty and no peer is
  refused for a pack today. The offer is computed but nothing draws it yet. Reason: that half lives in
  main.rs/menu.rs, which P315g rewrote, and P315g was not on this branch's base.
- **The two-headless-shell check in the brief** (fliers on one shell and not the other; the Spitfire
  switch mid-ride with settled checksums matching): not run, for the same reason.
- **Hashing cost**: the first handshake after start reads every byte of every installed pack, music
  folders included, on the network thread. 21 MB here, so not measured. A large music pack could delay
  a Hello by about a second. Reason: a cheaper signature (sizes and mtimes) isn't available on the page
  build, and a hash that skipped bytes would let changed files compare as equal.
- **A pack edited in place** (bytes changed, same `mod.json` and file names) keeps its old hash in a
  running shell until restart. Reason: there is no cheap change check on wasm. Imports and the store
  rewrite `mod.json`, which drops the cached hash.
- **Track mods are counted twice**: once by the existing track check, and again as a `track/<id>` pack
  in the offer. This does no harm, because the offer only greys and the track check is unchanged.

## Next

Rebased on a main with P315g (the run reopens P318 after it). Do these:
1. The Garage's Fliers/Machines tabs and the pause's Switch To Machine picker take `room.offer()` while
   online. Grey a pack skin whose `Offered` is not open, and give its `why()` as the reason (P315g's
   greyed-with-why path).
2. Drop the `a.fliers = Some(false)` online override. Fill the lobby `Pick::packs` from the seat's
   roster picks (pack ≠ `original`).
3. Put a mid-ride pick (machine, skin, class, handling) in the input stream the way N17's switch is,
   applied on the agreed tick. Preload every offered pack's `.phy` at room start. The host sends
   `Cmd::InUse` whenever a seat's packs change.
4. The two-shell test from the brief, `--online-script` style (S249).
