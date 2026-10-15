# 1 — s14 freeride opponent (READY)

Task S14. 2026-10-15. Research only; no code, no tests. This feeds S8 (the acceptance window) and S10
(the landing test).

## What I did

Five questions, answered in this order:

1. **The AI-interface bit 0x40 that `hit_freeride_attach` sets** — the head start, done first because
   everything else assumes freeride opponents behave the same in every area. They do not, and
   `research/oracle.txt` had the polarity backwards (fixed in this commit).
2. **Which frame of `first_recording.p2m2` the NPC contact lands on** — *inferred, not measured*; see
   **Not verified**.
3. **Whether Free09 spawns opponents deterministically from the save state** — the count yes, the
   riders no.
4. **What `OPPONENT_ENABLED` / `OPPONENT_DIFFICULTY` were set to** — neither is a settings key; the
   simulation-side truth is `NumberOfRacers = 4` and the event's AI difficulty byte = 90.
5. **Whether a crash swaps the physics set out from under S10's landing test** — it does not swap the
   bike's parameter block, but it does take the body away from the bike physics entirely.

Plus one thing I did not go looking for: **`watch/acceptance.toml`'s pointer chain reaches an AI
rider, not the human**, in this very save state. A replacement chain is below, live-verified.

## What the original does

### Bit 0x40: the two areas where the AI stops looking ahead

`./kit fn 0x00330630` (`hit_freeride_attach`) branches on the player's kind at `player+0x34`
(1 human, 2 AI). For an AI player it binds `MXVehicleAIInterface` and then:

```
if (strcmp(world+0x140, "Free10") == 0 || strcmp(world+0x140, "Free06") == 0)
        ai->flags(+0x30) |=  0x40;
else    ai->flags(+0x30) &= ~0x40;
```

`strcmp` is `./kit fn 0x00410fe8` (the tail is `subu $2,$3,$2`). I read the MIPS as well, because this
is a polarity the whole answer hangs on: `./kit dis 0x003307b0 0x00330818` has `ori $2,$2,64` on the
match path and an `and` against `-65` on the else path. **Bit 0x40 is set on Free10 and Free06 and
cleared on every other area, including Free09.** `research/oracle.txt` said the opposite; corrected.

What the bit gates: `./kit fn 0x002858d0` (VehicleAI.cpp) and `./kit fn 0x00386d30` (MXVehicleAI.cpp)
both build the AI's look-ahead feeler fan and run the per-rider avoidance scan **when**

```
(flags & 0x80) == 0 || (flags & 0x40) == 0
```

— i.e. the probing is *skipped only when both bits are set*. Bit 0x80 is the per-tick one, set and
cleared by `./kit fn 0x00383580`. A third test in the same loop, `(flags & 0xc0) == 0`, admits the
type-3 scenery objects to the scan only when neither bit is set.

So: on Free10 and Free06 the look-ahead is suppressed on the ticks where 0x80 is up; on Free09 bit
0x40 is clear, so the opponents run the **full** path every tick — feeler fan, per-rider avoidance and
the scenery branch. Freeride opponents are not uniform across areas, and the area the recording was
made on is the expensive one.

### Who is in the level

From the recording's save state (`./kit state ram`, walked offline — see **How it was proved**):

| list | what |
| --- | --- |
| player manager (`game_manager -> 0x30`) | count 5: player 0 kind 1 (human), players 1–3 kind 2 (AI riders), player 4 kind 2 (a type-1 actor, not a bike) |
| actor manager (`game_manager -> 0x34`) | count 9: four type-2 `MotocrossVehicle`s, four type-4 bike actors, one type-1 |

Each player points at both halves of its bike: `player+0x48 -> +0x0` is the `MotocrossVehicle`
(type 2), `player+0x4c -> +0x0` is the type-4 actor the acceptance capture watches. That holds for all
five players, so it is structural, not a coincidence of one object.

The physics names on the vehicles are the measured answer to D7's open edge: the human's vehicle has
`Bike\Race125P.phy` at `+0x84`, the three AI vehicles have `Bike\Race125A.phy` there, and **all four**
have `Bike\Wreckbike.phy` at `+0x188`. `./kit fn 0x00208c50` formats the P name and the A name into two
stack buffers and hands both to the build; which one lands in `+0x84` is decided downstream and I did
not trace it (see **Not verified**).

### Where the four racers come from

`./kit fn 0x002b0a20` is the branch that starts a freeride (league 2, activity type 5). It sets the
activity type, then:

```
if (player count == 1) NumberOfRacers = 4; else NumberOfRacers = player count;
```

and seeds `srand` from the wall clock. `./kit fn 0x00207b20` then spawns the humans and
`NumberOfRacers - humans` AI riders, each identity chosen with `rand()`, and takes the AI skill range
from the event's `+0x117`.

So the **count** is deterministic: a one-player freeride is always four racers, with no menu and no
setting involved. The **identities** are not — they come off a clock-seeded LCG, so the only way to
reproduce this session's three opponents is to start from this save state.

