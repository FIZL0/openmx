# 1 — f3 vu0 (PART)

Task F3. 2026-10-15.

## What I did

`crates/ps2-float/src/vu.rs` already had VU0's *arithmetic* (chop, clamp, flush, the full-width adder,
field masks, `Vf`). What it did not have is the rest of macro mode: the accumulator, `Q`, the three flag
registers, and one method per instruction so that a ported routine can be written line for line against
its listing. That is what this task added, as a new section appended to the file — nothing above it moved.

Added, all in the one new section:

- `Vu0` — the state an instruction sequence carries: `acc`, `q`, `mac`, `status`, `clip`.
- One method per op the game actually uses: `add sub mul madd msub` (+ the `q` forms `addq subq mulq
  maddq msubq`), the accumulator forms `adda suba mula madda msuba`, `opmula`/`opmsub`, `max`/`mini`,
  `abs`/`mov`/`mr32`, `clipw`, and `div`/`sqrt`/`rsqrt` writing `Q`. Broadcasts are the plain op with
  `Vf::bc(Field)` on `ft`, so `vmaddz.x vf2, vf3, vf2z` ports as
  `vu.madd(Dest::X, vf2, vf3, vf2.bc(Field::Z))`.
- `Mac`, `Status`, `Clip` — the three `cfc2` registers, with the hardware's bit layout so a ported `andi`
  mask reads unchanged, plus named accessors (`mac.sign(Field::W)`, `mac.signs()`, `clip.outside()`).
- `max`/`mini`/`abs` as free functions, `Vf::{max, mini, abs, mr32}`, and `max_abs3` for the
  `vabs.xyz` + `vmaxy.x` + `vmaxz.x` idiom the game uses for a bounding test.
- A doc table of the op histogram (counts only, no addresses) so the next reader can see what the list is
  and is not, and a doctest that transcribes the game's normalise instruction for instruction.

Also fixed three real bugs in the kit's own VU0 disassembler, `ps2kit/decomp.py`, because the op list was
derived from its output and two of the three would have put wrong ops in the list (details below).

11 new tests in `crates/ps2-float/tests/vu.rs`. `./kit check`: 72 tests pass; `./kit audit` clean.

## What the original does

**The op list is the game's, not the ISA's.** Every COP2 word in the boot ELF, decoded and counted.

The first cut of that histogram was wrong in a way worth writing down: scanning *all* of the executable
segments finds 9,225 words whose top six bits say COP2, 271 of which do not decode at all — and the ones
that do decode include `vftoi0`, `vitof4`, `vmaddq`, `vmsubq` and a `viadd`. Those are ASCII strings and
constant data at `0x42xxxx`, not instructions (`0x49` is `'I'`, which is why so much of the game's string
table looks like COP2). Restricting the scan to words that lie **inside a decompiled function** gives
**8,406 COP2 instructions, every one of which decodes**, and the five phantom ops disappear. The 819
words that were dropped are all outside any function.

In code, then: 724 of the 8,406 are the `qmtc2`/`qmfc2`/`cfc2`/`ctc2` transfers in and out, and **7,682
are VU0 arithmetic**. Descending, with broadcast forms folded into their base op: `vmadda` 1,441,
`vmul` 1,057, `vopmula`+`vopmsub` 932, `vmadd` 717, `vadd` 695, `vsub` 645, `vadda` 432, `vmula` 316,
`vwaitq` 300, `vmulq` 293, `vrsqrt` 247, `vmax` 238, `vmsuba` 68, `vmove` 63, `vdiv` 48, `vnop` 47,
`vmsub` 42, `vabs` 22, `vclipw` 21, `vmr32` 18, `vsuba` 11, `vmini` 7, `vaddq` 7, `vitof0` 6, `vsqrt` 6,
and one each of `vcallmsr`, `vlqi`, `vsqi`. The full table is in the vu.rs section header.

The three singletons are SDK leaf routines rather than maths, and each is 16–64 bytes:
`./kit dis 0x003f1fe8 0x003f1ff8` is `andi 0xffff` · `ctc2 $vi27` · `vcallmsr $vi27` — "start the
micro-program at this address" — and `./kit dis 0x003f2308 0x003f2348` is one quadword into VU0 data
memory through `$vi1` (`vsqi`), with the matching `vlqi` next door. So micro mode *is* reachable in this
game, by exactly one route, even though all the arithmetic is macro mode.

Two things the list settles:

- **The I register is never used**, and neither is the integer unit. No `vaddi`, `vmuli`, `vmaxi`,
  `vminii`, no `vrnext`/`vrxor`, no `viadd`/`visub` in any function — and no `vftoi` in any form, only
  `vitof0` unpacking small integers. So `Vu0` has no `I` and no `R` by evidence rather than by omission.
