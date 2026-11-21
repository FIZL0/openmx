# 1 — ini float reader matches (FINAL)

Task R65. 2026-11-21.

## What I did

- B47 saw free07's `[Machine1]` `Direction` y `-0.03599517` come out as `0xbd136fab` in the original
  (car +0x280) and `0xbd136fac` "in Rust", and filed this to check the port's ini reader.
- The port's reader (`game_sim::phy::Ini::vector` → `phy::number`) already gives `0xbd136fab`. The
  `0xbd136fac` came from the B47 test itself: `car_spawn.rs`'s `BAJA_SPOT` writes the nose as `f32`
  literals, and Rust rounds a literal to nearest. The test's 1e-6 tolerance hid the bit.
- The shell parks the car from the file through the same reader (`vehicle::in_play` → `Spawn::section`
  → `Ini::vector("Direction")`), so play already gets the original's bits. No product change.
- Filed S497 (low): build `BAJA_SPOT`'s nose with `phy::number` and assert its bits.

## What the original does

- `research/oracle.txt` `ini_float` (`./kit fn 0x0014b520`): `strtod` on the text, then the C library's
  double-to-float routine, which truncates toward zero. `phy::number` ports exactly that (`strtod`-style
  decimal → `fpu::chop`).
- `-0.03599517` lies between `0xbd136fab` (−0.0359951667) and `0xbd136fac` (−0.0359951705). Nearest is
  `…fac`; toward zero is `…fab`. The original's word is the truncated one, as the oracle notes say.

## How it was proved

- A scratch program (not committed) beside the worktree's `game-sim`: `Ini::parse` of the line
  `Direction = 0.9989964, -0.03599517, 0.02666235`, then `vector("Direction")`, against the three words at
  car `0x00605270` + 0x280 in `context/states/r52-free07-baja.ee.bin`:
  - port `Ini`: `3f7fbe3a bd136fab 3cda6b00`
  - original:   `3f7fbe3a bd136fab 3cda6b00`
  - Rust literal `-0.03599517f32`: `bd136fac`
- All three components equal bit for bit.

## Not verified / not 1:1

- Only this one line was checked against the original. The reader's truncation as a whole was already
  proved by earlier work (oracle notes on `ini_float` / `double_to_float`); I did not rerun an oracle
  over generated inputs here.
- The B47 test still compares the nose within 1e-6 against its literal; tightening it is S497, not done
  here (research task: no code change).

## Next

- S497 tightens the test. Nothing else to do for R65.
