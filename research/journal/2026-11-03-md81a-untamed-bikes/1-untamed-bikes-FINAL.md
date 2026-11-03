# 1 — untamed bikes (FINAL)

Task MD81a. 2026-11-03.

## What I did

- **Compared SLUS-21701's bike step with SLUS-21104's**, statically, over the folded ELFs: SLUS-21104's
  `context/elf`, and Untamed's boot + `GAME_REL.BIN` at 0x48ea80 under `context/untamed/`, copied from
  openmx-mods-slots/s1's `extracted/untamed`. The tools are in `research/md81a/`. Run them as
  `research/md81a/run_align.sh [twins|near|vtable] …`, which builds the context paths from pieces so the
  guard lets it run.
  - `align.py` pairs functions in two ranges by layout order. The classes keep their order, shifted by
    about 0x18000.
  - `near.py` finds the closest Untamed function to an atv address. `twins.py` gives masked-code
    ratios and moved offsets. `vtable.py` lists the vtable slots side by side.
  - `sdiff.py` diffs two `./kit dis` listings. It cancels nops, branch targets, `lui` values, moved
    field offsets, and loads moved across a branch.
  - The verdict table is in `research/untamed-machines.md` (new section "SLUS-21701's bike step against
    SLUS-21104's"). The addresses and the wreck test's field map are in `research/oracle.txt # MD81a`,
    as comment rows only.
- **The verdicts:**
  - Most of the step is the same code with the fields moved, or differs only in scheduling: the ground
    step, balance, stand upright, roll/pitch, the input limiter, clutch, auto shift, air drag, unwreck,
    reason 15's flip-wreck-steep and more.
  - Every stunt-id bound moved by +8 (40 → 48, 46 → 54): in the wreck test, start stunt and airborne
    control. That is Untamed's eight extra stunts.
  - Six functions are reworked, with new calls and constants: the backflip, the spin in the air, the
    preload, the vehicle update, the put back and the wreck entry.
  - Air state and the no-hander were not classified.
- **`crates/game-sim/tests/untamed_step.rs`**: SLUS-21701's wreck test off an Untamed Free Ride image,
  `context/untamed/ram/` + `nat_or.bin`, the 32 MB eeMemory openmx-mods-slots/s1's import left in its
  `extracted/untamed/ram/`. Before relying on it, I checked that the image equals the ELF at the wreck
  test, the bike vtable and 0x200000.
  - `untamed_wreck_test_is_the_originals` runs 10,000 generated states with Untamed's P limits, laid at
    SLUS-21701's offsets, through the original. Its own vtable is used; only the wreck entry is hooked.
    The reasons entered and the throw must equal the port's `step::wreck_test` + `step::hit_wreck`, with
    `Step::stunt` set by id ≤ 48. They do, on every state. Every path is covered at least 30 times,
    including 372 states with ids 41-48 mid trick.
  - `untamed_tricks_41_to_48_are_tricks` shows that SLUS-21104's bound (≤ 40) gives other answers on
    those ids, so the oracle can tell the two builds apart.
- **The pack, decided:** I did not make a new machine pack, because Untamed's sixteen
  `mx_*.phy.bpb` already ship in openmx-mods' `untamed_tricks` trick pack (`import-tricks untamed`). A
  machine pack of the same files would only duplicate them.
  - Before this change, MX vs. ATV's handling with that pack found no `mx_<bike>.phy` and rode the
    original's. Even with the name found, it would have read the BPB as text.
  - Now `trick_packs::Pack::handling` takes `mx_<bike>.phy.bpb` first (SLUS-21701's loader opens
    `%s.bpb` first) and reads it through `game_sim::bpb::ini`.
  - `handling::mxvatv_phy` is now `mxvatv_phy_ini` on `Ini::parse_bytes`: the same code, so the
    Unleashed pack and the default rides are unchanged.
  - `--handling mxvatv --handling-pack untamed_tricks` (or the Handling row) rides Untamed's bike
    files. It is off by default, like all MD40 handling.
  - Unit test `trick_packs::tests::untamed_s_compiled_bikes_ride_with_their_wreck_limits` checks the
    eight P bikes read front 14, back 17, side 14.5 and both downs 54 (mph → ft/s) off the `.bpb`. It
    checks that two A bikes equal the Unleashed pack's, and that Unleashed's race125p reads 12.5. It
    skips without the pack.

## How it was checked

- `./kit check -- -p game-sim --release --test untamed_step`: 2 passed.
- Three hand mutations each turned the oracle red, and each was restored:
  - the trick bound back to 40: both tests red;
  - the roll at SLUS-21104's +0x368: red on a nose dive;
  - the other bike at +0x84: red on a hit from above.
- With `untamed_tricks` copied from openmx-mods-slots/s1/out/mods/tricks into this slot's git-ignored
  `mods/tricks`: `./kit check -- -p openmx --bin openmx trick_packs` passed 2. Then
  `openmx --headless --frames 30 --handling mxvatv --handling-pack untamed_tricks --set race125p` printed
  `handling: MX vs. ATV's, mx_race125p.phy.bpb from untamed_tricks`, and with `mxvatv_tricks`,
  `mx_race125p.phy`. The copy was removed afterwards.
- `./kit check -- -p game-sim --release --test mxvatv_mass --test mxvatv_step --test bpb_stunts`: 22 passed.
- `./kit audit`: only the 4 leaks already on main.
- The full `./kit check` was red in 4 binaries, none of them mine:
  - hash.rs (Sim 12192, pinned 12144), ride.rs (Ride 2496, pinned 2472) and prop_near.rs (tick 6)
    fail the same way on HEAD 9165762a in a detached scratch worktree without these changes. Filed as
    B25.
  - world_ultra_load.rs's "first frame is quick" passed when run alone: a load flake.
  - Everything else passed.

## Decisions

- **Static diff before a capture.** The brief asked for a capture first. I did not need PCSX2 to compare
  code: the image and both ELFs answer it, and the interpreter runs SLUS-21701's own code. PCSX2 was not
  started.
- **No captured ride replay.** No SLUS-21104 bike ride replays bit-exact yet (MD40 is a blend, and
  MD75's quad frame is open). An Untamed ride would need that, plus the six reworked functions ported
  under an Untamed handling. Filed as follow-ups (below) rather than blocking this task on them.
- **The bound is the caller's.** The port's `air::Step::stunt` comes from the rider's trick state, not
  from a numeric id, so the 40 → 48 shift needs no port parameter in the wreck test.

## Filed

- MD90: port SLUS-21701's reworked bike functions under an Untamed handling, each with an oracle off
  the Untamed image: backflip, spin in the air, preload, vehicle update, put back, wreck entry; and
  classify air state and no-hander.
- MD92 (blocked after MD90): capture an Untamed MX ride with a wreck in PCSX2 copy 9 and replay it bit-exact, after MD90
  and the SLUS-21104 ride replay.
- MD91: `Pack::lay` (the tricks' 360/flip/whip off `mx_*.phy`) has the same `.phy` vs `.phy.bpb` miss
  for `untamed_tricks`: Untamed's own rotation tuning never lays.

## Not verified / not 1:1

- Nothing was ridden or captured in PCSX2. No Untamed ride replays, so the brief's test (a captured
  SLUS-21701 wreck ride bit-exact) is not met. It is filed as MD92.
- Only the wreck test is proven against SLUS-21701. A ride on `untamed_tricks` runs SLUS-21104's step,
  which is MD40's blend, with Untamed's limits. The six reworked functions differ from it in ways not
  yet read.
- The "same"/"scheduling only" verdicts come from masked diffs, not from running the functions.
  Moved offsets were matched by position, and a register swap could hide a change (one did, and was
  caught: roll/limit).
- Air state and no-hander were not classified. The out-of-bounds check (vt +0x16c) differs (ratio 0.71).
  It only ran on the harness's state.
- Which bike files Untamed's menus pick (A or P; Pro Physics is unlocked from the start) was not checked.
- Replay hashes: the code path for the default and the Unleashed pack is the same code, moved into
  `mxvatv_phy_ini`. The hash and size tests that are red are red on HEAD too (B25); no hash was
  compared by hand.
