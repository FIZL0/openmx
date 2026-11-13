# 1 — s316 free ride seek load (FINAL)

Task S316. 2026-11-13.

## What I did

- **`Sim::seek: Option<Box<ai::seek::World>>`** (crates/game-sim/src/lib.rs). It holds a free ride's
  seeker world. `None` is what every state was before. It is cloned with the Sim. It is not walked by
  the hash or sent by the snapshot, because nothing steps it yet. A decoded snapshot keeps this
  machine's copy (snapshot.rs).
- **`Sim::load_seek(hit_dat, read, slots, open, ground) -> bool`** calls `ai::seek::load` (S313):
  - It reads the two databases that `hit.dat` names, through `read` (a file of the track's folder).
  - The owners are the Sim's riders, in player order. Player 1 is kind 1 and every other player is
    kind 2. Each owner's position is where its bike is now, and `hit_freeride` is false.
  - If `hit.dat` names no databases, or `read` misses one, nothing is loaded and it answers false.
- **The shell calls it at the map load** (crates/openmx/src/main.rs; android_so.rs is a symlink to
  it), right after the freestyle event, on every seat's Sim:
  - It uses the same `hit.dat`.
  - `slots` = the seat's players + 1.
  - `open` = "the first of each progression" for every player.
  - Tracks without hit data (the MX tracks) load nothing.
- **The task text's "hand-built seeker world" is not in the product.** Only the tests had one. The
  Sim had no seeker world at all, so this adds it rather than replacing anything.
- **Nothing reads `Sim::seek` yet.** The AI lane that steps the riders through `ai::seek::seek` is
  S323.
- **research/oracle.txt**: `ai_seek_load` now says where its slot count comes from.
- **opentennis**: it has no counterpart to this, so nothing was taken from it.

## What the original does

- **The caller.** The activity manager's map load (`./kit dis 0x0021b620 0x0021b800`, s3 = the
  manager) builds the seeker root (`./kit fn 0x003a9520`). It then calls the load
  (`./kit fn 0x003a9680`) with three arguments:
  - the root;
  - the `hit.dat` reader;
  - `slots` = the count of the manager's +0x5c list. That list is the activity's players, the same
    owner objects the sets point at: +0x30 is the player number and +0x34 is the kind.
- **The image.** On recording2 (free09) that list holds 5 players:
  - the human, player 0, kind 1;
  - the three AI riders, players 1–3, kind 2;
  - a fifth player at a static address: player 4, kind 2, and no set joined to it.

  So `slots` is 5 there, and only 4 sets are used.
- **The sets.** Riders take them as they join (`./kit fn 0x003a9d60`, `0x003a6600`). The port makes
  one set per owner it is given, in player order.

## How it was proved

**`s316_a_free_ride_sim_loads_its_seeker_world`** (crates/game-sim/tests/ai/seek_world.rs) is a
save-state capture plus the disc.

- **Setup.** A Sim with four riders, each placed where recording2's image has its model root. It
  loads free09's `hit.dat` and its databases from the user's disc.
- **Checked against the image:**
  - 4 sets;
  - each owner's kind and player (1/0, 2/1, 2/2, 2/3);
  - the Sim's players + 1 = the image's slot count, 5.
- **Checked against a direct call.** The rest of the world equals a direct `ai::seek::load` call with
  the same owners. S313's test proves that load field for field against the image (106 targets, 12
  groups).
- **Also checked:**
  - every target carries `slots` done flags;
  - a clone of the Sim carries the world;
  - a `hit.dat` with no databases loads nothing and reads no file.

Result: passes (`./kit check -- -p game-sim --test ai -- s31`, 2 tests). Run alone it took 0.12 s and
printed no skip line, so it really ran.

**A live shell run** (`openmx --track free09 --headless --frames 5 --players 2`) printed
`ai seek: 106 targets, 12 zones, 1 riders, 2 slots`. In the shell, each seat holds its own Sim with
one rider, so each seat's world has one owner, its own rider, as kind 1.

## Not verified / not 1:1

- **The slot count is "players + 1".** In the original it is the count of the activity's player list,
  which on free09 is the 4 riders and one static player. What that fifth player is, and whether the
  list always holds exactly one extra player, was not traced. It holds for recording2's 4-rider free
  ride only. A 1- or 2-human free ride with no AI riders was not captured.
- **Owner kinds.** Player 1 is 1 and the others are 2, as on recording2, where the others are AI
  riders. In the shell today the other players are humans (split screen or online). What the
  original gives a second human's player object at +0x34 was not checked. Only `build` reads the
  kind: it centres the pick lists on the first owner of kind 1.
- **Owner position.** The port uses the bike's position (`ride.position()`) at the load. The original
  uses the model root's node +0x70 when the rider joins, and does not use the bike body. The two are
  not shown equal; the test places each rider at the image's root and checks only that the plumbing
  passes it through.
- **`hit_freeride` is false for every owner.** It is a vtable answer that was not traced (S313 and
  S306 say the same).
- **The shell's open flags are a guess.** It uses "the first of each progression, for every player",
  as if from a fresh profile. The original reads the save profile's progression; that is S315, not
  ported. While `hit_freeride` is false, `build` does not read them.
- **The shell loads the world on every map that has hit data, races included.** The original's load
  sits in the activity manager's map load. Whether a race's activity reaches that call was not
  traced.
- **The seeker world is not in the state hash or the snapshot.** Nothing changes it until the AI
  lane (S323). Once riders seek, its picks, done flags and learnt legs become state that must be
  walked and sent; a note on S323 records that.
- **The shell's seats each load a one-owner world.** Each seat's Sim holds only its own rider, so
  the other seats' riders are not owners there. In the original, one world holds a set for every
  rider. Which riders a seat's world should hold is for the AI lane (S323) to decide, once AI riders
  exist in the Sim.

## Next

None. The AI lane (S323) is what reads `Sim::seek`.