- **The MAC flag is control flow, not diagnostics.** Two consumers in the game's own code:
  - `./kit dis 0x003dc320 0x003dc3e8` — a `.w`-masked dot product (`vaddax.w`, `vmadday.w`, `vmaddz.w`),
    four `vnop`s, then `cfc2 $t3, $vi17` and `andi $11, $11, 16`. Bit 4 is `Sw`: "the w lane came out
    negative", i.e. the point is behind the plane. The whole block repeats per point.
  - the same `cfc2` with `andi 0xf0` after a four-lane op, tested for `== 0xf0`: "all four lanes
    negative".

  Both only work if the lanes the destination mask did *not* write have their four flags **cleared**, not
  carried over — which is how `Vu0::write` does it, and what `mac_flags_describe_only_the_lanes_the_mask_wrote`
  pins down.

Recipes for the rest (addresses live here and in `research/oracle.txt`, never in the port):

| what | recipe |
|---|---|
| `vdiv` + `vmulq`, 9 instructions, pure | `./kit dis 0x001088f0 0x00108914` |
| the normalise the doctest transcribes | `./kit dis 0x00103198 0x001031c8` (inside `./kit fn 0x00103020`) |
| `vitof0` + `vmulx` byte unpack | `./kit fn 0x001008e0` |
| quaternion → 3 matrix rows, `vmr32.w` to zero each w | `./kit fn 0x0013ebd0` |
| the MAC `Sw` branch | `./kit fn 0x003dc320` |

The normalise is worth reading in full, because it is the shape of a few hundred routines in this game:

```
vmul.xyz   $vf2, $vf1, $vf1      # x², y², z²
vmaxw.xyzw $vf3, $vf0, $vf0w     # vf3 = 1.0 in every lane (vf0 is (0,0,0,1) — a constant, not a max)
vadday.x   $vf2, $vf2y           # ACC.x = x² + y²
vmaddz.x   $vf2, $vf3, $vf2z     # vf2.x = ACC.x + 1.0*z²   — the dot product, in two chops
vrsqrt     Q, $vf0w, $vf2x       # Q = 1.0 / sqrt(len²)
vwaitq
vmulq.xyz  $vf1, $vf1, Q
```

### Three disassembler bugs, found by using it

The histogram is only as good as the decoder, and three entries came out wrong. Each fix was re-checked
with `./kit dis` against the game's own instructions:

1. The special2 table (funct `0x3c..0x3f`, `ext = (bits10..6 << 2) | (funct & 3)`) was shifted by one from
   `0x2b` on: `vopmula` was printed as `vopmsub`-adjacent nonsense, `vnop` as `vmove`, `vmove` as `vmr32`.
   `0x2b` is unused; `0x2e`/`0x2f`/`0x30`/`0x31` are `vopmula`/`vnop`/`vmove`/`vmr32`.
2. `vdiv`/`vsqrt`/`vrsqrt` printed their two lane suffixes swapped — FSF is bits 22..21 (the `fs` lane),
   FTF is bits 24..23. `vrsqrt Q, $vf0w, $vf2x` was printing as `$vf0x, $vf2w`, which reads as a plausible
   instruction and is the wrong one.
3. `vmini` was classified as an I-register op by an `endswith(('q','i'))` name heuristic, so `vmini` was
   printed as `vminii` (`Q`/`I` membership is now two explicit sets). This is the bug that would have made
   the "the game never touches I" conclusion false.

`tests/test_static.py` still passes (47/47); its `R5900_WORDS` fixture does not cover the changed slots,
and I deliberately did not edit that shared file — F3 is not its owner and another slot may be in it.

## How it was proved

Not against the original. Nothing ran: D1 is blocked on a missing BIOS so the game cannot boot, and the
function oracle (O1) is not built yet, so there is no path to a bit-exact comparison this session. I never
started an emulator copy.

What there is instead:

- **The op list is proved** — it is a decode of the image itself, not a reading of a decompilation, and the
  counts are instructions.
- **The flag layout is proved from the game's consumers**, not from a document: a mask of `0x10` after a
  `.w` op and `0xf0 == 0xf0` after a four-lane op only make sense for one layout and one clearing rule
  (above). PCSX2's `& 0xfcf` on the status register corroborates the sticky-bit half independently.
