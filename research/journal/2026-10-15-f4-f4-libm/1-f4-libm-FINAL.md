# 1 — f4 libm (FINAL)

Task F4. 2026-10-15.

## What I did

Found out which transcendentals the disc actually links, ported the ones it does, and proved all six of
them bit-exact against the game's own code over every exponent an `f32` has.

The inventory came first, because the brief's list was a guess and it was wrong. Fingerprinting the
constants in the decompilation and counting callers into the libm range (0x404000–0x413000) in
`boot.xrefs.tsv` gives, for floats:

| the disc links | the brief expected | not in the image at all |
| --- | --- | --- |
| `sinf` `cosf` `atanf` `powf` `fmodf` | `sinf` `cosf` `sqrtf` `powf` | `atan2f` `expf` `logf` `tanf` `asinf` `acosf` |

`sqrtf` is in the image but it is not a library function: it is `sqrt.s $f0,$f12; jr $ra`, and the other
305 square roots in the image are that same instruction inlined. `atan2f` is not there — the one `atanf`
call site does its own quadrant work. There is no float `expf`/`logf`: the only exponential in the image
is inside `powf`, and the only logarithm is the *double* `log`.

There is also a whole double-precision family (`sin` `cos` `tan` `acos` `log` `sqrt` `ldexp`
`rem_pio2`), which the game reaches through C's implicit promotion and which runs on soft float.
Addresses and call counts are in `research/oracle.txt`; porting it is not F4's and I did not.

Then the ports, into `crates/ps2-float/src/libm.rs`: `fmodf` and `kernel_rem_pio2f` (with `floorf`,
`scalbnf` and `copysignf`, which only it needs) are new; `sinf`, `cosf`, `atanf`, `powf` and `sqrtf`
already existed in the crate as generic fdlibm and needed fixing, each fix found by the oracle.

## What the original does

The float set is fdlibm compiled with **every decimal constant rounded toward zero** — `S1` is
`0xbe2aaaaa` where fdlibm's source rounds to `0xbe2aaaab`, `lg2` is `0x3f317217` not `0x3f317218`, `1e30`
is `0x7149f2c9` not `0x7149f2ca`, and so on through every table. I read each one out of the
instructions (`./kit dis <addr>`, the `lui`/`ori` pairs) rather than trusting the decompiler's decimals,
after confirming the decompiler prints the shortest round-tripping decimal, which makes `0.6931471`
and `0.6931472` distinguishable.

Beyond the constants, the SDK patched fdlibm in ways that change results well inside the ordinary range,
and all of them are **a denormal is zero**:

* `powf` returns 1.0 when `|y| < 2^-126` (fdlibm: `y == 0`), and takes its `x = ±0` path when
  `|x| < 2^-126`. `fmodf` treats a denormal divisor as a zero divisor, i.e. `(x·y)/(x·y)`, which on the
  EE is `0/0 = +max` and not a NaN.
* `powf`'s underflow limit is `z <= -126`, not fdlibm's `-150`: there are no denormal results to
  produce. Its overflow limit is `z > 129`, where fdlibm's source says 128, and the last exponent is
  left to saturate on its own through `scalbnf`.
* `scalbnf` has no gradual-underflow path at all: below the smallest normal it returns
  `copysign(tiny,x)·tiny`, which is `±0`.
* In the `|y| > 2^27` shortcut `powf` returns the word `0x7f7fffff` and `0.0` outright where fdlibm
  returns `s·huge·huge` — it is reachable only with an even integer `y`, so the sign is always +1.

Two more things that are fdlibm's own and that a port written from the paper gets wrong:

* `t_h` in `powf` is built from the exponent field as `((ix>>1)|0x20000000) + 0x0040000 + (k<<21)` and
  is **not** masked to 12 bits. The float flavour nudges by `0x0004_0000`; the scaling of the double one
  would say `0x0040_0000`. The port had the double's constant and the double's mask, and that was one
  ulp in `powf` for about one input in a thousand.
