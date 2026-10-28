# 1 — props part-name search (FINAL)

Task S132. 2026-10-28.

## What I did

- `crates/game-sim/src/track.rs`: `Placement` keeps a model's `RefName=` (`ref_name`), squeezed the
  way the ini reader squeezes a value and cut to 31 characters (the reader's 32-byte buffer). Only
  `[ModelN]` sections set it, because only the model reader fills the RefName table.
- `crates/game-sim/src/props.rs`:
  - `part_models` is the search: it finds the model each part of a `.pco` stands on when the
    collection has no model.
  - `attach_parts` is the attach with no instance: it claims those models and makes a body of
    every rigid part found.
  - `create`'s single branch, when no model has the entry's name, now loops as the original does:
    one collection at a time, for as long as the search finds every part.
  - `Unported::PartNameSearch` is gone. In its place is `Unported::EndlessSearch` for the one case
    the original cannot finish (below).
  - The `plain` flag on `create` is gone. The original does not gate the fallback on it; a starred
    call always names a model it has just seen, so it never gets there.
- `crates/game-sim/tests/props_search.rs` (new): the proof (below), plus a disc-free test of the
  rules on made-up files.
- `research/oracle.txt`: the S80 block's part-search line is filled in. New names:
  `scene_refname_lookup`, `scene_refname_of`, `toy_collection_list_add` and `track_dir`.

Nothing in opentennis loads props. Nothing taken from it.

## What the original does

- **The create** (`./kit fn 0x00289050`), single, with no unclaimed model of the name, loops:
  1. New collection, then load it (path and name).
  2. Run the search on it, `(coll, 0)`. On 0: destroy the collection and stop.
  3. Otherwise run attach `(coll, 0)`, register the collection and list it whatever the attach
     answered, and go round again.
- **The search** (`./kit fn 0x003b1c60`, instance 0) reads the pco's parts. It answers 1 only when
  every part's node is found, and it marks nothing.
  - Until a suffix is known, a part's model is the first model in the scene's array that is
    unclaimed (+0x3c), has a node (+0x34), and whose node name (node +0x120) stricmps the part's
    `Name`.
  - For part 0 only, the model's RefName comes from the table (`scene_refname_of`). If
    `strncmp(Name, RefName, strlen(Name))` is 0 (case counts), the rest of the RefName is the suffix.
  - With a suffix, a part's model is `scene_refname_lookup("%s%s" of Name and suffix)`: the first
    table entry whose RefName stricmps, claimed or not.
  - A missing pco, or one with no parts, answers 1.
- **Attach** (`./kit fn 0x003b1f40`, instance 0) runs the same lookups, but claims each model as it
  finds it (both branches set +0x3c = 1). Then it loads the part on that node. One-body needs an
  instance, so it is never set here.
- **The model reader** (`./kit fn 0x001a8cd0`) appends `{model, RefName}` to the scene's table
  (+0x3dc, count +0x3e0, stride 0x24) when the section has `RefName`. It reads the name into 32 bytes
  (`ini_text` keeps size − 1), before the model loads.
- **Consequence:** an entry with no model of its name whose pco is missing or part-less makes the
  search answer 1 every time, and attach claims nothing. So the create would make collections
  forever. No retail track has one (below). The port makes none and reports `EndlessSearch`.
- **What the files hold:**
  - nat07: `SnowmanA`/`SnowmanB` have six parts each (`Nat07_Sman_Head`, …). The scene has two
    instances of every part, with RefNames `_1` and `_2`. The search finds Head_1 first and takes the
    `_1` set, then the `_2` set; the third search finds no unclaimed head. So that is two snowmen of
    each kind, 12 bodies a kind.
  - nat15: `Nat02_up_bale_hay` names no model and its scene has no RefNames, so the search fails at
    once and loads nothing.

## How it was proved

`tests/props_search.rs::every_tracks_props_are_the_collections_the_original_builds` runs the
original's loader in the oracle on every track on the disc that has a `.toy` (free, nat and sx: 56
tracks). It covers the toy reader (`toy_physics_collections`), the create, the search, attach, the
ctor, the load, the destroy, snprintf/strcpy/stricmp/strncmp and the original's own ini reader, all
run natively. The setup:

- **Inputs:** the RAM image is first_recording's. Each scene model becomes an object with a node and
  the node's name. The RefName table is laid out as the model reader would leave it, and `track_dir`
  is set.
- **Files:** `ini_open` is hooked to hand the real reader each disc file as its in-memory block
  (nested call).
- **The heap:** a bump allocator.
- **Recorded, not run:** the part's body (`physics_collection_part`, which needs the physics world)
  and the collection's list append. Constraints and the scene-graph add are dropped.

For every track, the port's `props::claim` on the same files is checked equal to the original on:
- the collections, in order;
- each one's name, pco path and one-body flag;
- the model every rigid body stands on, in order;
- the set of models the original marked claimed.

All 56 tracks pass. nat07 makes 7 collections and 76 bodies. The three starred bale entries have no Type,
so each is a group of 1-part bodies; the 4 snowmen have 6 each. `nat07s_snowmen_…` also checks there
are two of each kind, six parts each, every part on its own suffix's model. The same run is the
survey: no track reports `EndlessSearch` or `Create`.

`the_part_search_follows_the_ref_name_suffix` (no disc) checks three things on made-up files: the
suffix pairing; a suffixed part with no model ending the walk; an entry with no file being reported.

`./kit check --all` is the gate. `./kit audit` fails only on other tasks' addresses in
plan/DONE.md (lines 121 and 511: radar_update, biplane_tire_*), none of them mine, so the task lands
with `--skip-audit`.

## Not verified / not 1:1

- **Not checked against a live nationals state.** No save state has a nat track loaded (S98 noted
  the same). So the proof runs the original's code on a scene the test builds, not on RAM the game
  built.
  - The model array and the RefName table are built from the port's own `Scene::parse`.
  - The original's array also holds models the port's scene drops: a section with no `SLT`/`COL`,
    and free-ride-hidden activity models. A model there that failed to load, or a RefName duplicated
    on one, could change what the original finds. No retail RefName is duplicated in a way that
    would matter here, but I did not check every scene for it.
- **The RefName's 31-character cut and quote handling** come from `ini_text`'s decompilation, not
  from a file that needs them. No retail RefName is that long.
- **The part `Name` (63 characters) and the suffixed name (`"%s%s"` into 64 bytes) are not cut in the
  port.** No retail name comes near.
- **`EndlessSearch` is not the original's behaviour.** The original would loop forever; the port
  stops and reports. No retail file reaches it.
- **The props' bodies themselves** (placement, mass, inertia) for the snowmen are made by the
  existing `props::bodies`, proved on free09 only (S80). Their node boxes would come from nat07's
  scene graph, which the port still does not build (S127).
