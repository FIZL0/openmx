# 1 — atv tire phy (FINAL)

Task MD64b. 2026-11-02.

## What I did

- Read the PC's `pc_quad_tire_phy` first (`./kit fn 0x005d91e0 -o pc`), then SLUS-21104's
  `atv_quad_tire_phy` as asm (`./kit dis 0x004bb880 --program atv`, to 0x004bc9e0). Ghidra's PS2
  decompile hides the five stacked arguments of the side setter and reorders the per-wheel calls. The
  asm is the reference. The PC has the same order and the same quirks.
- `crates/game-sim/src/phy.rs`: the per-surface block of `Tire::read` is now `phy::Surface::read(s, i,
  front)`, so the quad reader reads its `%d` keys the same way. `Tire::read` calls it and is otherwise
  unchanged. Its tests (`--test phy`) still pass.
- `crates/game-sim/src/quad.rs`:
  - `tire_phy(t: &mut TireSetup, ini, mass) -> [Option<[Vf; 2]>; 4]`. It returns the segment each
    wheel's sweep shape is rebuilt to, for wheels that have one.
  - Private setters for the suspension, tire, steering and derived steps. The tire and suspension math
    reuses `phy::spring_rate`, `phy::damping`, `tire::tire_mass`, `tire::spin_inertia` and
    `tire::tire_ply_scale`. They are the same as MX Unleashed's wheel setters.
  - New types: `TireSetup` (the four wheels, the two pairs' stopped/moving springs, the mud),
    `WheelTire` and `TireSurface`.
  - Anything the reader does not reach keeps its value in the struct, as the RAM keeps it.
- `crates/game-sim/tests/atv_quad.rs`: the two tests. Their hooks and helpers have `tire_` names and
  are separate from MD64's harness.
- research/oracle.txt: an `# MD64b` block (field offsets in comments only), plus names for the seven
  ATV wheel setters, the sweep shape rebuild, the ini getters and the C library's snprintf, strtod and
  strtok.
- The eight quad `.phy` files are in `context/mxvatv-ps2/md37a/phy/` (git-ignored, s3). Recipe: from
  `MX vs. ATV Unleashed (USA).iso`, take `MXVSATV.PSR` → `vehicletype_1.res` (`game_sim::archive::Psr`,
  `Res`), then `Veh\atv_{race,pro}250{a,p}.phy` and `Veh\atv_s_{race,pro}250{a,p}.phy`. Names are
  written lowercased.

## What the original does

a0 is the quad, a1 the ini. The wheels are at quad +0x17b0 (LF), +0x17b4 (RF), +0x17b8 (LR) and +0x17bc
(RR).

**Front.**
1. Selects `[Front Tire]`.
2. Writes `UseParametricCurve` straight onto LF +0x34. This is the flag read: the int read with a
   default of 0 written when the key is missing.
3. Reads into locals: StrutCompression (default 1), MaxCompressionRate (20), MaxDecompressionRate (1),
   TireRadius (1), TireInertia (1), Spring (0) and the two damping ratios (0).
4. Writes `SpringWhenStopped` raw into quad +0x137c.
5. Reads the three steering keys (0) into locals.
6. Writes `MaxSteerAngle` raw onto LF +0x70 and `BodyTireForcePosition` onto LF +0xa0 (default
   (0,0,0,1)).
7. Calls the suspension setter on LF with the stopped spring and copies LF +0x48 back into +0x137c.
   Calls it again with the moving spring and copies into +0x1380. So the damping rates come from the
   moving spring.
8. Then the tire setter (r, I, 6, 24), the steering setter and the derived setter.
9. +0x70 *= 0.01745329, and +0x74 = −that.
10. `NumberOfSurfaces` is read into a stack word. For each surface i (from 1):
    - 25 keys (`%d` by snprintf);
    - `MudTime%d` → quad +0x1654 + 4(i−1) and `MudAmount%d` → +0x1694 + 4(i−1), with no bound;
    - drive, side (donut scale 0) and drag setters on LF, then on RF.
11. RF copies LF's +0x34, +0x70, +0xa0..+0xac and +0x74, then gets the suspension (moving), tire,
    steering and derived setters.

**Rear.** Selects `[Rear Tire]` and does the same, with these differences:
- No steering keys and no `MaxSteerAngle`.
- `OtherTireNormalLoadScale` (default 1) goes onto LR +0xb8.
- The springs go to +0x1384/+0x1388. The suspension setter runs three times (stopped, moving, moving).
- **No steering setter on LR.**
- `NumberOfSurfaces` goes into **the same stack word**, so a rear section without it uses the front's
  count.
- 24 keys per surface: `DonutsSideForceSpeedScale%d` replaces the mud pair.
- RR copies LR's +0x34, +0x70, +0xa0..+0xac, +0x74 and +0xb8. Its +0x70/+0x74 are LR's constructor
  values (±0.7854 in the images).
- RR gets the suspension, tire and **steering from the front's locals**, then the derived setter.

**Derived setter.** If wheel +0x1dc (the sweep shape) is set, it calls the rebuild with (shape, 1,
[(0, strut, 0, 1), (0, −radius, 0, 1)], 0, 0). Then +0x100 = mount (+0x90), and +0x104 −= radius.

The RAM images back the steering quirk: LR +0x64/+0x68/+0x6c/+0x8b4 are 0, and RR's are the front's
(20, 0.8, 1.25, 200).

## How it was checked

- `the_quads_tire_reader_is_the_originals`: 10,000 generated files.
  - Every key the reader asks for is present 4 times in 5.
  - Values as files write them (whole, 1–7 places, full precision).
  - The radius, tire inertia and steering inertia are sometimes ≤ 0.
  - The flag is any of 1, 0, 2, −3, 0x10, True, false, Yes, n, On, OFF, T, F.
  - Front counts are 0, 1–3, 2–15, 16 or −2. The rear's count is missing 1 time in 4.
  - Vectors have 3 or 4 components. 1 file in 50 has no front keys but the count, and 1 in 50 has no
    `[Rear Tire]`.

  The quad and the wheels are filled with finite noise. The wheel pointers, a mass object, random sweep
  shapes (half null) and the mounts are laid on top. The original runs with the ini's section select
  and string lookup hooked, along with the syscall stubs and the sweep rebuild (its calls recorded).
  Everything else is the original's own code: the int/flag/float/vector getters, strtod, strtok,
  snprintf and the seven setters.

  The hook hands over each value as the game's line reader keeps it, with the whitespace squeezed out,
  via `phy::Ini`. Then the port runs on what the RAM held and writes back. Every word of the quad
  (0x17c0) and the four wheels (0x8b8 each) must be equal, and so must the sweep rebuild calls.

  The original may write nothing else but its stack, strtok's cursor (reent +0x5c) and strtod's Bigint
  pool (the freelist at reent +0x4c, which the image already allocated).

  19 paths are tallied, each seen at least 50 times. `./kit check -- -p game-sim --test atv_quad --test
  phy --test mxvatv_mass`: 21 passed. The tire test takes about 160 s in release.
- Mutations, each red:
  - no steering on RR;
  - the rear count not falling back to the front's;
  - the damping from the stopped spring;
  - RF not copying the lock;
  - steering inertia at ≥ 0;
  - RR not copying the load scale.
- `the_quad_images_wheels_are_their_phy_files`: in each of MD37a's six images (atv-r and atv-s; spawn,
  air, spawn-late), the image's own reader-written words are compared with `tire_phy`'s answer. The
  answer is computed from a poisoned start: every scalar the reader writes is set to NaN first, so a
  missed write cannot pass on the image's value.
  - The words compared: all four wheels' scalars, vectors and 24×16 tables, the quad's four spring words
    and the mud.
  - Left out: +0x48 (the ground head swaps the spring) and +0x100 (the rest point).
  - Each image must match exactly one of the four candidate files. All six match `race250p`: atv_ for
    the r images (mass 13.043477), atv_s_ for the s ones (13.975154).
  - The three RR/RF/load-scale mutations red this test too.
