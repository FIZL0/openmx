# Proving a port matches

A re-implementation that "looks right" is worth very little. The difference between this kit and reading
a decompiler's output into an editor is that everything here exists to answer one question cheaply:

> does my code do what the original did, for inputs I did not think of?

> **Want to see one all the way through first?** [docs/EXAMPLE.md](EXAMPLE.md) does the oracle method end
> to end — find the function, read it, port it, prove it bit-exact over ten thousand inputs, then break
> the port on purpose and watch the proof catch it two bits in. It runs against a synthetic program the
> kit generates (`./kit demo`), so it needs no disc and no emulator.

There are four ways. They cost very different amounts, and picking the wrong one is the most common way
to waste a day.

| Method | Use it for | Cost per answer | Catches |
|---|---|---|---|
| **Function oracle** | a pure function of its args and RAM | seconds, in a unit test | everything, including inputs you would never have recorded |
| **Live capture** | timing, frame flow, state machines, anything with history | minutes, needs the emulator | divergence in sequence and timing |
| **Save-state RAM** | tables, constants, struct layouts | one extraction, then free | wrong data, wrong layout |
| **Replay harness** | the whole simulation | build once, then seconds | any regression, anywhere |

Rule of thumb: **try the oracle first.** Reach for a recording only when the thing you are checking
genuinely depends on time or on state you cannot construct.

---

## 1. The function oracle

`crates/ps2-oracle` is a small R5900 interpreter. It loads the game's own code — from *your* disc, at
test time, never committed — puts your arguments in the right registers, runs the function, and gives you
back what it returned and what it wrote to memory.

```rust
let mut ee = Ee::game()?;                  // code from the configured disc
for _ in 0..20_000 {
    let (speed, stick) = (rng.f32_in(0.0..12.0), rng.f32_in(-1.0..1.0));
    let want = ee.call(addr("walk_ramp"), &[], &[speed, stick], 100_000)?.f0();
    let got  = my_port::walk_ramp(F::new(speed), F::new(stick)).bits();
    assert_eq!(want.to_bits(), got, "speed={speed} stick={stick}");
}
```

Why it is the best tool here:

- **No emulator, no recording, no human.** It runs in `./kit check` with everything else.
- **Bit-exact by construction.** You are comparing against the actual instructions, so there is no
  tolerance to argue about.
- **It explores.** Ten thousand generated inputs find the branch that only triggers at exactly zero, the
  denormal that flushes, the sign of negative zero. A recording of real play visits a thin slice of the
  input space, and it is never the interesting slice.
- **Failures are actionable.** You get the inputs that diverged.

What it cannot do: anything that depends on the DMA, VU1, the GS, the IOP, interrupts, or time.
Those need a recording.

```sh
./kit oracle list                             # the names in research/oracle.txt, with addresses
./kit oracle add walk_ramp 1000a0             # give an address a name, once you know what it does
./kit oracle walk_ramp --floats 6.5,-1.0      # one call by hand, while you work out what it does
./kit check -p ps2-oracle --test game         # the ten-thousand-input comparison, as a test
```

`./kit oracle <name>` is for *looking*: a few calls by hand while you read the decompilation. The proof
is a Rust test that makes thousands of calls and compares every one against your port
(`crates/ps2-oracle/tests/game.rs` is the worked shape). `research/oracle.txt` maps names to addresses,
so no address appears in Rust. It is committed; it
contains names and offsets, which are research notes, not game data.

**When the interpreter hits an instruction it does not implement** it panics naming the instruction and
the address. Add it to the interpreter — most are a few lines, and each one you add is permanent. This is
the normal way the oracle grows and it is not a blocker.

---

## 2. A live capture

For anything stateful: the order things happen in, how many frames a state lasts, when a transition
fires, what the RNG does across a sequence.

```sh
./kit record watch/player.toml --state 5 --frames 600 --out context/t12/run1.csv
```

A **watch spec** is a small TOML file naming the addresses to sample each frame, their types, and any
pointer chains or arrays to follow (`watch/README.md` has the format). The recorder coalesces them into
the smallest set of contiguous reads and fetches them in one batched request per frame, which is what
makes a 600-frame capture of fifty fields take ten seconds rather than ten minutes.

Then your port replays the same input from the same seed and the test compares the two traces.

The things that go wrong, every time, in every project:

