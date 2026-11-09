# 1 — s13 race activity (FINAL)

Task S13a. 2026-11-08.

## What I did

- `crates/game-sim/src/race.rs`: [`Supercross`] — the supercross/nationals activity class's own state
  (start timeline, lane table, lane per rider, the counters the race manager's tick runs down), its
  construction (`Supercross::new`, the ctor's values), the loader's lane table (`lane_table`), its own
  reset slot (`reset`) and its grid slot (`shuffle_lanes`, `grid_spot` with `GridStart`, `starting_place`).
- `research/oracle.txt`: corrected the activity_supercross block (which slots are its own, field offsets)
  and named activity_supercross_vtable / _reset / _grid, activity_grid_base, race_overlays_bind,
  interface_id.
- `crates/game-sim/tests/race.rs`: proofs 3–5 (below).
- Nothing was taken from opentennis: it has no races.

## What the original does

The class is 0x4e0 bytes with four vtables: `./kit fn 0x2f7da0` is the base ctor, the class's own vtable
is 0x4585d0 at +0x44, and the race-manager subobject's vtable 0x458784 sits at +0x3a0 (its slot 8 is
race_manager_tick). Of the slots the brief named, only three are the class's own: slot 2 (dtor
0x2f9d10), slot 14 (reset, `./kit fn 0x2fb2e0`) and slot 16 (grid, `./kit fn 0x2fa520`). The rest are
inherited from the base vtable 0x458240.

**Loader** (0x2f9f40) fills the lane table at +0x3bc: lane k = k·0.19 − 0.871 (an FPU mul, then a sub).

**Reset** (0x2fb2e0):
1. The base reset (thunk 0x2f8110 → the act's own vtable +0x28, +0x2c), then presentation resets 0x2fe4b0 / 0x2ff270 when +0x3a8 / +0x3ac are set.
2. The race reset 0x2fd570 (vt+0x7c on every line in the two arrays at +0x3f8 and +0x3e0).
3. Cue +0x110 = 0.1, and CLOSED is set on the start gate (+0x3dc)+0x30.
4. The playing start call is stopped. If rand has not been seeded, srand from the RTC.
5. A new call is picked from +0x47c clips: rand index, stepping past the last pick (wrapping to 0).
6. The start delay +0x10c becomes 1.5 + drop[i]; then lights [0, 0.1) and warning [drop − 1.5, +0.1).
7. If the intro is unseen (+0x4c4 == 0), 0x2fd800 runs: it stops clips, makes HUD calls by viewport mode, picks an intro the same way over +0x438 and plays it, then moves the delay and both windows 6 s later and clears +0x4c8.
8. Then sound (app+0x570), HUD overlays by viewport mode, and the overlay objects at +0x46c / +0x44c[+0x470].
9. Finally +0x410 = +0x470 = +0x474 = 0, +0x4cc = 1, +0x4d0 = +0x4d4 = 0.

**Grid** (0x2fa520):
1. lanes[i] = i for i below the rider count.
2. Unless AllAI, `count` passes of neighbour swaps on rand unit < 0.5.
3. For each card with a rider: from the grid gate (+0xb8 → +0x58: centre, normal, width), facing = flat unit normal and lane slot = lanes[id] + (8 − count)/2. The spot is 8 ft behind the centre and out by 0.5·(offset·width). Without a gate: the track's start point, 4 ft per card id.
4. Ground snap, then the entity's place slots.
5. Then the base grid slot and card+0x2c = lanes[id] + 1.

## How it was proved

All in `crates/game-sim/tests/race.rs`, `./kit check -- -p game-sim --test race` (5 pass, none skipped —
checked with `--nocapture`):
- **the_reset_matches_the_activitys_reset_slot**: a function oracle of the reset slot over 10,000 generated activities.
  - Varied: 1–16 clips with stray drop times, last call −1 / count / any, intro count and last intro, intro seen or not, HUD stage, sound on/off, "is playing" answers, gate flags, any generator state.
  - Compared bit for bit on every field the reset writes, plus the generator.
- **the_grid_matches_the_activitys_grid_slot**: a function oracle of the grid slot over 5,000 generated starts.
  - Varied: 1–8 riders, cards in any order, some cards without a rider, a gate (including negative widths) or the track's start point, stray lane tables, any generator.
  - Compared: the lanes, every ground query point, every placement (entity, spot, facing), card places and the generator.
- **the_sx01_image_holds_the_activity_as_ported**: S196's RAM image `context/states/s196-sx01.ee.bin`.
  - The live activity's vtable is the class's.
  - The lane table is bit for bit `lane_table()`.
  - The start timeline is bit for bit what `reset` gives for the image's picks (call 1, intro 1).
  - The start gate is closed.
  - The card's place is `starting_place`.
  - The rider stands within 0.05 ft of `grid_spot` (x exact to print, z ≈ 0.008 off).

## Not verified / not 1:1

- Presentation is not ported. That covers the HUD lookups, overlay and announcer clip play/stop calls, the sound call, the 0x2fe4b0 / 0x2ff270 presentation resets and the grid's scene overlay calls and HUD bind. In the oracles these land on no-op hooks or are skipped by data (viewport mode 3).
- The race reset's line resets (vt+0x7c over +0x3f8 / +0x3e0) are not owned here: the oracle runs them with empty arrays.
- When rand is unseeded the reset seeds it from the RTC. Not ported (it needs a clock); the oracle seeds.
- The grid's vehicle/rider resets (0x27d670, 0x319270, over the list at +0x14) and the interface id lookups are not ported; the oracle runs an empty list.
- The base grid slot (0x2f8120) is hooked out. It is the base class's and not ported here.
- The ground snap is not ported: `grid_spot` returns the point handed to the ground query, before the snap.
- The AllAI grid path (no shuffle, HUD calls instead) is not oracled. The oracle always runs with all_ai = 0; the port skips the shuffle when `all_ai` is set, read from the code only.
- The dtor (slot 2) is not ported (nothing to own in Rust).
- The per-tick side: race_manager_tick's countdowns and the start timeline (unanalysed code at 0x2fb9b0, which runs the lights, call and warning windows) are not ported. The countdowns are S13b; the timeline is filed as S13f.
- The sx01 image has one rider, so it proves only lane 0 on a real grid; the oracle covers 1–8. Its spot check is within 0.05 ft, not bit-exact, because the rider has settled since the grid placed it.
- What weight (+0x128) and the two seat flags (+0x4d0 / +0x4d4) mean is unknown. They are carried with the ctor's / reset's values only.
- The loader's other work (rebuilding the grid gate's normal and loading clips) is not ported. Only its lane table is.

## Next

S13f (filed) ports the start timeline and oracles the AllAI grid path; S13b the race manager tick's countdowns.
