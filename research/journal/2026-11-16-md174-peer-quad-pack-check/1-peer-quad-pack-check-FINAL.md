# 1 — peer quad pack check (FINAL)

Task MD174. 2026-11-16.

## What I did

A `--peer` ride now refuses a seat's quad unless every machine holds that quad pack with the same files.

- **New `crates/openmx/src/online_quads.rs`** (as P318 put its pure parts in `online_fliers.rs`):
  - `Card`: a machine's quad pick (`pack/stem`) and every quad pack it holds, by id, with its files' hash.
  - `wrap` / `unwrap`: the card around the P318 blob. Still starts `Q152`. Then the pick's length and the pick,
    as before, then a count of packs and each pack's id length, id and 8-byte hash, then the P318 blob.
    MD152's `wrap_quad` / `unwrap_quad` / `QUAD_WRAP` moved here and grew the pack list.
  - A machine with no pick but quad packs still sends its card: other seats' picks need its hashes.
  - `refused(cards, seat)`: why a seat's pick is refused, or `None`. A pick rides only when every seat's
    machine holds the pick's pack with the hash the picker's machine sent. Every machine hears the same
    cards, so every machine makes the same call.
- **`online.rs`**: `quad_mine` is now a `Card`. `take_markers` collects every seat's card, prints
  `online: seat N picked the quad pack/stem, refused on every machine: <why>` for a refused pick, and builds
  only the picks that are not refused.
- **`main.rs`**: `quad_card` builds this machine's card: the pick (as before), plus every `atv` machine pack in
  the mods folder, hashed with the lobby's pack hash.
- **`lobby.rs`**: `folder_hash`, a public wrapper over the lobby's own `pack_files` + `pack_hash`. It goes through
  `crate::files`, so phones work too. So a `--peer` pack hash is the same number the MD37e handshake compares.
- Lobby rides and drop-ins have not changed. They still trust MD37e's handshake.
- opentennis: its handshake (`hst-base/src/handshake.rs`) already gave this repo the pack hash scheme. Nothing
  else from there fit a rideless `--peer` exchange.

## What the original does

Nothing. Online play and quad packs are openmx's own.

## How it was proved

- `tests/online_atv.rs`:
  - `a_quad_card_wraps_the_markers_and_reads_back`: a card with a pick and packs reads back. So does a card
    with packs and no pick. An empty card adds nothing. Unwrapped or cut-short bytes give an empty card and
    the bytes as they were.
  - `a_quad_pick_rides_only_where_every_machine_holds_the_same_pack`: same hash → rides. Other hash → refused,
    for both seats. Pack missing on the other machine → refused. Picker sent no hash → refused. Pick with no
    pack in it → refused.
  - `two_online_shells_refuse_quads_whose_packs_differ`: two headless `--peer` shells on free09 for 320 ticks.
    The guest's mods folder is a copy of the quad pack plus one extra file. Both machines print both refusals,
    build no quad and do not switch. Both report 0 desyncs, and both have the same state before tick 300.
  - MD152's `two_online_shells_both_ride_their_quads_and_agree` still passes with the new card. Same pack on
    both machines → both quads built and ridden.
  - I checked both two-shell tests really ran, not skipped, with a temporary panic that printed their output.
    In the refusal run the guest printed `0 desyncs, state before tick 300 0x8e3bb318c8ec1e13`.
    This worktree had no `mods/machines/mxvatv_atv`, so I copied it from slot s1's untracked `mods/` (git-ignored).

## Not verified / not 1:1

- **A machine missing the pack entirely is proven by the unit test only.** The two-shell test covers "other
  files under it" (one extra file). "No such pack" takes the same `refused` path, but no two-shell run covers it.
- **`--online-script atv` still asks for the switch** after a refusal (`vehicle: atv at the next tick every
  machine agrees`). The script never checked whether the seat has a quad, but the pause menu does and offers
  none. Every machine has no quad for that seat, so the switch does nothing everywhere. The test shows no
  `vehicle: atv` switch line, no `online: quads` line and 0 desyncs.
- **Builds of different versions talking over `--peer`** read each other's `Q152` card wrongly: the layout after
  the pick changed. `--peer` has no version check at all, so this was never supported. Not tested.
- **Hashing cost**: each online ride start now hashes every quad pack's files (about 14 MB for `mxvatv_atv`),
  lobby rides too. Not timed.
- **Lobby and drop-in rides** still rely on MD37e's handshake and do not use the card. Not retested here.

## Next

Nothing for this task.