- **11 unit tests** over the new surface, each written as the behaviour a port depends on rather than as a
  round-trip of the implementation: the two MAC idioms the game uses; masked lanes clearing; zero / `-0` /
  clamp / flush flag combinations (`0x000f`, `0x0011`, `0x8000`, the adder's `0x00c0|0xc000`, `0x0808`);
  status last-op bits moving while the sticky bits do not; the clip register as a shift register, four
  judgements deep, with the game's own four-point AND/OR cull; `vmax`/`vmini` selecting without rounding
  and without touching MAC; the accumulator across a masked transform; the `vopmula`/`vopmsub` pair; a
  masked move giving a matrix row its `w`; `Q` written by the divider and read by the `.q` forms; and the
  normalise doctest asserting the exact bits `[0.59999996, 0.79999995, 0.0, 1.0]`.

## Not verified / not 1:1

- **No oracle and no emulator proof of any value.** Every number in the new code is modelled, not
  observed. O1 (the function oracle) is the proof, and it cannot run until D1's BIOS is in place
  (`d1-the-disc/2-the-bios-READY.md`). The four call sites in `research/oracle.txt` are chosen to be the
  cheapest first proofs: the first two are small and pure.
- **Where the hardware is ambiguous I followed the reference emulator (PCSX2), consistently with what
  vu.rs's header already claims.** Specifically:
  - the `-0.0` vs `+0.0` tie in `vmax`/`vmini` is a float compare (so `max(-0.0, +0.0)` keeps the first),
    not hardware's sign-magnitude integer compare — these can disagree, and the game's uses (clamps
    against literal constants) never hit the tie;
  - a flushed underflow sets **both** `Z` and `U`;
  - `vmax`/`vmini`/`vabs`/`vmove`/`vmr32`/`vftoi`/`vitof` leave the MAC register untouched;
  - the clip register's bit order (x>+w, x<-w, y>+w, …) is the manual's and PCSX2's. The game only ANDs
    and ORs all six bits of each judgement, so its own code cannot distinguish the order and does not
    prove it.
- **`vmadd`'s MAC flags describe only the final add.** A clamp or a flush inside the chopped product is
  not separately flagged. This is the reference behaviour, and no test in the game's own code depends on
  it, but it is a guess about hardware that does not care to tell you.
- **`vrsqrt` is modelled as two chopped operations** (reciprocal of a square root), matching how `rsqrt.s`
  is handled in `fpu`; the divider's internal width is not modelled, so the last bit of a `vrsqrt` is the
  first thing to check against the oracle. `vdiv`/`vsqrt`/`vrsqrt` round to nearest, like the FPU's.
- **"The exact value in `f64` decides the flags" is an argument, not a proof.** Two `f32`s add and multiply
  exactly in `f64`, and near the overflow/underflow boundaries the operands' exponents are close enough
  that the `madd` chain's intermediate is exact too — so the flag thresholds are right for every input the
  game can produce. Written out in the code's comments; not machine-checked.
- **The `ps2kit/decomp.py` fixes are out of F3's file list** (the task said `vu.rs` and its test). I made
  them because the op list is read out of that disassembler and two of the three bugs would have corrupted
  it; each is verified against the game's own instructions, but they are a tool change riding along in this
  branch rather than something F3 was asked for.
- **No pipeline, no timing, no micro mode, no VU1.** `vnop` and `vwaitq` are no-ops here; the game's
  four-`vnop`-before-`cfc2` spacing is a hardware latency this model does not need and does not reproduce.
  The one `vcallmsr` means some micro program does run on VU0, and nothing here covers it: whatever it
  computes has to be ported by hand or hooked out (`ps2-oracle`'s `hooks`). I did not look at what it
  uploads or who calls the uploader — out of F3's scope, and worth a line in whichever task owns VU1.
- **"Inside a function" is Ghidra's idea of a function.** An op used only in code reached by a jump table
  or a function pointer, which R1 says Ghidra may have missed, would be invisible to this histogram. The
  counts are therefore lower bounds, and the "never used" claims hold for everything Ghidra found — 8,558
  functions, 99% of the image. The 819 dropped words were spot-checked, not all read.
- Ops the game does not use are not implemented here: `viadd`/`visub` and the integer file, `vrnext`,
  `vmfir`/`vmtir`, `vlqi`/`vsqi`, `vaddi`/`vmuli`/`vmaxi`/`vminii`. (`vftoi`/`vitof` *are* implemented, on
  `Vf`, from before this task; the game only uses `vitof0`.) If R2/R3 turn up one in a routine that
  matters, it goes in then — the histogram is a statement about this image, and adding to it is cheap.

## Next

1. **O1/O2 should extend `crates/ps2-oracle/src/ee.rs` before anything else uses it.** Its COP2 interpreter
   already numbers special2 correctly (independent corroboration of fix 1 above), but it is missing
   `vmax`, `vmini`, `vabs`, `vclipw`, `vftoi`, `vitof`, and — the one that blocks two of the four oracle
   entries — `cfc2` of `$vi16`/`$vi17`/`$vi18`. Without the flag registers the MAC behaviour this task
   models cannot be compared against the real code at all. I left that file alone deliberately: another
   slot owns it.
2. Then run the four `research/oracle.txt` entries against `Vu0` over generated inputs, in that order, and
   turn this entry into `-FINAL` with the N.
3. For porting work in the meantime: transcribe listings one method per instruction (the `Vu0` doctest is
   the model), and never reach for `fpu` in a routine the listing shows on VU0 — the adder widths differ,
   which is the drift this crate exists to prevent.
