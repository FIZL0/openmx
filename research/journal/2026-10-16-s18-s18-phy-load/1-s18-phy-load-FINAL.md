# 1 — s18 phy load (FINAL)

Task S18. 2026-10-16.

## What I did

Wrote `crates/game-sim/src/phy.rs`: the game's own plain-text reader, and the whole of the bike's
`.phy` loader on top of it. Every physics parameter in the port now comes off the disc instead of
out of a test — the key → offset map S3 recovered by reading, run for real.

| piece | original | what it is |
|---|---|---|
| `Ini::parse` | `ini_open` | the file split into sections and pairs, the way the reader splits it |
| `Section::{text,float,float_or,int,flag,vector}` | the six getters | one value, six ways, including the two that leave their destination alone |
| `Phy::read` | `bike_phy_load` | the seventeen sections into the bike, its chassis, its two wheels, its engine and its collision record |

`Phy` carries the sections as they are written (`Mass`, `Surface`, `Tire`, `Peak`, `Roll`, `Pitch`,
`Joystick`, `Donuts`, `Balance`, `WreckTimes`, `Collisions`, `Shoulders`, `Animation`) and, filled
from across them, the two sets the rest of the port had been waiting for: `move.rs`'s `Params` and
`air::Params`, together, from one pass. `max_crossup` is the value S6 could not prove — the retail
files say `MaxCrossupAngle = 45`, and 45 degrees in radians through the loader's own multiply is
`0.78539807` (`0x3f490fd9`), two bits below a quarter turn because `mul.s` truncates.

The freeride bike is D7's: `physics_file(FREERIDE_ENGINE, FREERIDE_PRO, Tuning::P)` is
`Bike\Race125P.phy`, and proof 4 below finds the object in the human's own recording carrying exactly
that name, so the bike the port reads is the bike the human rode.

Four things that were not where the port's reader thought they were, and are now:

* `strtol` here works in a 64-bit `long`, so a value too big for an `int` is **cut, not clamped**:
  `1234567890123` reaches the destination as `1912276171`. The port was saturating at `i32::MAX`.
* the double-to-float routine **drops the sign of a zero**: `-0.0` in a file is a plain zero.
* the flag getter is the integer getter with a default bolted on, so it writes the whole word
  (`Flag = 400` lands as 400, not 1). The port narrows to a `bool` — see **Not verified**.
* the stack argument slots of the one setter that needs them are **eight bytes apart**, and the one
  the front tire has no key for is a zero the compiler stores, not a leftover.

`research/oracle.txt` gained the S18 section: the ten reader entry points and the two pieces of
`ini_open` worth naming, `strtod`/`strtol`/`strtok`, the four file calls, `rigid_body_set_mass`, the
seven wheel setters, the seven collision setters, `bike_physics_lines`, and the three the oracle has
to stand in for (`libc_lock`, `libc_unlock`, `sbrk`).

## What the original does

`./kit fn 0x002539e0` is the loader; `./kit fn 0x0014a3e0` is the reader it opens the file with.

**The reader.** It reads the whole file, then walks it a line at a time. Whitespace is *removed*, not
trimmed — `Spaced Key = 2` is the key `SpacedKey` — except inside `[...]` or quotes, and quotes
themselves are removed. A line whose squeezed form starts with `;` is a comment; `[` starts a
section; everything else is a pair, split at the **first** `=`, and a pair whose value is empty is
dropped. Matching is case-insensitive, and a second `[Section]` with a name already seen is a second
section whose keys the getters never reach. The getters: text copies the value; int takes
`T/F/ON/OFF/Y/N` first and otherwise `strtol` with base 0 (`0x20` is 32, `020` is 16); float is
`strtod` and then the EE's software double-to-float, which **truncates** (`./kit fn 0x00418700`);
bool and the float-with-a-default are those two with a default; quadword is `strtok` with three
delimiter sets in turn (`" \t,;"`, then `"();"`, then `"(),"`), and a lane the text does not reach
keeps the default's. Two getters — text and float — have no default and leave the destination
untouched when the key is absent, which is what "the constructor's value stands" depends on.

