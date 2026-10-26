# 1 — online trick packs (FINAL)

Task MD19. 2026-10-26.

## What I did

MD16 forced every online ride onto the original's tricks ("until MD6's check"). MD6 is still open, so its
check for a trick pack is done here, the same way the handshake already holds every machine to one track
and one set of bike files: a name plus a hash of the bytes each machine would load.

- `openmx_net::Room::tricks`: the id of the host's MX vs. ATV trick pack (`mods/tricks/<id>/mod.json`'s
  `id`), empty for the original's tricks (also what an older build sends; serde default).
- `Handshake::tricks` / `tricks_hash` and `Disc::tricks(id)`: each peer hashes its own pack of that id;
  `check` refuses a different id ("the trick pack differs (host: …, you: …)"), a missing pack ("the host's
  trick pack mxvatv isn't in your mods folder") and a different one ("… isn't the same in your mods folder
  as in the host's").
- `trick_packs::by_id` (a pack by its id, not just the first) and `Pack::hash`: FNV-1a over `mod.json`,
  the stunt table and every `.phy` the pack lists, each prefixed by its name. Clips are drawn, not
  simulated, so they are not hashed.
- `lobby.rs`: `DiscFiles` hashes packs from the mods folder the front end's Mods row gives
  (`tricks_from`, set every `Network::pump`), and `name_tricks` fills the room of a `Cmd::Host` /
  `Cmd::Quick` from the host's Tricks row (`room_tricks`: the first pack, as `ride_trick_pack` picks it).
- `main.rs`: the lobby's ride no longer forces `Tricks::Original`; it rides the room's tricks, with
  `Args::room_tricks` = (this machine's mods folder, the room's id). `ride_trick_pack` loads that pack
  by id and makes the ride fail rather than silently ride other tricks than the other seats. Every
  seat's bike file in the lobby ride and the drop-in ride (N10) gets the pack laid over it
  (`lay_room_tricks`, on the set's own `.phy` name, as the local ride does). "Open to Online" (N10) puts
  its own ride's pack in the room — before this, a local MX vs. ATV ride opened online would have had
  joiners riding the original's tricks against a host on the pack.
- `--online` (no lobby, no handshake) stays on the original's tricks; its line now says why.

From opentennis: nothing new — the handshake structure (name + hash, refusal in words) is the one N3
already took from `hst-base/src/handshake.rs`.

## What the original does

Nothing: MX Unleashed has no trick packs and its online play is not ported. This is the port's own rule.

## How it was proved

- `openmx-net` unit test `every_seat_rides_the_hosts_trick_pack`: same pack joins, another hash, no pack and
  another id are each refused with the pack named; a room serialized without the field reads as the original's.
- `openmx` bin unit test `trick_packs::a_pack_is_found_by_its_id_and_a_changed_file_hashes_otherwise`:
  the same files in two folders hash alike; a changed table or a changed `.phy` hashes otherwise.
- `tests/lobby.rs` `every_seat_of_an_online_ride_has_the_hosts_trick_pack`: two real LAN networks with the
  shell's own `DiscFiles`, generated packs in two temp mods folders. The host's menu (Tricks MX vs. ATV)
  opens a room naming `mxvatv`; a guest with the same files rides (both get `Act::Online`, the guest's
  start room says `mxvatv`); a guest whose `mx_race125p.phy` differs is refused: "Can't ride: the host's
  trick pack mxvatv isn't the same in your mods folder as in the host's."
- Gate: `./kit check --all`, green (see the done commit).

## Not verified / not 1:1

- The shell's lobby ride with a pack was not ridden live on two machines: the lobby's ride path in
  `main.rs` (`room_tricks` → `trick_packs::by_id` → `lay_room_tricks` on every seat) is covered only by
  the build. There is no unattended way to drive two shells' menus into a lobby ride (the headless
  `--online` route has no lobby or handshake), so no rollback state hash on a pack was compared between
  machines. The functions it calls are the ones MD16's local ride proved.
- The host changing its Tricks row while its lobby is open does not change the room: the room names the
  pack the Tricks row said when the lobby was opened. And a pack's files edited between the handshake and
  the ride's load are not caught: every machine reads them again when the ride loads, as it does the disc.
- The lobby screen does not show the room's trick pack; a guest without it learns at the host's Start,
  as with a track or bike file that differs (MD6's track mods would be the place to add a line for both).
- `--online` stays on the original's tricks: no handshake there to check the pack.

## Next

None; the task lands.
