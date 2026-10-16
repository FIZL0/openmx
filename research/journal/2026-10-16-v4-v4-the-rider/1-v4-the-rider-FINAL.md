# 1 — v4 the rider (FINAL)

Task V4. 2026-10-16.

## What I did

New module `crates/game-render/src/rider.rs` and disc-backed tests `crates/game-render/tests/rider.rs`;
`first-draw` gained a `--rider` flag (default `thq1`, `none` disables) and draws the body on the bike.
Everything is additive — nothing existing moved, renamed or reformatted.

The module reads the three-file chain and nothing more:

```text
  Rider\thq1.res                                the archive, named after the rider
    rider\anim\thq1.sub     [Rider] MCF = rider.mcf    ->  the motion set
                            [Base Materials] / [Material Changes]
    rider\anim\mrider.mcf   SLTFile = mrider.Slt       ->  the geometry
                            VUTfilename, RotationType, NumberOfMotions, [MOTION n]
    rider\anim\mrider.psg   22 nodes, 30 meshes, 5 LOD groups, 2035 triangles at LOD 0
```

* `Suit::parse` — a rider's `.sub`. Same shape as a bike's, minus the `[SubObjectN]` parts.
* `MotionSet::parse` — the `.mcf`. `geometry()` turns `SLTFile=mrider.Slt` into `mrider`, because `SLT`
  is the editors' name for the format and the archive member is a `.psg`.
* `Skeleton::of(&Model)` — the node tree as bones: local matrix, parent, and the composed rest matrix.
* `bolt_onto(rider, chassis, anchors)` — the translation that puts the mean of the rider's named anchors
  on the mean of the chassis's.
* `Rider::load(&[Res], name, sex)` — the whole chain, returning the geometry name the caller loads
  through `Assets` and the material pairs it repaints with.

Three things the data said that are worth keeping:

**The `m` in `mrider` is the body's sex.** The `.sub` says `MCF = rider.mcf`; the loader puts `m` or `f`
in front. No archive on this disc ships an `frider.mcf`, so `Sex::Female` must *fail* rather than quietly
serve the male body — which is what suffix matching did, since `mrider.mcf` ends with `rider.mcf`. The AI
`.sub` names `airider.mcf` outright, and the `ff_*` archives prefix their members with the mode, so the
rule that works is `names_it`: strip the asked name from the member's basename and accept only if what is
left is empty or ends in `_`. That covers `mrider.mcf`, `ff_mrider.mcf`, `airider.mcf`, `ff_airider.mcf`
and refuses the male or the AI body to a `Female` request.

**The gear is a repaint, not a model.** The body's meshes name `THQ_Helmet_01`, `THQ_Pants_01` and four
more; a licensed rider's `[Material Changes]` maps them to `Thor_Helmet_01`, `Alpinestars_Boots_01` and
so on, whose `.psm`/`.pst` its own archive carries. That is the same mechanism a bike's `.sub` uses to
paint the shared chassis, so `Assets::rename_materials` already did the job and no rendering code was
added for it.

**The bind pose is already drawable.** Every one of the 22 nodes has an identity rotation, so the node
tree is pure bone offsets, and the vertices are stored in bone space with the node index in the
position's `w` exactly as in any other `.psg`. `Model::triangles` therefore yields the rest pose — a
T-pose — with no skinning maths at all. Skinning proper has nothing to do until there are motions.

## What the original does

Not read from the decompilation. Everything above is the file formats, read from the bytes on the disc;
no retail function was disassembled for this task, so this section is deliberately empty and the claims
it would have supported are listed below instead.

The one structural claim the files do make: the rider's four `Dummy_hand_L/R`, `Dummy_foot_L/R` nodes
hold no geometry and exist only as targets, and the bike's chassis has four nodes with the same names at
its grips and pegs. Those four names are the entire intersection of the two models' node names, which is
what makes them the bolt-on points rather than a guess among many.

## How it was proved

Disc-backed tests, run through `./kit check`; five integration tests over the real archives plus six unit
tests on literal file text. The sweep covers all 164 `Rider\*.res` and both `.sub`s in each (162 player,
162 pack — `custom`/`ff_custom` and `ghost_ai_01`/`ff_ghost_ai_01` ship one each, which is why it is not
326):

* `the_three_files_of_a_rider_name_each_other` — thq1's chain end to end, 40 motions, `Sex::Female`
  errors, `thq1_ai` resolves to `airider`.
* `every_rider_archive_holds_the_same_skeleton` — every archive's body has the same 22 bone names, every
  rest rotation is exactly identity (worst error 0.0), and every vertex lies inside its own node's stored
  bounds to within 1e-4. That last one is the proof that the bone index read out of the position's `w` is
  the right one: across all 164 archives the worst excursion is 0.00005 feet. Also: every gear material
  named by a `[Material Changes]` resolves to a `.psm` and a `.pst` in that rider's own archive.