**The loader.** Seventeen sections, read in file order, into five objects. Most keys are a float
straight into the bike, but a dozen are worked on as they land: `Weight/32.2` goes to the chassis
through its vtable (`+0x78`); speeds are multiplied by `1.46666` (mph to feet per second) and angles
by `0.01745329`; `WheelieTopSpeed` arrives as a speed and is stored as `1/(v*1.46666)`;
`CollisionWeight` becomes the record's own mass unit at `*0.0310559`; two angles —
`NoHanderMaxRollAngle` and `HighSpeedMinCollisionAngle` — are not stored but **looked up in the
engine's shared `{cos, sin}` table**, each wrapped into `±pi` twice before indexing, so the table has
to exist before the loader runs. The spring rates are stored twice: the loader hands a rate to
`wheel_set_suspension`, reads back what the wheel made of it (`wheel+0x54`) into the bike, and does
it again for the other rate — which is why the port's `spring_rate` is `(rate * mass * 32.2).abs()`.

The two wheels and the collision record are not written by the loader at all: it calls fourteen
setters. `[Front Tire]` gets the suspension twice (the stopped spring, then the rolling one),
`[Rear Tire]` three times (the rolling one again, after both read-backs); then the tire, the front's
steering, and the numbers the wheel derives for itself. `NumberOfSurfaces` rows follow, three calls
each: the drive forces, the side forces — thirteen arguments, five of them on the stack — and the
braking torque with the friction and drag. The engine's three arrays are allocated, filled from
`RPM%d`/`HP%d`/`GearRatio%d` and freed again, the setters having copied them. The collision record
gets its mass and centre of gravity, its eight springs, the high-speed check, the two angular
switches, four moment-scale quadwords, two frictions, and then one terrain friction written into
**all sixteen** surface slots whatever `NumberOfSurfaces` said.

Last, the loader opens a *second* file — the name at `bike+0x188`, the wreck bike's own `.phy` — and
hands the opened reader to the rider through a vtable slot (`+0xe8`). That is a different object's
parameters and is not part of this task.

## How it was proved

`crates/game-sim/tests/phy.rs`, five tests, all against the original: 1987 keys and 42.5 million
interpreted instructions of loader.

1. **`the_readers_primitives_are_the_games_own`** — every `(section, key, value)` of three files
   (a generated file written to be awkward, and both retail `Race125` files), each read by the
   game's six getters and the port's six, 1987 keys and 85.7 million instructions. Then six absent
   keys and a missing section, so "reports absent" and "leaves the destination alone" are proved
   too. The generated file squeezes spaces and tabs, quotes keys and values, drops an empty value,
   repeats a key, repeats a section, spells a vector four ways, spells a flag nine ways, and holds
   hex, octal, signed, exponent, leading-point, trailing-garbage, non-numeric, huge and tiny numbers
   and a negative zero.
2. **`the_whole_loader_fills_the_bike_the_way_the_original_does`** — `bike_phy_load` run on a graph
   laid out as the constructor leaves it and **zeroed**, with the file served to the game's own four
   file calls, against the port poked into the same graph from the same zeroed start: **960 words**
   of the bike, the chassis and the engine, bit-exact, plus the power curve and the gear ratios out
   of the arrays the engine's setters allocate. Zeroed on both sides, so a word only one of them
   writes is a word that differs.
3. **`the_setters_are_handed_what_the_port_works_out`** — a second run with all fourteen setters
   hooked, each recording the registers and stack slots it was called with and returning: **128
   calls**, every argument bit-exact and in order, including the six words the loader puts on the
   wheels itself.
4. **`the_bike_in_the_recording_is_the_one_the_port_reads`** — no interpreter at all: the EE image
   saved beside the human's recording (`first_recording.p2m2_SaveState.eeMemory.bin`), the bike
   found by the file name it carries, the curve table taken out of the same image, and the file read
   off the disc. **190 words** of a bike the original's loader filled a whole race earlier, bit-exact
   against the port's.
5. **`the_race125_pair_holds_the_retail_numbers`** — both `Race125` files off the disc: 400 pounds,
   ten curve points, five gears, sixteen surfaces on both wheels, `Drag` the float below a fortieth,
   and `MaxCrossupAngle` at `0x3f490fd9`.

## Not verified / not 1:1

- **A flag is a `bool` here and a whole word there.** The original's flag getter writes whatever
  integer `strtol` makes of the value; the port narrows it to `true`/`false`. Proof 1 compares the
  word itself wherever the original's answer is 0 or 1 — which is every flag in every retail file —
  and the *truth* of it otherwise. A modded file with `AngularImpulseEnabled = 400` would differ.
- **A pair before the first section is not compared.** The original indexes its section array at
  `-1` and reads unmapped memory for it (`0x0014c9d8`, address `0xfffffff4`), so there is no answer
  to compare against. The port drops such a pair. No retail file has one.