### `OPPONENT_ENABLED` / `OPPONENT_DIFFICULTY`

Neither is a `.cfg` key, and nothing on the simulation side reads either name. They are front-end
identifiers: `./kit fn 0x002ccd10` is the screen loader that binds widget names, `"OpponentDifficulty"`
among them. `plan/HUMAN.md` calls them "real settings keys"; they are not, and nothing in the recording
was configured through them.

The simulation-side equivalents for this run are: opponents on, because `NumberOfRacers = 4` (forced,
above), and difficulty = the event's `+0x117` = **90** (0x5a) — the `MXEvent` constructor default,
i.e. never written by a menu. `./kit fn 0x00207b20` maps 90 to an AI skill range of [4, 4].

### What a crash does to the physics set

`./kit fn 0x0024fca0` (reached from `./kit fn 0x0027aa70`) builds the wreck name once, at vehicle build:
`snprintf(obj+0x188, 0x104, "%sWreck%s.phy", "Bike\\", <class>)` → `Bike\Wreckbike.phy`. The `.phy`
loader `./kit fn 0x002539e0` opens that name at the end of the build and hands it to the subsystem at
`obj+0x7a8` (vtable slot 0xe8) — the ragdoll. The bike's own parameter pointers, `obj+0x7a0` and
`obj+0x7a4`, are never rewritten by a wreck.

So the answer to S10's question is: **no, a crash does not swap the parameter set under the landing
test** — but it does something that matters just as much. `./kit fn 0x00252ca0` is `Wreck(reason)`
(vtable `+0xfc`): it sets `obj+0xb7c = 1` and hands the body to the ragdoll subsystem, which integrates
it from `Wreckbike.phy` from then on. A landing test must therefore end at the frame `+0xb7c` flips;
past that frame the bike physics is not what is moving the bike. On `reason == 0xd` it also draws
**three** `rand()` values into the ragdoll at `+0x1b0/+0x1b4/+0x1b8`, and the wreck test itself
(`./kit fn 0x002522c0`) draws two more at 0x00252534 and 0x00252568.

### Where the tick-exact acceptance window has to end

This is the conclusion S8 needs, and it is a measurement, not an argument from the code.

I loaded the recording's save state on emulator copy 4 and captured 300 samples with **no input at
all** (`context/capture/s14/s14-idle.csv`, 0 torn reads, frames 8078..8676). With the human parked and
nothing being pressed, the shared C-library LCG at `rand_state` still advances **2 to 19+ draws per
simulation tick, median ≈ 8**. The per-tick bike physics draws none of them: they are the opponents,
their AI, and the effects. `dt` was 0.033330000936985016 on every row, confirming the half-rate tick.

Therefore:

- **A port that does not simulate the opponents cannot match `rand_state` at any tick past 0.** Not
  approximately — the state is 64 bits and wrong after the first tick.
- **The tick-exact window ends at the first wreck or contact**, because that is the first event where
  the player's own path consumes draws (2 in the wreck test, 3 more in `Wreck(0xd)`) whose values
  depend on how many the opponents consumed before them. Up to that point a port can stay bit-exact on
  the player's state while ignoring the RNG; after it, it cannot.
- **The minimum opponent simulation to go past that point** is therefore not "move three bikes
  plausibly": it is every `rand()` call the three AI riders and their effects make, in the original's
  order, on an area where bit 0x40 is clear so the full look-ahead path runs. That is the whole AI, and
  S8 should not plan on it for the first acceptance run. The cheap alternative is to judge the port on
  the player's state only, and stop the comparison at the contact.

### The acceptance chain reaches the wrong rider

`watch/acceptance.toml` walks `@game_manager -> 0x34 -> 0x34` then five `-> 0x8` hops. In the
recording's save state that lands on actor index 5, which is **an AI rider**: `a5_type = 4` and
`a5_kind = 2`, moving, while the human is index 8 (`a8_type = 4`, `a8_kind = 1`, nearly stationary).
Both were confirmed live in the idle capture, not just offline. The spec's own guard does not catch it:
`bike_player != 0` is true for an AI rider too, because an AI *has* a player object — the kind byte at
`player+0x34` is what separates them.

The fix is to stop counting hops and go through the player manager, whose head is the human
(`./kit fn 0x00207b20` creates the humans before the AI riders):

```toml
[[field]]                       # must read 1; if it reads 2 the capture is of an AI rider
name = "p0_kind"
addr = "@game_manager -> 0x30 -> 0x34 -> 0x0 -> 0x34"
type = "u32"

[[field]]                       # +0x00 of the actor: still must read 4
name = "bike_type"
addr = "@game_manager -> 0x30 -> 0x34 -> 0x0 -> 0x4c -> 0x0 -> 0x0"
type = "u32"

[[field]]                       # +0x130 velocity, +0x180 transform: same fields, right rider
name = "bike_vel"
addr = "@game_manager -> 0x30 -> 0x34 -> 0x0 -> 0x4c -> 0x0 -> 0x130"
type = "f32x4"

[[field]]                       # the wreck flag: 0 until the bike wrecks, then 1 — the window's end
name = "bike_wrecked"
addr = "@game_manager -> 0x30 -> 0x34 -> 0x0 -> 0x48 -> 0x0 -> 0xb7c"
type = "u32"
```

