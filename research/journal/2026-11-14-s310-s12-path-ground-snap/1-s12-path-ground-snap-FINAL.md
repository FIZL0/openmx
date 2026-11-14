# 1 — s12 path ground snap (FINAL)

Task S310. 2026-11-14.

## What I did

- `ai::path::Net::load(file, terrain, props)` (crates/game-sim/src/ai.rs): a race file's net as the race
  activity's load leaves it — `TrackFile::snap_to_ground` (S296) on the terrain, `raise_on_props` (S296a)
  on the flagged nodes when a scene is given, then `TrackPaths::new` and `Net::from_track`. `from_track`'s
  doc no longer claims a ray-down-from-1000 snap (S296 corrected that: terrain with a floor at 0, the prop
  sweep only for `word_04 & 0x80` nodes, first point only).
- The task line named `race.rs`; the snap itself was already there (S296), so the only new code is the
  net's constructor, which belongs beside `Net` in ai.rs. Nothing in the Sim or the shell builds an AI net
  yet (S331's lane spawns without one), so `Net::load` has no caller beyond the test today; whoever wires
  the follower's net into the Sim should build it with `Net::load`, not `from_track`.
- Nothing taken from opentennis: it has no path net.

## What the original does

`./kit fn 0x002f8b40` (activity_race_load): each of the two race files (`<track>.tdf` → act+0xb8,
`<track>AI.tdf` → act+0xbc) is read, then every node's three points go to y 0, then onto the terrain's
ground query with a floor at 0; nodes with `word_04 & 0x80` get the downward prop sweep that lifts the
first point. The snap writes only heights and the path walk reads only flags and links, so walking before
or after the snap is the same.

## How it was proved

A save-state RAM image: `sx01s_path_nets_off_the_disc_are_the_race_images` (crates/game-sim/tests/ai/path_nets.rs)
now builds both of sx01's nets with `Net::load` on sx01's terrain (`Tracks\sx01.res`'s scene and
`.ps2.trn`) and compares every node's centre and two edges on **all four lanes**, bit for bit, against
s282c-sx01-race.ee.bin's act+0xb8 (96 nodes) and act+0xbc (444 nodes) — before S310 it compared x, z, w
only. The existing 10,000-case locate oracle on those nets still runs on the snapped nets. Checked
that the test runs to its end rather than skipping (a temporary panic after its last assert fired).

## Not verified / not 1:1

- The prop raise through `Net::load` (`props: Some(..)`) is not exercised on an image here: sx01's two files
  have no flagged node (asserted). The raise itself is proved by S296a on nat03/sx06/sx08/sx11/sx20 at the
  `TrackFile` level; `Net::load` only calls it.
- Nothing in the Sim calls `Net::load` yet: no AI net is held by the Sim or the shell (the follower's net
  wiring is not this task), so no ride or replay reads the snapped heights yet.

## Next

None for S310. Whoever gives the Sim the follower's AI net builds it with `Net::load`.
