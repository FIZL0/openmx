# 1 — freeride on a track mod opens online (FINAL)

Task F6. 2026-11-04.

## What I did

- `crates/openmx/src/main.rs`: the pause's "Open to Online" (N10) no longer needs `modded.is_none()`.
  The new `opens_on_mod(ride, world_mod, mods, track)` decides it: a disc track opens (as before, and a
  disc world, P232, too, as before: its `modded` is `None`); a world mod (MD8, `modded` and `world_mod`
  both set) does not, since the handshake hashes only one track mod's folder; a track mod opens only
  when the settings' Mods row folder, which `lobby::DiscFiles` hashes for the handshake
  (`track`/`track_mod`) and which the joiner's ride is built from, finds the track in the same folder
  the ride was built from. So a `--mods DIR` ride, or one whose Mods row is off, stays closed: its
  handshake would hash nothing (or another copy) and every joiner would be refused with a misleading
  "your disc's ... isn't the same".
- The joiner's side needed no change: a drop-in arrives as `Event::DropIn` → `Act::Online`, the same
  path as a lobby's ride, which already sets `mods` to this machine's Mods folder when it holds the
  room's track (MD6). The room's track is `args.track`, the mod's id.
- Nothing taken from opentennis: it has no track mods.

## What the original does

Nothing: online and mods are additions (the original has neither).

## How it was proved

- Unit test `opens_on_mod::a_track_mod_opens_where_the_lobby_finds_it` (main.rs): two generated mods
  folders holding the same track mod; disc track / disc world open, the mod opens on its own folder and
  not on the other folder's copy, a missing folder, the Mods row off, or as a world mod.

## Not verified / not 1:1

- No end-to-end run of a host opening a freeride on a track mod and a second shell dropping in: the
  netplay tests drive the session (track-independent `Ride` sims), and the shell's pause cannot be
  driven headless into Open to Online. The joiner's path is the already-tested MD6 lobby path, read,
  not run here.
- The check is made when the ride starts (and again on a player leaving, through `opens_alone`); a
  Mods row switched off in the pause's Settings afterwards is not re-checked before opening: the
  handshake then refuses joiners (with the generic "isn't the same" line), it does not desync.

## Next

none
