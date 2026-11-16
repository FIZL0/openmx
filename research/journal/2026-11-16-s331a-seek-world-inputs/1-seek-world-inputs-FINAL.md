# 1 — seek world inputs (FINAL)

Task S331a. 2026-11-16.

## What I did

- **A race builds no seeker world now.** The shell (crates/openmx/src/main.rs) used to load one on
  every map with hit data, races included. It now asks `ai::seek::loads_world(activity)`: a free ride is
  the original's freestyle activity (yes), a race is a single race (no). With no world, `Sim::spawn_ai`
  hands the lane an empty one, as it already did on tracks with no hit data.
- **Slots are the players, not players + 1.** `ai::seek::slots(players)` answers the player count.
- **Zones open to the humans only.** The shell's open closure now opens a place-0 zone for player `p`
  only when `p` is one of the Sim's humans. A free ride's Sim holds only humans, so that is every
  player it has, and never player 1 in a one-player ride. The original skips AI owners (kind 2) and
  opens from each human's profile; S315's `opener` ports that, and the shell still has no profile.
- New in `ai::seek` (crates/game-sim/src/ai.rs): the event's activity types as constants,
  `loads_world` and `slots`.
- research/oracle.txt: `ai_seek_load` notes the gate and the slot rule.
- opentennis has nothing like a seeker world, so nothing was taken from it.

## What the original does

- The activity's map load (`./kit dis 0x0021b620 0x0021b870`) reads the event's activity type
  (`*(*(app+0x3d4)+0xc)+4`) at 0x0021b74c. Unless it is 4 (fast-lap attack) or 5 (freestyle), it
  returns before the seeker root is constructed (`./kit fn 0x003a9520`, the one caller) and before
  the load (`./kit xref 0x003a9680`, one caller). So a single race, practice, ghost race or free ride
  has no seeker world: the root word stays 0.
- The load's slot argument is the activity's player count (manager +0x5c; the players are a linked
  list, the array at +0x64). It is one per rider: free10 and free11 ridden alone hold 1, C33's four
  racers hold 4.
- recording2's session (recording2 and first_recording, the same ride) is the exception: its list holds a
  fifth, static player (player 4, kind 2, no set). The S316 "players + 1" came from it alone.
- The zone opening (S315's notes) skips kind-2 owners, so in C33 only player 0 (the human) has a
  zone open.

## How it was proved

- **Save-state images**, test `s331a_only_freestyle_loads_build_a_seeker_world`
  (crates/game-sim/tests/ai/seek_activity.rs). It reads 10 images: three single races (c31 and s282c
  on sx01 with four racers, s296a nat03), two practices (s196 sx01, s133 Nat11) and five freestyles
  (C33, recording2, first_recording, free10, free11). For each one it checks:
  - `loads_world(activity type)` equals "the activity manager holds a seeker root". It does for all 10.
    The races and practices hold root 0.
  - In each world, the slots the load made (root +0x34) equal `slots(sets the riders took)`, plus one
    for recording2's session.
  - `./kit check -- -p game-sim --test ai -- s331a s316 s313` passed (3 tests). Every image is there,
    so nothing skipped.
- **Shell runs:**
  - `openmx --track free09 --headless --frames 5` printed `ai seek: 106 targets, 12 zones, 1 riders, 1 slots`.
  - `--race --racers 4` on free09 printed no seeker line.
  - On sx01 it still spawned its 3 AI riders.

## Not verified / not 1:1

- **recording2's static fifth player is not explained.** It is a player object in static memory
  (player 4, kind 2, no set). I found no code that writes its address directly. Its session had
  all_ai 0, as free10 and free11 do, so all_ai is not the reason. If some setting adds it, the shell
  would be one slot short in that case. That changes nothing today: the port reads the done flags by
  player number only, so slots past the players are never read.
- **A shell free ride counts as freestyle.** The shell has no activity type. Its free ride (stunt
  zones, the freestyle event) matches the original's freestyle, and the original's activity 0 ("free
  ride") loads nothing on a free area (R7). A fast-lap attack would build a world too, but the shell has
  no such mode.
- **No fast-lap image was read.** That activity 4 builds a world comes from the code's test
  (`type - 4 < 2`), not from a capture.
- **The open flags are still a fresh profile's guess** (place 0 for each human). They are not read off
  the save; that is S315's `opener`, which needs a profile the shell does not have. Only humans are
  opened now, as in the original. While `hit_freeride` is false, nothing reads these flags.
- **A second human's owner kind** (S316's note) is still not checked. This task did not touch it.

## Next

None.
