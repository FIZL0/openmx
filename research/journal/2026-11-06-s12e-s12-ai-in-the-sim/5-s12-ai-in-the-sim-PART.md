# 5 — an AI rider's brain spawned, proved whole and on free09's riders

2026-11-12. After S317 landed. This part covers part 4's "Next" #1: a `Racer` built from the roster's
`Character`.

## What was done

- **`ai::spawn(rng, kind, mode, event_kind) -> Spawned`** builds a fresh motocross AI brain. It is the
  original's `ai_mx_spawn` (boot 0x00384cc0), which does the following in order:
  1. allocates 0x400 bytes;
  2. runs the motocross constructor (`ai_mx_construct` 0x00382c50), which first runs the vehicle
     constructor (`ai_vehicle_construct` 0x00282ca0);
  3. attaches the brain to the rider's interface (`ai_attach` 0x00283000: brain+0x40 = interface,
     brain+0x30 = the flags);
  4. picks the mode (`spawn_mode`: the event kind at app→+0x3d4→+0xc; kind 2 rides, 0 and 1 are mode
     2, anything else is 0), unless the caller gives one;
  5. runs the set-up.

  `Spawned` holds three things:
  - the `Racer` (brain, senses, seeker, tuning, avoidance, vehicle, filter, reach, plan, controls);
  - the `Character` as the constructor leaves it, before the reader;
  - a `Wander`: the follower's ten construct-time draws and its drift, kick and period words. Nothing
    builds a follower from it yet.
- **`Racer::take(&Character)`** copies a character (after `read_character`) into the racer's tunings:
  - steering and avoidance;
  - the filter's rate and jitter;
  - the reach's base and max;
  - the trick plan's cooldown and chance;
  - the habits' numbers;
  - the assist's two weights.
- `SPAWN_FLAGS` names the flag word the spawn leaves on.
- In the tests, `character::ch_slots` and `ch_retail` are now `pub(crate)` so `spawn.rs` can use them.

## How it was proved

`tests/ai/spawn.rs`:

- **`the_brain_spawn_matches_the_originals`** runs the whole spawn in the interpreter. Only
  `operator_new` is hooked, and it hands out a buffer the test filled. Ten thousand cases cover:
  - the generator anywhere in its chain;
  - event kinds −1..4;
  - a given mode in a third of the cases, else the pick;
  - interface kinds 0..7;
  - the buffer filled with zeros, or with a NaN pattern no constructor writes.

  All 256 words of the brain are checked, each one of three ways:
  - **Words the port models** must equal the port's value, bit for bit. Where two of the port's
    fields share a word, both are checked.
  - **Words no constructor writes** must still hold the fill. The port has these as 0, so the port
    matches a zeroed allocation.
  - **Written words the port has no field for** are listed in `SP_UNMODELLED` and skipped (see the
    next section). Any other word must still hold the fill.

  The test also checks three more things:
  - the generator's state afterwards;
  - the brain↔interface pointers;
  - that each mode branch is taken.

  It passes, all ten thousand cases.
- **`free09s_ai_riders_spawn_as_the_image_holds_them`** builds each of recording2's three AI riders
  the way the game does. It reads three things off the image:
  - the skill (interface +0x104);
  - the event's difficulty byte (event+0x117 = 90);
  - the event kind (2, ride).

  Then it runs `spawn` + `read_character` (on the retail `AIChrctr.aip` from MX.PSR) + `take`. The
  test compares the following against the image's brain and interface: the mode; the spawn's flags;
  75 tuning words the tick never writes; and 15 habit/assist words on the interface. Every one
  matches on all three riders.

## Not verified / not 1:1

- **Written words with no port field** (`SP_UNMODELLED`). The constructors write these, but no ported
  code reads them yet:
  - the base object, below +0x30 (its type, a refcount, a 0xff, a flag byte, and a global object
    count it increments);
  - +0xd4/0xd8/0xdc;
  - 0x108 = 1;
  - the ignore list at 0x150/0x154;
  - 0x1c4/0x1cc;
  - 0x20c = 0.5;
  - 0x224/0x228;
  - 0x258 = 100;
  - the feeler bundle at 0x2a0;
  - 0x2b8 (0.8, which the set-up then zeroes);
  - 0x314/0x318 = 0.2, and 0x31c = 1;
  - 0x39c, 0x3a0;
  - 0x3b8..0x3cc, the motocross numbers.

  The global object count is a side effect the port does not keep.
- **Unwritten words are taken as 0.** The original's allocator returns memory that was not cleared.
  The port's 0 for these words (the spin, the plan's air, the ground, the seeker's aim, the
  controller's angles and flat forward, and the far range) assumes they are never read before
  something writes them. The joined tick (part 4) supports this on free09, but it is not proved in
  general.
- **The interface's own defaults are not built.** For the profile words the reader keeps where the
  file has no key, the free09 test copies the image's values in before reading. The rider's
  interface constructor is not ported. Filed as S318.
- **Reach gain and lead are not compared on free09.** The bike's race start (`bike_race_start`
  0x0027f5e0, AI branch) overwrites the reach gain at brain+0x218 with the rider's +0x148 (or +0x14c
  on one race type). It also makes three or four rand() draws. The seeker's own states set lead
  (+0x25c). Neither is part of the spawn, and the race start's AI branch is not ported (S318).
- **Gate delay and wheelie start are not compared.** The reader draws both, and the image does not
  record the generator's state at the spawn.
- **No AI riders in the Sim yet.** Nothing gives a spawned `Racer` a bike or steps it in `tick.rs`, so
  `plan_trick`/`gesture_gate` are not wired. Filed as S319.
- **The free09 4-racer capture is not recorded** (filed as C33). The replay the task asks for is
  blocked on it and on S319.
- **The follower is not built from `Wander`.** Only its construct-time words are proved.

## Next

S12e is blocked after S318, S319 and C33. When they land, replay C33's capture through S319's lane,
checking `rand_state` and the three AI bodies each tick.

## Gate note

The first `kit done` was red on atv_quad's `the_sweep_that_hits_the_palm_is_the_ports`. That test was
already red on this branch's base, and K51 fixed it on main (42d6c765e). The fixed
`tests/atv_quad/sim_frame.rs` was taken from that commit unchanged (the commit's parent had the same
file as this branch). Nothing else from main was merged.