- **A decimal outside a float's range is not compared.** The double-to-float routine reassembles the
  exponent without checking it, so `3.5e38` becomes `0x7f8a4f46` — an infinity-or-NaN pattern —
  where the port's `chop` clamps to the largest float. The generated file stays inside the range;
  underflow and denormals are not compared either (it stops at the smallest normal).
- **`strtod`'s other spellings are not modelled**: hex floats (`0x1p3`), `inf`, `nan`. No retail
  file holds one.
- **Lines longer than the reader's 1024-byte buffer are not modelled or compared.** The longest
  line in either retail file is far shorter.
- **`strtol`'s saturating case is not proved.** The port works in an `i64`, clamps at *its* limits
  and keeps the low word; the truncating case is proved against the original
  (`1234567890123` → `1912276171`), the clamp above `i64::MAX` is not.
- **The repeated-key cache is not modelled.** The reader keeps a cursor so a second getter for the
  same key starts where the last one stopped; the port looks up each key from the start of the
  section. Only the answers are compared, and they agree for 1987 keys read six ways.
- **What the wheels and the collision record make of their arguments is not compared** — only the
  arguments, which is what the loader is responsible for. The one derived word that comes back to the
  bike (the spring rate the suspension setter works out, `wheel+0x54` → `bike+0x6f4..0x700`) is in
  proof 2's comparison, with the real setter running. The rest of `wheel_set_tire`,
  `wheel_set_steering`, `wheel_set_tire_derived`, the three surface setters and the seven collision
  setters is for the tasks that port those objects.
- **Proof 2 hooks the seven collision setters away.** The record is not one of its regions, proof 3
  covers their arguments, and one of them (`0x00270100`) runs an EE parallel instruction
  (`pminw`) the interpreter has not been taught.
- **Three words of the engine are excluded from proof 2** and documented in the test: the two array
  pointers and the cursor's pointer are allocator addresses, which a port has no business
  reproducing (the arrays they point at *are* compared), and `EngineInertia` itself is kept by the
  original beside the reciprocal it uses, which nothing reads.
- **Two words of the engine are excluded from proof 4**: the cursor the engine walks its power curve
  with (`+0x90`, `+0x94`), which the loader leaves at the start of the curve and which had moved by
  the time the image was saved. They are compared in proof 2, where the loader has just run.
- **The oracle stands in for four things**, none of them anything the reader reads: the game's
  allocator (a bump allocator), the C library's `sbrk` (a second bump, contiguous, because its malloc
  checks), `memset` (which broadcasts its byte with an EE parallel instruction), and both halves of
  the C library's lock (`GetThreadId` and `WaitSema` are kernel syscalls). `strtod` itself, including
  the hard rounding path that allocates, runs for real.
- **`CurveTable::build()` is still not bit-exact** (S7). Proofs 2, 3 and 4 take the table out of the
  game's own RAM, so the two cosines the loader looks up are the game's; proof 5 asserts nothing that
  comes through the port's table.
- **The second `ini_open` is not followed.** The loader opens the wreck bike's own `.phy` (the name
  at `bike+0x188`) and hands the reader to the rider's vtable slot `+0xe8`. The proofs keep it out by
  answering `fs_open` only for the bike's own file name, and the port stops at the bike's file.
- **`bike_physics_lines` (`0x00253900`) is not ported.** It is inert unless `bike+0x7c8` is set,
  which is how the proof's graph is laid out and how a freshly constructed bike arrives.
- **`rev_limit` with `NumberOfRPMPoints = 0`.** The original's curve setter reads `rpm[count-1]` with
  no guard, which for a zero-length array is memory it never allocated; the port returns
  `curve_rpm[0]`. No retail file has fewer than ten points.
- **`MaxRollRateCommand` is absent from both retail files**, so the original leaves `bike+0x6dc`
  alone and the port leaves its own default. Proof 4 shows the live bike holding zero there, so zero
  is right for the bike the human rode, but nothing proves what a file *with* that key would do —
  the key is read by a required getter, so it would simply land.
- **The rear tire's steer limit is never written** (`MaxSteerAngle` is a `[Front Tire]` key), and the
  front's `OtherTireNormalLoadScale` and `DonutsSideForceSpeedScale%d` are never read. The port's
  `Tire` carries those fields for both wheels; for the wheel that has no key they hold the port's
  default, and only the front's donuts zero is proved (the original passes a literal zero).

## Next

Nothing in this task. For whoever picks up the wheels or the collision record: proof 3 already has
the arguments those objects are handed, in order, as a table — `tire_calls` and `collision_calls` are
the two functions to read, and the setters are named in `research/oracle.txt`.
