# 1 — r6 actor table (FINAL)

Task R6. 2026-10-15.

## What I did

Found where the game keeps its live objects, and what the first fields of one are. The short answer, and
the thing to carry away: **there is no actor array.** Nothing in this engine has a base address, a stride
and a count in the sense the task line asks for. Live objects are held in intrusive-free doubly-linked
lists owned by a handful of managers that hang off one singleton pointer, and the only "stride" in the
picture is the 12-byte list *node*. So the deliverable is a pointer chain rather than a base and a stride,
and it is written down twice: in prose below, and as an executable spec in `watch/actor-table.toml` whose
every `->` is one link of the claim.

Then I mapped the fields of one actor — type, flags, owner back-pointers, the four rigid-body points, the
velocity and the 4×4 world transform — by diffing five save states taken ~62 ticks apart in a running
Freestyle session, and confirmed the whole chain and the field meanings with a 120-tick live capture that
re-resolves every pointer every frame.

The velocity turned out to be the interesting part: position does **not** advance by `vel * dt`. See
"Position is integrated with a fixed 1/24 s step" below.

## What the original does

### The registry

```
0x004e1e98                       GameManager*  — the singleton; null outside a level
  +0x30   GamePlayerManager      (0x50 bytes)  vtable 0x00454200
  +0x34   GameActorManager       (0x48 bytes)  vtable 0x00452ae0
  +0x38   a third manager        (0x50 bytes)  same shape, not identified
  +0x3c   (0x148 bytes)          not identified
  +0x40   (200 bytes)            not identified
```

The singleton is written by the GameManager constructor — `./kit fn 0x002462d0`, whose last statement is
the store to 0x004e1e98. Every manager is one instantiation of the same container template
(`ContainerLinkedList.h`, named by the `__FILE__` string its `operator new` call sites carry):

```
manager +0x30   u32    count
manager +0x34   Node*  head
manager +0x38   Node*  tail
manager +0x3c   u32    count   ) a second list, of per-type groups
manager +0x40   Node*  head    )
...
Node +0x00  void* item
Node +0x04  Node* prev
Node +0x08  Node* next          — 12 bytes, heap-allocated per entry
```

`GameActorManager::Add` is `./kit fn 0x0021a8d0`. It does three things: appends a node for the actor to
the master list at +0x30; looks up (or creates) the per-type group whose key is the u32 at **actor+0x00**
and appends the actor there too; and stores the manager back-pointer into **actor+0x1c**. Appends go on
the tail, so list order is registration order and is stable for a given level load. The list helpers are
`./kit fn 0x0021b120` / `0x0021b2c0` (append) and `./kit fn 0x0021afe0` (find). Constructor and destructor
are `0x0021a780` / `0x0021a7d0`.

`GamePlayerManager::Add` (`./kit fn 0x002459b0`) is the same machine with different keys: it groups on
**player+0x34**, writes the manager to **player+0x58**, and additionally links the player into the scene
node tree via `./kit fn 0x00132f50` — so players are reachable two ways and actors only one.

Walking to the sixth actor therefore reads, in full:

```
[0x004e1e98] -> +0x34 -> +0x34 (head) -> +0x8 ×5 (next) -> +0x0 (item)
```

### The actor record

Fields common to every entry, proved for all 9 entries in 5 separate frames:

```
actor +0x00  u32    type id: 1, 2 or 4 — the key the per-type groups use
actor +0x04  u32    flags (14 = 0xe on every live entry seen)
actor +0x1c  ptr    the GameActorManager that owns the entry (written by Add)
actor +0x20  ptr    the player this actor belongs to
actor +0x48  ptr    class vtable — this is what actually distinguishes the three kinds
```

The actor record is not a standalone object: it is a sub-object at **+0x400** inside its owning game
object, so `actor - 0x400` is the thing whose constructor and vtable identify it. The three kinds present
in a level, by the `operator new` size and `__FILE__`/`__LINE__` at the allocation site:

| type id | size   | allocated at     | constructor          | vtable     | note |
|---------|--------|------------------|----------------------|------------|------|
| 1       | 0x1080 | MXWorld.cpp:5272 | `./kit fn 0x003deca0` | 0x0045ed50 | ctor takes the string at 0x0042ecb8, `"MXMonsterTruck"` |
| 2       | 0x14e0 | MXWorld.cpp:4738 | `./kit fn 0x00279ed0` | 0x00454ef0 | |
| 4       | 0x0c60 | MXWorld.cpp:4885 | `./kit fn 0x00318c30` | 0x00458f68 | the rideable vehicle |

