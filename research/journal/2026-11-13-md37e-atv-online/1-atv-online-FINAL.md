# 1 — atv online (FINAL)

Task MD37e. 2026-11-13.

## What I did

- P318 had already put every installed pack in the handshake by kind, id and hash, with a refusal that
  names the pack and the tool that makes one (`Handshake::check`'s `needs` loop), for any pack a seat's
  pick lists in `Pick::packs`. MD37e rides on that rather than adding an ATV-only path:
  - `openmx_net::Pick::quad` (room.rs): the quad a seat's ride offers, `pack/stem`
    (`mxvatv_atv/atv_Race250P`); empty, and left out of the JSON, for none and from an older build.
  - `Handshake::machines` (handshake.rs): every seat's machine in seat order, `handshake::machine` —
    the start machine (`bike` for the empty word, `biplane`) and `+ quad <pack/stem>` — compared seat
    by seat ("rider 2's machine differs (host: …, you: …)").
  - `menu::Lobby::set_quad` (menu.rs): puts the Garage's Quad row pick (`Menu::quad_pick`, off the
    machine offer the shell reads from the packs) in the lobby pick, and its pack key in
    `Pick::packs` beside a flier's (`joined_packs`, so `set_flier` no longer drops the other's pack).
    It takes the quad only where the room's host holds the same files under the pack as this
    machine. Unlike `set_flier` it does **not** need every seat to hold it: a flier falls back on the
    ride's own biplane, but a quad has no fallback, and the task wants a seat without the pack refused
    by name, not the quad quietly dropped. A guest with the pack the host lacks picks none.
  - main.rs's lobby frame calls `set_quad` after `set_flier`.
- Tests: handshake.rs `every_seat_holds_the_quad_a_seat_rides` (machines, needs, refusals for missing,
  other version and other files; old picks/handshakes); tests/lobby.rs
  `two_peers_on_quads_ride_online_and_one_without_the_pack_is_refused_with_it_named` — two real
  `openmx_net` networks on the LAN, the shell's own `lobby::DiscFiles` hashing each side's mods
  folder: both on quads (host Race 250 P, guest Pro 250 A Sport) start the ride with each seat's quad
  on both machines; a guest with a changed file, another version, or no pack is refused with
  "the pack MX vs. ATV Unleashed quads 1.0.0 (machine/mxvatv_atv)" in the note, and not seated.
- From opentennis: nothing new; its handshake had already been taken over (N3/M2e), and it has no
  per-seat machines.

## What the original does

Nothing: online play and the quad are both additions. No original behaviour is involved.

## How it was proved

Unit test and the two-network lobby test named above. No emulator: nothing of the original's.

## Not verified / not 1:1

- **The online ride does not ride the quad yet.** "Two peers ride ATVs online" is proven to the lobby's
  Start: both machines get a started room whose seats carry their quads, and the handshake passes. The
  ride itself still refuses the quad online (main.rs `ride_atv` with `--online`, and the lobby ride
  clears `a.atv`), because the quad lives in the shell's `Garage`, not in the seat's `Sim`, so it is not
  in the rollback state. That is filed as MD148 (the quad as a lane of the seat's Sim, switched by the
  N17 pad byte). Nothing here reads `Pick::quad` in the ride yet.
- A seat whose machine lacks the quad pack is refused at the host's Start (where the handshake runs),
  not greyed earlier in the Garage, as fliers are; the Garage's Quad row does not show who lacks it.
- A drop-in onto a ride under way: the same `needs` check applies (P318), but no test drives a drop-in
  with a quad seat.
- The test's pack is a stand-in folder (a `mod.json` and two small `.phy` files), not a real
  `tools/import-atv atv` pack; the hash covers whatever files a real one has the same way (P318's
  `installed_packs`).

## Next

Done here. The online ride of the quad itself is MD148.