- **A state that needs a human playing is not reproducible.** Hunt for a self-running state — a demo,
  attract mode, a CPU-vs-CPU match. Keep it in slot 5. It is the most valuable save state you own.
- **A skipped frame silently corrupts a capture.** The recorder detects gaps in the frame counter and
  fails the whole run rather than writing a plausible-looking CSV with a hole in it. If you see
  `CAPTURE FAILED`, the file is rubbish — fix the cause and record again.
- **One PINE connection at a time.** Opening a second client while a recorder is polling hangs both. If
  you need a static value during a capture, read it out of a save state instead.
- **Record more than you think you need.** Re-recording costs minutes and needs the emulator; an extra
  column costs nothing. Capture the neighbouring fields and the frame counter always.

---

## 3. A save state

A PCSX2 save state is an archive containing the whole EE RAM. For anything static — a table of
constants, a character record, a struct you are reverse-engineering the layout of — extracting it from a
state beats reading it live:

```sh
./kit state extract 4 --addr @shot_table --len 0x800 --as f32   # one region, formatted
./kit state ram 4 --out context/ram4.bin                       # the whole 32 MB image
./kit state list 4                                             # what the state file contains
```

No lock, no emulator running, repeatable forever, and safe to do while a capture is in flight. When a
tool needs one number from RAM in the middle of a recording, this is the answer.

The states themselves are yours and are never committed. `kit.toml`'s `[states]` records what each slot
holds so tools and agents know which to load; slots 8 and 9 are scratch and the kit's tools overwrite
them.

---

## 4. The replay harness

The acceptance test for the whole simulation: a recorded sequence of inputs plus a seed goes in, and a
hash of the simulation state per tick comes out. Same input, same hashes, forever.

```sh
./kit check --test replay                   # whatever your test command calls them
```

Build it early, on a simulation that barely does anything (`docs/WORKFLOW.md` explains why). From then
on, every port either keeps the fixtures passing or does not, and you find out in seconds instead of
noticing in a month that something drifted.

Two properties worth paying for:

- **The hash must be platform-independent.** Same hashes on x86-64, ARM and WebAssembly. That means no
  host transcendentals anywhere in the simulation — use the game's own `sinf`/`cosf`/`atan2f`
  implementations, or a fixed library you ship, never the platform's libm, which differs by a bit or two
  between machines. There is a test for this; keep it.
- **A fixture is cheap to add.** Every bug you find becomes one. A replay fixture that reproduces a
  divergence is the best possible regression test, because it tests the real thing rather than your
  theory of the thing.

### The human reference: the 2p ride

The user's two-player recording is the human recording of record (C10, A6): **a new test of wrecks,
collisions, the ragdoll, rider-vs-rider, moving platforms, wreck thresholds or the animations reads
it.** `game_sim::replay::two_p` is the one place that names it — the tape, the capture of record (both
riders' lanes, `p1_`/`p2_`), the tape's start image and the per-event RAM images — and it has the
readers: `two_p::read` streams the ~215 MB capture keeping only the columns asked for, `two_p::lane`
turns either rider's lane into a one-rider `Capture` every older reader understands, `two_p::tape` is
both pads with the engine's own `dt`, `two_p::tape_of` / `PadTape::alone` one rider's pad on port 0, and
`P2m2::tape_per_field` reads the `.p2m2` a field a tick. Two facts first: this state **ticks at 60 Hz,
one tape field a tick** (not the one-player tape's one field in two), and a capture row's polled pad is
the field `two_p::POLLED_FIELD_LAG` behind it. `tests/replay.rs`'s
`the_2p_recording_drives_both_riders_from_its_state` seats both riders from the start image on the
disc's Race 500 over free09 and rides the whole tape; start a new test from it. The event index (every
wreck, near miss, contact, ragdoll and animation stretch at its tick) is C10's journal and
`research/c10/events.json`; read that, not the rows.

When an older test of one of those subjects reads first_recording, recording2, animandpump or the S93
split tapes, the task in its area adds the 2p case beside it. Old bit-exact pins stay; drop an older
fixture only where the 2p case covers the same thing, and say so in the journal. The ride holds no
rider-vs-rider contact, truck ride or prop contact (C10): those cases wait on a second recording
(`plan/HUMAN.md`).

---

## Reading the PC exe first: aim the proof, don't widen it

MX vs. ATV Unleashed's PC exe (`context/mxvatv-pc/MXvsATV.exe`, a Ghidra program of its own since R10;
`research/mxvatv-pc.md` has the class map) is the same engine compiled for a CPU Ghidra decompiles
well, with MSVC RTTI class names, vtables and `.cpp` assert strings left in. The PS2 decompilation of the
same function is anonymous MIPS with the branches folded into delay slots. So the exe is the best map
there is of *what a function does*. Use it to make the proof smaller and sharper, not larger
(user, 2026-10-26: "more accurate with less specific tests since the EXE is so helpful").