- `cargo check --workspace --tests`: no new warnings. Clippy can't lint game-sim (memory).
- `./kit audit`: only the existing plan/DONE.md:519 and :533 leaks.

## Not verified / not 1:1

- **A count past 16.** The setters drop the index, but the original's mud writes run on: MudTime17 lands
  on MudAmount1's word and further ones past the table into the quad. The port clamps the loop at 16 and
  writes no mud past it. Never generated.
- **A missing front `NumberOfSurfaces`.** The original reads an uninitialised stack word. The port reads
  0, and the test zeroes the stack so the two agree. No shipped file lacks it.
- **"ON…" words.** The original's int read matches `ON` as a two-letter prefix; `phy::Section::int`
  matches the whole word. Words like "Only" were not generated. `On` and `OFF` themselves agree.
- **NaN radius or inertia.** `tire::spin_inertia` tests `r <= 0`, the original `0 < r`, so a NaN radius
  differs. strtod can parse "nan"; no file has it, and none was generated.
- **One mass for all four wheels.** The original reads each wheel's own body (+0x1e4 → +0x18). The
  port takes one mass, and the oracle points all four at one object. In all six images the four
  wheels' +0x1e4 is the quad's own rigid body (quad +0xb8).
- **The sweep shape rebuild** is not ported: the port returns the two ends, as the hook recorded them.
- **Space-separated vectors.** The game's line reader squeezes whitespace, so "1 2 3" reads as one
  number. The oracle hands over squeezed text and generates commas only. That `phy::Ini::parse`
  squeezes as the original's reader does is earlier work, not re-proved here.
- **Missing sections.** The hook's section select always says yes. What the original does when the ini
  has no such section (whether the lookups then fail) was not run. A missing section is modelled as
  every key missing.
- **Which file a real game picks.** The images say `race250p`. What chooses P over A (difficulty,
  setting) was not traced.
- **Not wired in.** Nothing calls `tire_phy` yet: there is no ridden quad in openmx.

## Next

Nothing in MD64b's scope is left. Wiring the reader into a ridden quad belongs with MD37b's Moto and
MD65.