* `a_licensed_riders_gear_is_a_repaint_its_own_archive_carries` — reed1's six base materials and six
  changes.
* `the_rider_bolts_onto_the_chassis_at_its_pegs` — the four shared names; the feet midpoint lands on the
  pegs midpoint to 1e-5; the pelvis sits above the chassis's top and the head above the bars; nothing
  ends up below the wheels; and the hand anchors, used instead, put the feet underground — which is the
  measurement behind choosing the feet.
* `the_rider_is_built_the_way_a_bike_is` — `Suit::parse` agrees with the existing `Vehicle::parse` on
  `250sx.sub`'s `[Material Changes]`, and `bike.mcf` parses as a `MotionSet` whose `geometry()` is
  `bike`.

Looked right: `first-draw --track sx19` draws 58891 triangles where V1 drew 56856, the difference being
exactly the rider's 2035, and a side-on render of the chassis, its seven `.sub` parts and the bolted
body shows the boots at the peg line and the body on the bike rather than through it. Both renders are
in the session scratchpad, not committed (`context/` and screenshots never are).

## Not verified / not 1:1

* **The bind pose's placement on the bike is a choice, not a fact.** The files say where the rider's
  anchors are and where the chassis's are; nothing on the disc says where a *rest-pose* body sits,
  because the game never draws one — the motions carry the root's position per frame. `bolt_onto` with
  `FOOT_ANCHORS` is the defensible reading (it is the pair the straight bind-pose legs suit, and the
  hand pair demonstrably buries the feet), but it is not the original's placement and was not compared
  against one. V5 replaces the question entirely.
* **Nothing here was read out of the decompilation.** The formats were derived from the bytes: the
  `.mcf` keys, the `SLT`→`.psg` equivalence, the sex prefix, the dummy-node convention. The retail
  loader was not disassembled, so a key this disc never uses could be misread and no test would catch it.
* **The task's premise is wrong and was not followed.** The brief says "the skeleton in `bike.mcf`".
  `bike.mcf` is the *bike's* motion list (six clips) and holds no skeleton; the rider's skeleton is the
  node tree inside `mrider.psg`. Both were opened to confirm it. `bike.mcf` is parsed by the same
  `MotionSet` and asserted on, so the premise is at least recorded in a test.
* **The brief's path is wrong too.** `UI\Rider\*.res` holds the same `mrider.psg` body with a UI motion
  set; the race rider is `Rider\*.res`, which is what this uses. Both were checked; the UI copy's own
  motion set was not otherwise examined.
* **No motions, no skinning, no LOD switching, no inverse kinematics.** The body is drawn at LOD 0 in its
  rest pose. Weighting is one bone per vertex because that is all the format stores; whether the original
  blends between LODs by distance was not investigated.
* **The rider drawn is `thq1`, chosen arbitrarily.** Nothing in a `.scn` names a rider — the choice is a
  menu's, and there is no menu yet.
* **Only the male body exists on this disc, so `Sex::Female` is untested against data.** The code path
  returns an error, which is asserted; what the original does when a female rider is selected is unknown.
* **The three trailing `[MOTION n]` sections of `mrider.mcf` are past its own `NumberOfMotions = 40`**
  and name `Turn_fast_*` clips no archive ships. The parser honours the count and keeps the excess in
  `MotionSet::unlisted`. That the game ignores them is inferred from the count, not observed.
* **`air_Bck_return.MOT` is named by all 324 `.sub`s and shipped by none.** Asserted as the one absence.
  The fallback is assumed to be the sim's documented "a gesture slot whose clip is missing falls back on
  the same fixed pose"; not observed in the original.
* **`VUTfilename` and `RotationType` are parsed and carried, not used.** Nothing here knows what a `.vut`
  is or what the rotation type changes.
* **`crates/openmx/src/main.rs` was not touched.** The playable shell still draws no rider; putting a
  T-posed one in it would be a visible regression, and the merge surface is smaller this way. V5 owns it.
* **No emulator comparison.** The by-eye check is against V1's existing screenshot pair and the geometry
  itself, not against a fresh capture of the original standing a rider on a bike — the original never
  shows this pose, so there is nothing to capture it against.

## Next

V5, the motions: `.vue` clips named by the `[MOTION n]` sections, the root channel they carry, and the
skinning that follows from it. When that lands, `bolt_onto` stops being the placement and becomes at most
a sanity check, and `crates/openmx/src/main.rs` can draw the rider for real.
