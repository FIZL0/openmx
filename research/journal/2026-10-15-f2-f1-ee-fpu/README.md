# F2 — madd.s, msub.s and the accumulator

2026-10-15. Task: `./kit plan F2`.

F1 had `madd.s` arithmetically right (`add(acc, mul(a, b))` — the product chopped to a single *before*
the badly-aligning adder) and unproved. This task proved it: 77,080 input triples × four instructions
against an integer model that shares no code with the one under test, plus counts of how often the two
plausible wrong answers differ (15,108 inputs tell `madd.s` from a true FMA, 4,313 show the product
being rounded before the add rather than at the end), plus exact hand-derived cases — the sharpest being
`(1 + 2^-23)²` against an accumulator of `−(1 + 2^-22)`, where the EE cancels to exactly zero and the
real answer, `2^-46`, is a representable single. It also gave the accumulator an API: `fpu::Acc` is the
`ACC` register, so a `mula.s`/`madda.s`/`madd.s` chain ports instruction for instruction; `suba`/`mula`
(documented by F1 but missing), `fpu::dot`/`F::dot` for the usual compiler-emitted dot product; and one
of F1's assertions, which compared `madd` against `acc.mul_add(a, b)` — i.e. `acc·a + b`, a check that
could not fail — now compares against the right expression. Nothing is verified against the real
console yet; that is O2. Details in `1-f1-ee-fpu-FINAL.md`.
