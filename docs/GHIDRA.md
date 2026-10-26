# Decompiling PS2 code

Ghidra does not know the PlayStation 2 out of the box. With the right extension and a correctly laid-out
program it is very good at it. Getting there takes four things: the processor extension, the ELFs at the
right addresses, one slow analysis pass, and the habit of never reading the output whole.

---

## The processor extension

Install `ghidra-emotionengine-reloaded`, matched to **your exact Ghidra version** — the extension API
changes between releases and a mismatched build fails silently, leaving you with a processor list that
does not contain the PS2.

1. Download the build for your Ghidra version.
2. Unzip into `<ghidra>/Ghidra/Extensions/`.
3. Launch Ghidra's GUI once so it registers, then close it.
4. `./kit doctor` confirms the extension is present.

It provides the `r5900:LE:32:default` language. The IOP is a plain `MIPS:LE:32:default` — if you ever
decompile IOP code, import it as a separate program with that language.

Without the extension you get a generic MIPS disassembly in which every R5900-specific instruction
becomes an unknown or, worse, is misdecoded as something plausible. The float ops the generic MIPS
decoder does not have:

| Instruction | What it does |
|---|---|
| `adda.s` / `suba.s` | add/subtract into the accumulator |
| `mula.s` / `madda.s` / `msuba.s` | multiply into / accumulate into the accumulator |
| `madd.s` / `msub.s` | truncated product, then aligned add/subtract with the accumulator |
| `max.s` / `min.s` | single-instruction max/min, no branch |
| `rsqrt.s` | reciprocal square root |

Plus the whole MMI set (the 128-bit integer SIMD instructions), which games use for memory copies and
packing more than for arithmetic.

---

## Getting the programs in at the right addresses

Covered in `docs/PS2.md`: a game is a boot ELF plus overlays, and an overlay read at the wrong address is
unreadable. So before Ghidra:

```sh
./kit elf --scan            # guess each blob's load address from its own references
./kit elf                   # write one ELF per program into the private directory
./kit elf --info <file>     # check the segments look sane
```

`--scan` is a guess. Confirm it: with the right base, string references land on strings, jump tables land
on code, and function pointers point at function prologues. With the wrong base everything is off by a
constant and obviously so once you look at one.

---

## Analysis: once, slowly

```sh
./kit ghidra analyse        # tens of minutes per program. Leave it.
./kit ghidra dump           # seconds. Refreshes the dumps.
./kit ghidra status         # what is imported, and how old each dump is
```

`analyse` imports each program, runs Ghidra's full analysis and saves the project. It is slow, it only
needs doing once per program, and it is the only slow step — because everything afterwards reads the
dumps, and `dump` regenerates them in seconds by running post-scripts against the saved project with
analysis disabled.

That split is worth understanding, because the naive way to use headless Ghidra is to run a script that
re-analyses every time, and that turns a two-second question into a forty-minute one.

What `dump` writes, into the git-ignored private directory, per program:

| Dump | Contains |
|---|---|
| decompilation | every function's decompiled C, with address markers the readers use to seek |
| cross-references | per function: callers, callees, data references |
| disassembly | every function's instructions |
| data | labelled data, strings, and anything that looks like a table |

These are 10–30 MB each. They are an index to be queried, never a file to be opened.

---

## Reading the output without drowning in it

```sh
./kit fn 1000a0                 # one function's decompiled C
./kit fn 1000a0 -p menu         # ... from a particular program
./kit xref 1000a0               # its callers and callees, one line each
./kit xref 1000a0 --similar     # functions shaped like it; 'ported' = you already have Rust for one
./kit dis 1000a0                # its instructions
./kit data --strings --grep walk
./kit data 0x4c1000 --f32 64    # read a table as 64 floats
```

`--similar` deserves a paragraph. It ranks every function by how much its decompiled text resembles the
one you name, using token trigrams, and marks the ones that already have a port. Game code is repetitive
— the same author wrote forty entity update functions the same way — so once you have ported one of a
family, this finds the rest in a second. Over a long project it is the biggest single time-saver in the
static toolkit, and it inverts the usual workflow: instead of hunting for the next thing to port, you ask
what the thing you just ported has siblings.

---

## What Ghidra gets wrong here, and what to do about it

**Float code.** Read the disassembly. The decompiler renders `madd.s` as a multiply plus an add (it is
neither — `docs/PS2.md`), may turn `max.s`/`min.s` into comparisons, and sometimes reassociates
arithmetic into a mathematically equivalent but bitwise different form. For anything whose numbers must
match, the C is a map and the instructions are the territory.

**VU0 macro-mode blocks.** Shown poorly or as intrinsics with no useful body. Disassemble them. Note the
field masks; they matter and the C does not show them.

**Register-argument calling conventions.** The MIPS convention passes the first arguments in registers
and Ghidra usually gets this right, but structs passed by value, varargs and float arguments are often
wrong. If a function's parameters make no sense, fix the signature in Ghidra (or just read the
instructions) rather than believing it.

**Jump tables.** Usually found, occasionally not, in which case a `switch` appears as an unconditional
jump into nothing and the rest of the function vanishes. The missing branches are at consecutive
addresses after the table; disassemble from there.

**Globals the analyser did not label.** Anything unreferenced at analysis time gets no label. The game
reaches a lot of its state through pointers computed at runtime, so expect to identify globals yourself
with `./kit probe` on the running game and then record them in `kit.toml`'s `[symbols]` so every tool and
every note can use the name instead of the number.

**Overlays sharing addresses.** Two programs' functions can live at the same address. Always carry the
program name with the address. The tools default `-p` to the last overlay because that is usually the
gameplay code; when an address gives you something absurd, the first thing to check is that you asked the
right program.

---

## Naming as you go

Rename things in Ghidra as you understand them, and keep the names in `research/oracle.txt` and
`kit.toml`'s `[symbols]` too, so they are usable outside Ghidra and survive a re-import. Re-analysing
loses nothing if the names live in the repo.

Names in the *port* are a separate matter: they describe behaviour, never provenance. No `FUN_…`, no
`DAT_…`, no addresses — see `AGENT.md`. The mapping between the two worlds is `research/oracle.txt`, and
that is the only place it belongs.

---

## Other programs: a PC port, a sequel

A build of the game for another machine reads far better than the PS2's: an x86 exe decompiles cleanly,
and MSVC leaves class names behind. Name it in kit.toml and it gets a project of its own (the PS2 one is
shared by every worktree and must not be locked for an hour):

    [other.pc]
    file = "context/mxvatv-pc/MXvsATV.exe"

    ./kit ghidra analyse -o pc    # import + analyse; NameOther.java names vtable slots and asserting functions
    ./kit ghidra name -o pc       # NameOther again, after adding `name pc 0xaddr` rows to research/oracle.txt
    ./kit ghidra dump -o pc       # decomp/pc.c, pc.funcs.tsv, pc.xrefs.tsv
    ./kit fn MotocrossRider::vf44 -o pc

It is a reference, not an oracle: the port matches the PS2, and a proof runs against the PS2. What the
names mean is in `research/mxvatv-pc.md`.