I did not edit `watch/acceptance.toml`: it is S8's file and another session is working in it. This
block is the whole change.

## How it was proved

- **The bit-0x40 polarity**: read twice, once as C (`./kit fn 0x00330630`) and once as MIPS
  (`./kit dis 0x003307b0 0x00330818`), because the decompiler's `|=` / `&= ~` is exactly the thing that
  gets transcribed backwards — which is how `research/oracle.txt` came to have it inverted.
- **The riders, the links and the physics names**: a save-state RAM image, which is the cheap method
  for a layout question and does not disturb a capture. `./kit state ram` → `context/states/rec-start.ee.bin`
  (32 MiB, git-ignored), walked offline from `game_manager` through both manager lists for all five
  players and all nine actors. Every player's `+0x48`/`+0x4c` pair resolved to a type-2 vehicle and a
  type-4 actor.
- **The replacement chain**: live, not just offline — 10 frames through `./kit record` on copy 4 with
  the chain written in the kit's own syntax, reading `p0_kind = 1`, `p0_bike_type = 4`,
  `p0_veh_type = 2`, `p0_wrecked = 0` on every row.
- **The RNG drift and the dt**: a live PINE capture of 300 samples with no input,
  `context/capture/s14/s14-idle.csv` (git-ignored, in a subfolder so a stale capture cannot redden
  `./kit check` for other tasks), 0 torn reads, frames 8078..8676.
- **The tape**: `first_recording.p2m2` parsed offline — 570-byte header, 18 bytes per frame per pad,
  both ports, 3621 frames.

Emulator copy 4 was stopped at the end (`./kit pcsx2 stop`).

## Not verified / not 1:1

- **The contact frame is an inference, not a measurement.** PCSX2 can only play a `.p2m2` from its GUI:
  there is no CLI flag and no ini key for it (`strings` on the binary turns up `InputRecording`,
  `InputRecordingFile`, `InputRecToggleMode`, `NewInputRecordingDlg.cpp` — class and file symbols, no
  setting), and `./kit pad` drives the pad on the wall clock, which is not frame-exact. What I did
  instead was parse the tape: the only fully idle mid-run window — no buttons, both sticks centred — is
  video frames **1020–1139** (~2.0 s), which is what letting go of the controller after hitting
  something looks like. That puts the contact at **video frame ≈ 1019, simulation tick ≈ 510, ~17.0 s
  in**. Treat the number as ±a few frames and the method as circumstantial. To settle it, play the tape
  in the GUI with `./kit record` running on a spec containing the `bike_wrecked` field above; the first
  frame it reads 1 is the contact, exactly.
- **The three opponents' identities are not reproducible** from anything but this save state: they are
  `rand()` draws off a wall-clock `srand`. The count (4) is deterministic.
- **The 2–19 draws per tick are not attributed to specific callers.** I measured the LCG state moving,
  not who moved it; I did not instrument the call sites. The claim that the player's per-tick physics
  draws none of them comes from reading the bike path, not from a counter.
- **Which code writes the P or A physics name into vehicle `+0x84` was not traced.** `./kit fn 0x00208c50`
  formats both into stack buffers and the decompiler loses them at the call; what is proved is the
  values in the save state (human P, AI A, all four with `Wreckbike.phy` at `+0x188`).
- **The wreck path was read, not run.** No wreck happens in the idle capture, so `+0xb7c` reads 0 on
  every row I have; that the flag flips and the ragdoll takes over is from `./kit fn 0x00252ca0`, not
  from a capture of a crash. The 3 and 2 `rand()` draws are counted in the disassembly.
- **`every = 2`.** The recorder sampled every other video frame in both captures ("300 counter gaps"),
  which is fine for an RNG-drift rate and a chain check but would not do for a fixture.
- **Free10/Free06 behaviour itself was not observed.** The bit's polarity and its gate are read from
  the code; I did not load either area to watch an opponent stop looking ahead.

## Next

For S8, in order: take the player-manager chain above into `watch/acceptance.toml` (the current chain
captures an AI rider, so any fixture built from it today judges the port against the wrong bike), add
`bike_wrecked` as the window's end marker, and plan the first acceptance run as player-state-only up to
the contact rather than RNG-exact. For S10: the landing test may keep using the bike parameter set, but
must stop at the frame `bike_wrecked` goes to 1. For whoever has the GUI: play `first_recording.p2m2`
with a recorder attached and replace the inferred contact frame with the measured one.

One practical note for the next session in a worktree: `kit.toml`'s `[symbols]` is git-ignored, so this
worktree had only R7's eight names. `frame_counter`, `frame_dt`, `rand_state` and `game_manager` were
added back with `./kit sym` (its own atomic writer) before anything here could be recorded; `./kit sym`
wants `0x479d04`, not `0x00479d04`.
