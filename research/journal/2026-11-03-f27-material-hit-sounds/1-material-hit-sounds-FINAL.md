# The material table's collision sounds

## Who carries the listener

The brief noted a disagreement. S200 said the material listener sits on knocked props' probes. The
oracle named the same function `plane_probe_sound_listener`. I read it off `s195-knock-2881`'s RAM
(table at 0x00e17d90). Both are right, because it goes on every filed collision object:

- The table's interface (+0x30) is registered on the collision node. The node's file calls each
  interface's vt +8, and that attaches the listener (+0x34) to the filed record's +0x1f0 probe.
  Unfiling detaches it.
- In the knock state the listener is on:
  - the four bikes' 11-ray body probes (category 0x18008; not the wheel probe);
  - the parked plane's 41-ray probe;
  - the two beach balls that were woken once and are still filed asleep through
    `DetectCollisionsWhenDisabled`. These have three listeners. The three balls never woken have two,
    without the material one.

So a bike rubbing a fence, a bike on a bike (material 16, `bikehit1`) and a ball on the ground all
play through this path.

## The rule (`game_sim::material_hit::play`)

The table's update runs once a frame over the queue (owner, struck):

1. Each entry's speed is the length of its owner's velocity: the body's, or the chassis' when there
   is no body.
2. A pair queued both ways round keeps the faster entry. The slower one is swap-removed, and the
   later one goes on a tie. The swap-remove order is the original's, entry by entry.
3. The material is the struck collidable's byte, with two exceptions:
   - A struck bike (16) gives the striker's own material. A striker with no collidable keeps the
     previous entry's value, which the update holds in a register.
   - A struck plant (category & the foliage mask) gives `SolidFoliageID`.
4. The first table entry with that ID plays when speed ≥ `CollisionMinImpulse`. Its level is
   `(int)(100 · speed / CollisionMaxImpulse)` clamped to 0..=100, placed at the owner's origin, and
   heard 3D between Min and Max distance. The start flag does not stop the earlier voice, so hits
   overlap.

`audio::HitTable` reads the same keys from the disc, with `[Settings] SolidFoliageID`. Its `sound`
plays each `Sound` as a new one-shot at gain level/100. `Reading::hits(track_res)` loads it.

## Checked

- `game-sim/tests/material_hit.rs` against `s195-knock-2881` (all three pass):
  - `the_states_table_is_the_discs`: the RAM table holds 15 IDs, 30/125 ft, 5/25 (16: 2/9),
    foliage 22, and its clip names.
  - `the_hit_rule_is_the_originals`: 12,000 generated queues and worlds are laid in free RAM and run
    through the original's update with the sound calls hooked. The started sounds, levels and
    positions (bit for bit) equal the port's. Totals: played 23,824, not played 34,651, 8,849 cases
    lost a pair entry, 11,809 struck bikes (923 ownerless), 6,514 plants.
  - Four mutations each fail the comparison: `<=` in the pair test, no carried material, no pair
    drop, no foliage swap.
  - `the_level_and_threshold`: hand values plus 10,000 generated speeds.
- `openmx/tests/material_hit_sound.rs`:
  - the disc's free09 `template.cmt` read by `HitTable` equals the RAM table's numbers for the 15
    IDs, foliage 22, each with a clip (run, not skipped; a wrong foliage value fails it);
  - a frame of three hits plays three voices at 0.6 / 1.0 / 0.5, and the next frame's hit adds a
    fourth without stopping them.

## Not verified / not 1:1

- **Nothing feeds the rule from the ride yet.** Bike body rays, props' probes and the plane's probe
  do not queue hits, so no collision sound is heard in play. Filed as F27a.
- **No PCSX2 capture compared.** The brief's matched-filter test against a Video Capture of a knock
  needs our playback of the same ride, so it is F27a's.
- The register that carries the material starts at 0 in the test (`$s7` set to 0). What it holds
  when the real update is entered was not read. It only matters for a struck bike whose striker has
  no collidable as the queue's first played entry.
- The original's loader keeps only the IDs the track's meshes use. `HitTable` keeps every section.
  A material no mesh has is never struck, so the result is the same.
- The level goes to `Mixer` as a linear gain of level/100, the same mapping the drive-on loop uses.
  The sound object's own volume curve was not measured.
- The original has no distance check before starting. The port does not add one, so a far hit still
  starts a voice that the mixer leaves silent.