* `sinf`, `cosf` and `atanf` do not touch their argument with the FPU before branching on it, so a
  denormal argument is returned *unchanged* rather than flushed to zero, and an Inf bit pattern is
  reduced as the number `2^128` rather than saturated to max. (There is no Inf/NaN early-out in any of
  them: the SDK's libm was built for a machine with neither.)

The big argument reduction is `__kernel_rem_pio2f` with `prec = 2`: `|x|` split into three exact 8-bit
digits, multiplied digit by digit by 198 eight-bit digits of `2/π`, the integer part giving the
quadrant, the fraction multiplied back up by eleven pieces of `π/2`. It is only good to about 1.9e-7
absolute — near a multiple of π/2 the result has cancelled away almost entirely — and since this module
exists to be wrong in the game's way, that is now what `crates/ps2-float/tests/libm.rs` asserts above
201 radians: an absolute bound, not a relative one.

## How it was proved

Function oracle, `crates/ps2-oracle/tests/libm.rs` (new file), 52 228 inputs, every one bit-exact:

```
sqrtf: 5112 inputs   sinf: 10212   cosf: 10212   atanf: 10212   fmodf: 6240   powf: 6240 + 4000
```

Each sweep walks exponent fields 0..=254 with random mantissas and both signs, plus a fixed edge set
(±0, ±1, smallest normal, smallest denormal, largest denormal, ±max, the Inf and NaN patterns), so the
tiny-argument early-outs, the ordinary path, the big reduction, the over/underflow limits and the
denormal-is-zero tests are all covered by construction rather than by luck. `powf` gets both arguments
swept over the whole exponent range with a positive base, plus 4000 whole-number exponents where the
base may be negative (the only way to reach the odd/even-`y` sign logic).

Five disagreements, each a real difference:

| the oracle said | it was |
| --- | --- |
| `sqrtf(-0)`: game `+0`, port `-0` | `sqrt.s` takes `|x|` first; the port's own doc said so and its test asserted the opposite |
| `sinf(1e-45)`: game returns the denormal | the port filtered the argument through `fpu::operand` first; so did `cosf`, `atanf` and `powf` |
| `sinf(20284.223)`: one ulp | the port had its own exact fixed-point reduction instead of `__kernel_rem_pio2f` |
| `powf(x, denormal)`: game 1.0 | the zero tests are `< 0x00800000`, not `== 0` |
| `powf(6.66e-6, 1.296)`: one ulp | `t_h`'s nudge and mask, above |

Host-side, `crates/ps2-float/tests/libm.rs` keeps the accuracy and digest checks (the digest moved,
because most of the constants did) and gains `fmod_is_exact`, 100k inputs against `f64`'s remainder plus
the sign and zero-divisor rules. `./kit check`: 130 tests, green.

## Not verified / not 1:1

* **The double-precision family is not ported.** `sin` `cos` `tan` `acos` `log` `sqrt` `ldexp`
  `rem_pio2`, 50-odd call sites between them, all of them reached by implicit promotion of float data.
  They need an `f64` fdlibm port, which is its own task; the addresses are in `research/oracle.txt`.
  The EE has no double FPU, so they run on correctly-rounded soft float, which means a straight `f64`
  port of fdlibm should match them exactly — but nobody has checked that, and `ldexp` is the single most
  called libm function in the image (27 sites).
* **`tanf`, `asinf`, `acosf`, `atan2f`, `expf` and `logf` in `libm.rs` are unproved.** This disc links
  none of them, so the oracle has nothing to compare against. I left them in place (another game on the
  same SDK will want them) and rounded their shared constants — the `exp` polynomial `P1..P5`, `lg2`,
  `huge`, `tiny` — to the words `powf` proved, since the same compiler produced them. Their own
  constants are still fdlibm's nearest-rounded ones and are probably one ulp high, each.
* **`sqrt.s`'s `-0` is modelled, not measured.** The oracle's own interpreter implements `sqrt.s` with
  `fpu::sqrt`, so proving a one-instruction `sqrtf` against it is circular: what the sweep shows is that
  `libm::sqrtf` agrees with `fpu::sqrt`, not that either agrees with the hardware. Everything else about
  `sqrt.s` (round to nearest, `|x|` first) came from the manual and from 305 inlined call sites behaving,
  but `sqrtf(-0.0) == +0.0` rests on the model alone. A PINE recording of one `sqrt.s` on `-0` would
  settle it.
* **The oracle test is at `crates/ps2-oracle/tests/libm.rs`, not the brief's
  `crates/ps2-float/tests/libm.rs`.** It cannot be at the brief's path: `ps2-oracle` depends on
  `ps2-float`, so `ps2-float` cannot depend back on it to get an `Ee`. The brief's path keeps the
  host-independent accuracy and digest tests, which is what it is good for.
* **`floorf` and `scalbnf` are proved only through their callers.** Neither has a symbol of its own in
  the sweep; they are exercised by `powf` (which calls `scalbnf`) and by every `sinf`/`cosf` above 201
  radians (which calls `floorf`), and they are bit-exact there. `floorf`'s `x + 1e30 > 0` branch, which
  only a negative `x` near `-1e30` can take, is reachable from neither caller and is untested.
* **`copysignf` is ported but unreachable.** Only `scalbnf`'s saturating path uses it, and that path
  returns `±0` or `±max` either way.

## Next

Nothing is pending for F4. The obvious follow-up is the double family above: `./kit plan add` a task for
an `f64` fdlibm port proved the same way. The sweep in `crates/ps2-oracle/tests/libm.rs` is the shape to
copy, but it will need a double-argument `ee.call` — I did not look at whether the interpreter's calling
convention already handles one — and `ldexp` and `log` have enough call sites to be worth it.
