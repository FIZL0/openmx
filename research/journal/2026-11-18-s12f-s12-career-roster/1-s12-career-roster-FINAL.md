# 1 — s12 career roster (FINAL)

Task S12f. 2026-11-18.

## What I did

- **Ported the career branches** of the racer spawn as `ai::spawn_career_roster(&mut Rng, RosterIn,
  &CareerRoster) -> CareerSpawn` in `crates/game-sim/src/ai.rs`. `CareerRoster` is `Saved { count,
  riders }` or `Championship { count, identities }`. `CareerSpawn` carries the roster and the racer
  count the spawn writes back over the settings.
- **Shared the start** (AllAI pair, then the ghost race) between `spawn_roster` and the new function, as
  `spawn_start`. `spawn_roster` behaves as before; its oracle still covers it.
- **Proved it** in `crates/game-sim/tests/ai/career_roster.rs` (a new topic module) by running the
  original's whole spawn with the career byte set.
- **Found where the objects come from**, as far as the spawn reaches: the first player's profile, its
  career, the career's current round. Written up under `spawn_racers_career` in `research/oracle.txt`.
  S13 had not traced this (its journals only mention the +0x116 byte as `alt_outro`), so I read it here.
- opentennis has nothing like a career roster. Nothing taken.

## What the original does

`./kit dis 0x002081d0 0x00208528` (the +0x116 branch of `./kit fn 0x00207b20`):

- The AllAI pair, the ghost race and the humans run first, the same as the default path.
- **The round**: `app+0x3f4` is the per-player profile pointer array (player 0's is the word at
  +0x3f4). Its +0xb0 is the career. `./kit fn 0x0036c0b0` returns the career's +0x5c: the round.
- **Round +0 == 0, the saved roster**: the count is the byte at round+0x68. It goes straight into the
  settings' NumberOfRacers; the humans are not added. For i in 1..count, AI rider i (numbered on from
  the humans) takes the entry pointer at round+0x48+4i: identity = entry+0x74, skill = entry+0x78.
  Nothing is drawn.
- **Round +0 != 0, the championship**: `./kit fn 0x00369cb0` with a0 = the round (Ghidra drops the
  argument) returns round+4, the championship. The skill range is the default path's, from event
  +0x117. NumberOfRacers = humans + champ+0x38. AI rider i: identity = champ+0x2c+4i, skill = one
  draw, same shape as the default path. No identity draws.
- **Room**: seven saved entries fit between +0x4c and the count byte; three championship identities
  fit before the count word. More reads the count words themselves as riders.

The identity words go into the player's +0x6b8 as they are. On the default path +0x6b8 is the rider
database's entry pointer (`./kit fn 0x003712b0`). So the port's `identity` stays a database index and
the caller turns the career's entry pointers into indices.

## How it was proved

**Function oracle over 10,000 generated spawns**, `crates/game-sim/tests/ai/career_roster.rs`. Same
harness as S12a's `roster`: allocator, constructor and the two virtual calls hooked. Added the career
chain in RAM and both lists. The cases cover saved counts 0–8 with seven riders listed (random
identities and full-range skills), championship counts −2..3, every difficulty edge, humans 0–4,
AllAI 1 in 8, ghost race 1 in 16, and a random generator state. The count byte shares its word with
junk bytes, and the rider database is empty, so reading either would show. Each case checks every
player (kind, number, identity, skill), the racer count written back, the AllAI pair, and the
generator state afterwards. The test asserts each branch is reached often (saved > 3000, championship
> 1500 with mixed skills > 300, ghosts > 300, AllAI > 600).

Two mutants fail it: the saved branch adding the humans to the racer count, and the championship's
identities handed out in reverse.

My first harness pointed `app+0x3f4` at a list of profiles. The spawn reads the word itself as the
profile, so it fell into the saved branch on a zero count and the test failed. That is how I know the
array starts at +0x3f4.

## Not verified / not 1:1

- **What a real round holds** was not read from a save state. The words at saved-entry +0x74 and
  championship +0x2c are taken to be rider database entry pointers because +0x6b8 holds one on the
  default path, and the standings screen (`./kit fn 0x0029ea60`) names a rider from a record's +0x74
  through `./kit fn 0x00373340`. No career state exists in context to check it.
- **Who fills the round, and when** (the career's +0x5c, round +0 / +4 / +0x4c / +0x68, champ +0x2c /
  +0x38) was not traced. Nothing in the port calls `spawn_career_roster` yet: openmx has no career mode.
  The menu-side reader `./kit fn 0x002b0fe0` reads a different championship field (+0x78) for its
  racer count, so these may be two object types behind the same +4 getter.
- **Reading past the lists panics instead.** A saved count above 8 or a championship count above 3
  makes the original read its own count words as riders. The port asserts; the oracle stays inside.
- **The lazy wall-clock `srand`** is left to the caller, as in S12a.
- **Not against a live capture.** No career state exists, and the career round needs menus this
  slot cannot drive. The oracle runs the original's own instructions over the whole input space.

## Next

Nothing for S12f. A career mode in openmx (none is planned in this task) would call
`spawn_career_roster` and needs the round filled from the save first.
