# 1 — kit dis sqrt s (PART)

Task T5. 2026-11-16.

## What I did

- `ps2kit/decomp.py`: `sqrt.s` (COP1 fmt S, funct 0x04) joins the kit's R5900 table and prints
  `sqrt.s fd, ft`. The R5900 takes the operand from ft and ignores fs; llvm-mc only accepts ft = 0, so every
  `sqrt.s` with a real ft came out as `.word`. Ours now decodes every `sqrt.s`, ft = 0 or not.
- `rsqrt.s` (funct 0x16) was already in the table as `fd, fs, ft` (fd = fs / sqrt(ft)), which is the
  R5900 form; no change needed, one more test word added.
- `tests/test_static.py`: three words added at the end of `R5900_WORDS` (at the end so the `./kit dis
  --len 0x30` test still sees its 12 words): `0x46020084` → `sqrt.s $f2, $f2`, a sqrt.s with fs ≠ 0
  (fs must not show), and `0x46020c56` → `rsqrt.s $f17, $f1, $f2`.
- Nothing from ../opentennis: it has no Python R5900 disassembler table.

## What the original does

Not a port; a disassembler fix. Encoding per the EE core manual: sqrt.s fd, ft — fields rs=0x10, rt=ft,
rd=0, sa=fd, funct 4.

## How it was proved

- `python3 tests/test_static.py`: 47 passed.
- On the real SLUS-21104 ELF: all 12 `sqrt.s` words with ft ≠ 0 in its text now decode, e.g.
  `./kit dis 116a10 --len 0x10 -p atv` shows `mula.s / madd.s / sqrt.s $f2, $f2` (a length: x²+y² then
  sqrt), and `./kit dis 1090ec --len 4 -p atv` shows `sqrt.s $f0, $f4`.

## Not verified / not 1:1

- The real ELF has no `sqrt.s` with fs ≠ 0, so the "fs is ignored" case is only covered by the synthetic
  test word and the manual, not by game code.

## Next

None.
