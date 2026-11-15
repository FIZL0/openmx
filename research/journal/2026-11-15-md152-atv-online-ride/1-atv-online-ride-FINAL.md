# 1 — atv online ride (FINAL)

Task MD152. 2026-11-15.

## What I did

A seat online can switch to its quad. Every machine runs every seat's quad, under rollback.

- **The quad is a lane of the seat's `Sim`.** `game_sim::quad::Lane` (new, end of `quad.rs`) holds
  the `quad::Ride`, its trigger segments, `was_down`, where it was taken and its wrecks.
  `Sim::quad: Option<Box<Lane>>` sits beside `Sim::car`. It is cloned with the `Sim`, so ggrs saves
  and restores it.
  - It is hashed (`hash.rs`, blob `quad`) only while a quad is out. So MX Unleashed rides hash as
    before. `size_of::<Sim>()` is 12280 (was 12272).
  - The hash bytes are the `Debug` print of the ride's moving parts (`Lane::hash_bytes`). `quad::Ride`
    has dozens of nested types; a hand walk of each was too big for this task. `f32`'s `Debug` prints
    the shortest round-trip text, so two different numbers print differently.
- **The switch is pad byte 6.** `vehicle::Wanted` gives `Kind::Atv` the machine number 6 (bits 0, 1
  and 7). `net/rides.rs` hands each seat's quad copy to `vehicle::switch_lane_quad`. That builds an
  offline `Garage` with the quad and runs the same `Garage::switch`, then moves the quad out into
  `Sim::quad`. Off the quad, the machine taken spawns where the quad stands (`quad_spot`).
- **The step.** `vehicle::ride_lane` runs the track's path cars, then `quad::Ride::step` on the
  seat's hands, as offline's `Garage::step_among` does. The bike is not stepped.
- **Each seat's quad copy.** `Online::quads_here(mine, reader)`:
  - A lobby ride (and a drop-in) builds each seat's quad from `room.seats[s].pick.quad`
    (`pack/stem`). MD37e's handshake means every machine holds the same pack files.
  - `--peer`s have no room. Each machine's pick goes out wrapped around its P318 markers blob
    (`online::wrap_quad`: `Q152`, length byte, key, then the P318 blob). The quads are built once every
    peer's blob is in, before tick 0.
  - `main.rs`'s `quad_reader` reads a key off the pack in this machine's mods folder.
  - A ride opened to online builds its own seat's quad, and puts the pick (and its pack, for
    MD37e's handshake) in the room's seat. When the room changes, every machine builds the quad of any
    seat whose pick differs from the one built and is not on it (`Online::quads_from_room`). A joiner
    builds every seat's off the same room.
  - Each quad is made on the ride's modifiers. A host's mid-ride change updates them
    (`Modded::remodify`); a quad out keeps its numbers until its next take, as offline (P287).
- **The shell.** `ride_atv` no longer refuses online. The lobby's ride takes this machine's pick's
  stem as `--atv` instead of clearing it. The pause's switch (and `--online-script`'s `atv`) puts the
  quad in the pad when this seat has one. A drawn seat on its quad gets a copy of its seat's quad in
  its garage, and `Garage::follow` copies the lane's quad into it each frame.
- **Last line:** `online: quads 1P atv_Race250P 768.2 ft 0 wrecks, 2P atv_Pro250A 657.9 ft 1 wrecks`
  (feet from where it was taken), and `online: seat N quad pack/stem` when each is built.

## What the original does

Nothing: online play and quad packs are openmx's own. The quad steps on MD123's port as offline.

## How it was proved

- `tests/online_atv.rs` `two_online_shells_both_ride_their_quads_and_agree`: two headless `--peer`
  shells on free09 for 420 ticks, `--mods mods`, the host on `atv_Race250P` and the guest on
  `atv_Pro250A`. Host script `40:atv,41:gas`, guest `60:atv,61:gas`.
  - Both machines print both seats' quads, both switches, and 0 desyncs.
  - Both end on the same quads line (768.2 ft and 657.9 ft, the guest's having wrecked once).
  - Both have the same state hash before tick 300 (`0xe338563344932d0c` in the run I read).
  - I checked it really ran (not skipped) with a temporary panic printing both outputs.
- Same file: the quad's pad byte is 6 and reads back as the quad.
- The full gate ran in `./kit done MD152`. The two-shell test above was last run before the drop-in
  room rebuild (`quads_from_room`) and the opened ride's room pick were added; those were only built.

## Not verified / not 1:1

- **The quad is not on the snapshot wire.** `Rides::encode` returns `None` while any seat has a quad
  out (`snapshot::decode` builds `quad: None`). So while a quad is out: a resync says it cannot send
  its state and the desync is not mended; a drop-in waits; a restart after a leave ends the session.
  Filed as MD168.
- **The hash is a `Debug` print**, not a field walk. NaNs with different payloads print alike; any
  other difference shows. It is slow next to a field walk (a string per seat on the quad per tick).
  Not timed.
- **Quads are out of the contact lists.** A seat on its quad is in no other seat's contacts and has
  none, so riders pass through it online. Offline they do not. Filed as MD169.
- **`--peer`s do not check the pack.** With no handshake, a machine missing the pack builds no quad
  for that seat (and says so), and the ride desyncs when the seat switches. Filed as MD170.
- **Every seat's quad is drawn in this machine's look**, and online quads tick no clips (the rider
  sits still on it). No screenshot was taken. Filed as MD171.
- **Lobby rides are not run end to end.** No relay test: the room path (`pick.quad`, the lobby's
  `--atv`) is code reading only.
- **Drop-in (a ride opened to online, a joiner) is not tested.** The joiner's quad is built when the
  room says its seat, which should come before the join is taken on; that order was not tested. An
  admit waits while any quad is out (see above).
- **The quad's camera, engine and particles** online are the offline ones for this machine's seat,
  untested online.
- `online: fliers 1P bike, 2P bike` still says `bike` for a seat on its quad (P318's line says
  only planes).

## Next

Nothing for this task. The follow-ups are MD168–MD171.
