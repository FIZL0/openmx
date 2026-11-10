# 1 — atv in game (PART)

Task MD37d. 2026-11-10.

## What I did

I read what MD37b (`game_sim::quad` and the `atv_*` modules) and MD37c (`machine_packs.rs`, the
`mods/machines/mxvatv_atv` pack) left for the shell, then split the task. **No code changed.**

What is there:

- **The pack loads.** `machine_packs::read` reads the builds, the three chassis and their quad nodes,
  the rider and quad clips, the stunt table, the FC_ cameras, the engine sounds and the emitters. The
  8 `.phy` files that set the Race/Pro × A/P pick are in the pack: `atv_{Race,Pro}250{A,P}.phy` and
  their `atv_s_` twins.
- **The quad's step is ported and bit-exact.** `quad::frame` and `quad::Steps` replay C28's 15 rides
  (MD37b part 4), but always on these terms:
  - **Seeded from an image.** The quad, its body, wheels, engine and parts come from a C28 RAM image
    (`tests/atv_quad/image.rs`'s `read_ride`, which reads a few hundred words). No code in game-sim
    makes a quad from `atv_*.phy` and a build.
  - **Run on SLUS-21104's world.** The ground query is `atv_ground::Terrain` and the sweep is
    `atv_search::Oasis`: Desert Oasis's terrain, meshes and plants. The parts and scene go through
    SLUS-21104's own spatial database (`atv_spatial`, `atv_sync`). None of this reads MX Unleashed's
    `World`.
  - **Not every call is answered.** Some of the `quad::Call`s that `Steps` gets go to `rest`: the
    wreck, stunt start and end, rumble, posts, and the part made/hung/load calls. In the tests, `rest`
    is the interpreter. MD118 is about the record's outside, which still runs in the original.

So a quad picked in the garage would have nothing to make it from and no way to ride MX Unleashed's
tracks. The brief's "an ATV picked in the garage rides" needs both before any menu or garage work
means anything. A menu row or garage entry that rides nothing would be the kind of placeholder
AGENT.md rules out.

The split. `plan add MD37d …` is refused because the id ends in a letter (memory
plan-add-refuses-letter-ending-ids), so the pieces are under MD:

- **MD122**: the quad made from its files. SLUS-21104's quad loader (`atv_quad_init` and the shared
  moto `.phy` load, read in the PC exe first) fills `quad::Quad`/`Params`/`Bike`, `atv_body::Body` and
  the wheels from `atv_*.phy` and the build. It is proved word for word against C28's spawn images, as
  MD39b proved the car loader.
- **MD123**: the quad on MX Unleashed's worlds. `Steps`' ground and sweep are answered from
  `game_sim::World`, and every `rest` call is answered or dropped on purpose and listed. A ride type
  the shell can step each tick. Proved by a headless free09 ride and by MD37b's replays still passing.
- **MD37d** stays the shell part, with its text unchanged: the setting, the garage and pause pick, the
  quad drawn and posed, the cameras, the sound and particles, and the shell test. It is blocked
  `--after MD122,MD123`.

Nothing was taken from opentennis: its game has no machines.

## What the original does

Nothing new was read. The loader names are research/oracle.txt's `# MD37` rows (`atv_quad_init`,
`pc_quad_init`). The car's loader `atv_car_phy_load` and `atv_vehicle_phy_load` are in `# MD39b`. They
show the order a quad's load probably follows (the vehicle base, then the moto, then the quad's own
init), but MD122 has to read that and confirm it.

## How it was proved

Nothing was proved: nothing was built. The gap was found by reading these:

- `quad.rs`'s public items: `frame`, `Steps`, `Moto`, and no constructor;
- `tests/atv_quad/image.rs::read_ride`;
- MD37b part 4's and MD37c's journals.

## Not verified / not 1:1

- **The whole task.** No ATV is in the garage, the machine pick or the pause menu, none rides, and
  nothing of the pack is drawn, posed or heard in a ride. The reason is the two missing pieces above
  (MD122, MD123).
- **"With no ATV picked every replay hash is unchanged."** True trivially, since no shipped code
  changed. It is not a test of anything ATV-specific.
- **The MD122/MD123 split's estimate of what `rest` still gets** comes from reading `quad::Call` and
  MD37b's journal, not from listing the calls a real `Steps` run passes on. MD123 is to list them.

## Next

When MD122 and MD123 have landed and MD37d reopens:

1. Add the setting, off by default and greyed out without the `mxvatv_atv` pack. Model it on MD80e's
   `fliers` row.
2. Add `Kind`/`Machine` entries in vehicle.rs for the quad, with the Race/Pro and A/P pick from the
   pack's `.phy` names. Garage and Switch To Machine both go through `Lot`, as the fliers do.
3. Draw the chassis with `quad::Nodes` posed from the quad's clips, and put the ATV rider clips on
   `mrider`.
4. Add FC_ATV cameras (MD82d is the chase.rs side), the pack's engine sounds and its emitters.
5. Write the shell test: headless free09, a tape that switches into the ATV and rides; with the setting
   off, the plane hash and the replays stay unchanged.