**The exe is never the oracle.** Its float is x87 (80-bit intermediates, ≈1,900 `fmul` against 115
`mulss`), much of it is Xbox-derived (`XboxParticleSimulation`, `XboxVertexBuffer`), and it is a later
game: struct offsets moved (the engine model's fields sit 0xC lower), constants were retuned, and the
vehicle grew a rider object of its own. Its numbers prove nothing about MX Unleashed. Bit-exact proofs
stay against the PS2 ELF (SLUS-20767, or SLUS-21104 for MX vs. ATV's PS2 build) through the oracle, and
against PCSX2 through captures. Never put the exe's output in an `assert_eq!`.

How a task uses it:

1. **Find the function in the exe first.** A `pc_<name>` row in `research/oracle.txt` pairs it with the
   PS2 function already; otherwise look it up by class and slot (`./kit fn MotocrossRider::vf44 -o pc`),
   by source file (`context/decomp/pc.sources.tsv`), or by a rare string both builds read. Pairing a new
   one is worth an oracle row (`pc_<ps2 name>   pc <addr>   = <ps2 name>: how you know`).
2. **List every branch, clause and constant there,** in the journal: each `if`, each clamp, each early
   return, each special case (zero speed, airborne, wrecked, a surface index), each float constant and
   the order of the calls. Then find each one in the PS2 function (`./kit fn <name>`), which is where its
   numbers come from. A clause you cannot find on the PS2 side is either a PC addition (say so) or a
   branch you misread there.
3. **Aim the proof at the list.** For the oracle: shape the generated inputs so every branch on the list
   is taken thousands of times — the ranges either side of each threshold, exact zero and negative zero
   at each comparison, each surface or state index — rather than one broad uniform range that visits the
   rare branch twice. Count the hits per branch in the test if it is cheap, so a branch that the
   generator never reaches fails loudly instead of passing quietly. For stateful code: one short capture
   per branch the list says needs one (a state load, a `--trace-at` teleport, a hand-written `.p2m2`
   that does exactly that one thing), not many long rides in the hope that one of them hits it.
4. **A clause the exe has and the port lacks is a finding.** If the PS2 function has it too, it is a bug
   in the port: file it (`./kit plan add <PREFIX> "..."`) unless it is the task's own work. If only the
   exe has it, write it under the journal's `Not verified / not 1:1` as a PC-only difference, so nobody
   ports it later by mistake.

The list from step 2 goes in the journal entry, with each item marked *proved by* (the test or capture
that covers it) or *not covered* (and why). That is what makes a few aimed tests worth more than many
broad ones: the coverage is written down rather than hoped for.

---

## Choosing, in practice

- Is it a pure function of numbers? → **oracle**, 10k inputs, done in an hour.
- Does the answer depend on what happened before? → **capture**.
- Is it a lump of data? → **save state**.
- Did you just change something that could affect the simulation? → **replay**, and you did not have to
  think about it.
- Is it the renderer, a menu, or presentation? → by eye against a screenshot, per the rules in
  `AGENT.md`. Don't build a proof harness for something whose spec is "looks right"; you will spend a
  week matching rasterisation that nobody can see.

## And when you cannot prove it

Write it in the journal's `## Not verified / not 1:1` section, with the reason. Every task has that
section and `./kit done` checks it exists. [docs/EXAMPLE.md](EXAMPLE.md) ends with one, written for a
nine-line function, as a worked sample of the right level of detail.

This is not bookkeeping. A long port accumulates unverified corners at a rate of one or two per task, and
the only thing that decides whether the project converges is whether those corners are written down. A
list of forty known gaps is a work plan. Forty gaps nobody wrote down is a game that is subtly wrong in
ways that will never be found.
