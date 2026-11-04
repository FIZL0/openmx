# 1 — untamed bike step (FINAL)

Task MD93g. 2026-11-03.

## What I did

- I read the stunts loader, the "bound" lookups, the combo matcher and its parser, the trick record
  fill, and the stunt start in the Untamed decompile (`context/untamed/decomp/game.c`, copied from
  `openmx-slots/s3`). I read the stunt table and the bike's stunt words off `context/untamed/ram/nat_or.bin`
  with a throwaway Python reader. The addresses and offsets are in research/oracle.txt `# MD93g`.
- **`handling::untamed`**:
  - New `BACKFLIP_STUNT` (49), `THREESIXTY_L_STUNT` (52) and `THREESIXTY_R_STUNT` (53), and
    `spin_stunt(dir)`: 52 for a direction under 1, else 53.
  - `ride_spin` hands in the real stunt id while the 360's clip runs. Before, it was the stand-in's 49.
    Both ids are past 48, so the 360's end clears the same way and nothing on the ride changes.
  - The docs on `ride_spin` and `SpinIn::lip_x` now give the traced reason for each input.
- **`tests/untamed_reworked.rs`**: new test `untamed_ride_spin_input_is_the_shipped_bindings`. On the
  image it checks:
  - the stunt table's backflip/threesixtyL/threesixtyR indices and names;
  - the bike's three stunt words, which equal the new constants;
  - `spin_stunt` for -1, 0 and 1;
  - the game's "bound left" and "bound right" on a crafted pad record. Both answer yes for an idle pad
    and no with R1 held or the stick left, which shows that their combos are empty.

## What the original does

- **The 360 has no bound control.** Its "bound" test reads the same per-style byte as the backflip's.
  The stunts loader sets those bytes only on the row named "backflip". For threesixtyL/R it keeps only
  their indices. So the 360 takes its bound path exactly when the backflip does, and on the shipped
  stunts that is never.
- **Bound left/right**, if they were asked, match the pad record against the stunt's classic combo:
  - the stick's direction (centred, up, down, left or right, past 0.5 on its larger axis);
  - Circle, Triangle and R1 (Select is not compared).
  - The combos of stunts 49, 52 and 53 are all empty, so both would fire on a centred stick with nothing held.
    The bound byte being 0 is what keeps that from happening.
- **The stunt id.** The 360's stunt start passes the bike's threesixtyL word for a direction under 1 and
  the threesixtyR word for 1. A stick left arms direction 1, so it starts "R". A bound control's left is
  direction −1, so it starts "L". The start sets the rider's id to that stunt only when the current id
  is over 48, so a trick that is running keeps its id. That matches the stand-in's order: a held trick
  comes first.
- **The sticks** are as MD93f found. The lip's x and the decay's x are copies of two pad words. The port
  feeds both from the step's stick.

## Checks

- `cargo test -p game-sim --test untamed_reworked --release`: 11 passed.
- `-- --nocapture` on the new test: it ran on the image and printed no skip.
- `./kit audit`: the same 4 failures as HEAD (atv_wheel.rs, plan/DONE.md), none in the files this task touched.

## Not verified / not 1:1

- The port's rider keeps no stunt id, so the id is only read by the 360's own end test, and the clearing
  is not kept (as before).
- Remapped controls (the options screen) and the other styles were not traced. The game only ever fills
  the record in style 3.
- What writes the pad words the two sticks are copied from is still open (R30).