A one-player Freestyle session on Montauk Plains holds **9 actors in 3 groups**: one type 1, four type 2,
four type 4, registered in that order. The counts are per level and per mode; nothing here is a constant.

### The type-4 (vehicle) dynamic fields

These are the ones that move, measured on the first type-4 actor in the list — an AI-owned vehicle, which
is why nothing needs a pad held down to see it change:

```
+0x00d0  f32x4    rigid-body point 0  (x, y, z, 1.0)
+0x00e0  f32x4    rigid-body point 1
+0x00f0  f32x4    rigid-body point 2
+0x0100  f32x4    rigid-body point 3
+0x0130  f32x4    velocity (x, y, z, 1.0) — w is 1.0, i.e. stored in a point-shaped lane
+0x0180  mat4     world transform, row-major; rows 0..2 orthonormal to ~1e-4, w lanes 0,0,0,1
+0x01b0    = row 3, the world position
```

Type 1 and type 2 actors have unrelated data at those offsets; only the shared fields above are known to
be common.

There are other floats in the record that move every frame and that I could not name — `+0x00b0` is the
clearest: it changes on three of the four vehicles, stays exactly 0.0 on the fourth, ranges over 0..670
and is neither the speed nor the distance travelled (its change per tick is not proportional to
`|Δposition|`: the ratio runs from -0.37 to 2.64 across the five frames). Listed below rather than guessed
at.

### Position is integrated with a fixed 1/24 s step

The frame gives the simulation `dt = 0.0333300` (1/30; `./kit fn` for the dt pipeline is in the R3 entry,
app+0x344 = 0x00479cd4), and the tick rate really is 30 Hz: the app frame counter at 0x00479d04 advances
~31/s, the vsync counter advances exactly 2 per tick, and the 30 fps cap flag at 0x0046a808 reads 1.

But the position does not move by `vel * dt`. Over the 119 single-tick steps of the capture, a least
squares fit of `Δposition = vel / C` gives:

| velocity sampled | C | 1/C | rms residual |
|---|---|---|---|
| start of tick | 24.078 | 0.041532 s | 0.0554 units/tick |
| **end of tick** | **23.996** | **0.041674 s** | **0.0114 units/tick** |
| midpoint | 24.020 | 0.041632 s | 0.0317 units/tick |

So position advances by the **post-update** velocity times a fixed **1/24 s** (0.0416667; the fit is
0.041674, 0.03% off), against displacements of ~1.5 units/tick — semi-implicit Euler with a hard-coded
step, not the frame dt. Equivalently: `|Δpos| / dt` is 1.250× the stored velocity, and that 1.25 is
30.003/24, not a unit error. Two readings fit the numbers equally well and I could not separate them
without a second frame rate to test at: either +0x130 is units/second and the integrator uses a constant
1/24 s step, or +0x130 is units per 1/24 s and the integrator uses a plain `pos += vel`. A port must get
this right either way, because both readings say the vehicle moves at a rate the frame dt does not enter.

## How it was proved

Two independent passes, both from a real running game (Freestyle → Montauk Plains → 1 player, 125cc, Race
Physics, Monster Truck → Start Event, reached unattended from the slot-9 main-menu state of C3):

1. **Five save states, ~62 ticks apart**, extracted to EE RAM images with `./kit state ram` and diffed
   field by field. This is what established the chain and the offsets: for every one of the 9 actors in
   every one of the 5 frames, +0x00 is 1/2/4, +0x1c is the manager address, +0x20 is a player address,
   the type-4 transform rows stay orthonormal to ~1e-4, and the counts at manager+0x30 / +0x3c stay 9/3.
2. **A 120-tick live capture** of the whole documented chain, re-resolved every frame:
   `./kit record watch/actor-table.toml --frames 120` → `context/capture/r6-actors.csv` (git-ignored, with
   its `.json` provenance sidecar). Ticks 22498..22617, **0 torn reads**, and the `tick` column advances
   by exactly 1 on every row — so every arrow in the spec resolved on every frame. `dt` is
   0.033330000936985016 on all 120 rows; `actors` = 9, `groups` = 3, `a5_type` = 4, `a5_flags` = 14,
   `a5_manager` = 0x00c43770, `a5_player` = 0x00f3a3a0 on all 120 rows, while the four body points, the
   velocity and the transform change continuously. That file is also what the integration fit above was
   computed from.

The task's test line — "a dumped table from a state parses with the documented stride" — is the watch
spec: `watch/actor-table.toml` *is* the documented chain and stride, the capture is a dump of the table
through it, and a wrong link anywhere gives empty cells rather than plausible numbers (the `actors` field
is `required = true`, so a null singleton ends the capture instead of logging zeros).

Three names went into `[symbols]` (`./kit sym`) and are mirrored in `research/oracle.txt`, which is the
committed copy: `game_manager` 0x4e1e98, `frame_counter` 0x479d04, `frame_dt` 0x479cd4. The watch spec
uses only those names, no raw addresses, so `./kit audit` stays clean.

`docs/CONTEXT.md` is named on the task line but I left it alone: it is the page about context economy, and
nothing I learned here changes any of its advice. R6's findings belong in this entry and in
`research/oracle.txt`, which is where that page itself says addresses and measurements go.

## Not verified / not 1:1

- **Which of the two readings of +0x130 is right** — units/second with a fixed 1/24 s integration step, or
  units per 1/24 s with a bare `pos += vel`. The measurement cannot tell them apart; separating them needs
  either the actual integrator read out of the decompilation (I did not find the function that writes
  +0x1b0) or a run at a different frame rate. The *numbers* above hold either way.
- **The integration step is 1/24 s to 0.03%, not bit-exactly.** It is a fit over 119 ticks of a vehicle
  whose velocity changes within each tick, not a constant read out of the image. I did not locate
  0x3d2aaaab (1/24f) in the vehicle code to confirm it; the kit has no constant search and I was not going
  to disassemble blind.
- **`+0x00b0` and the other moving scalars in the vehicle record are unidentified.** `+0xb0` is ruled out
  as speed and as an odometer by the measurement above; what it is, I do not know.
- **No `state` field identified.** The task line asks for "position, velocity, state, type". Type (+0x00),
  position (+0x1b0) and velocity (+0x130) are proved; +0x04 is a flags word that read 14 on every live
  actor in every frame, so nothing distinguished it, and no other scalar in the first 0x400 bytes behaved
  like a state machine over the frames I had. A capture across a crash/respawn would be the way to find it.
- **Only type-4 dynamic fields are mapped.** Types 1 and 2 share the five common fields and nothing more;
  their own layouts were not chased, and the offsets above are explicitly *not* valid for them.
- **One mode, one track, one player.** Everything was measured in a single-player Freestyle session on
  Montauk Plains. The 9 actors / 3 groups / registration order are observations of that session, not
  properties of the engine; a race or a second player will differ, and the per-type group *keys* (1, 2, 4)
  are the only part I would expect to be stable.
- **The third manager at GameManager+0x38 and the blocks at +0x3c/+0x40** were sized but not identified.
- **The 30 Hz tick rate is a wall-clock observation**, not a decompiled proof: counter deltas over sampled
  seconds plus the cap flag at 0x0046a808 reading 1. R3 left the gsctx+0xc question open and this does not
  close it, it only says what the running game does.
- **`./kit probe vsync --save` wrote `vsync_addr = "0x47a9d8"` into `kit.toml`** (60.41 Hz). `kit.toml` is
  git-ignored and per-worktree so nothing is shared, but the vsync counter is C2's territory and I took it
  on trust because `./kit pine watch` refuses to run without one.
- **A kit quirk worth knowing:** `./kit sym <name> 0x004e1e98` fails with "no `[symbols]` entry named …"
  while `./kit sym <name> 0x4e1e98` succeeds — the add path appears to re-parse the address as a lookup
  when it has leading zeros. It also failed once on a well-formed add and succeeded on an immediate retry.
  All three symbols were verified present in `kit.toml` afterwards.
- **The in-game machine selector did not respond** to left/right on "Machine 1 : Monster Truck (Race)" in
  the Freestyle setup screen, through several press patterns. I did not chase it — the default machine was
  fine for this task — but it means the pad script cannot currently choose a vehicle, which some later
  task will want.

## Next

R6 is done. For whoever picks up the actors again, in the order I would do it:

1. Find the function that writes actor+0x1b0 (start from the type-4 ctor `./kit fn 0x00318c30` and its
   vtable 0x00458f68) and settle the 1/24 s question from the code.
2. Capture across a crash and a respawn to find the state field.
3. Map type 1 and type 2 the same way; the chain in `watch/actor-table.toml` only needs its `0x8` hop
   count changed to point at a different entry.

The emulator copy 1 is stopped. Slot 8 holds a Freestyle/Montauk Plains gameplay state a few seconds into
a run, which is the cheapest way back to a level with actors in it — it is in the emulator's savestate
directory, not the repo, so it survives only until someone overwrites slot 8.
