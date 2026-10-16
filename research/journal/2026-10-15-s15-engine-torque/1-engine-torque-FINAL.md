# 1 — engine torque (FINAL)

Task S15. 2026-10-15.

## What I did

Ported **the engine** — everything `engine_update` (`0x00246fd0`) does — into
`crates/game-sim/src/move.rs`, and proved it against the original in `crates/game-sim/tests/move.rs`.

That is the three things the brief asks for, and one more that came with them:

| the brief | here |
|---|---|
| the curve itself | `curve_seek` and `curve_value`, the original's own cursor over a table of points |
| the clutch's slip | `engine_update`'s middle third: the static friction that holds, the kinetic friction that slips, and the crossing test that locks it back up |
| the torque that reaches the driven wheel | `engine_update`'s last third: through the overall ratio, added into the wheel's torque accumulator |
| (and) where the revs go next | the crank's own integration against `EngineInertia`, clamped between `MinRPM` and the rev limit |

`Call::EngineUpdate` is **gone**, and so is its hook. A hook *replaces* a function in the oracle — the
dispatcher sets `pc = $ra` and carries on — so "record the call and then run the original" is not a
thing that can be arranged. With the body ported there is nothing left to record: `mv::step` now calls
`engine_update` for real, in the same place the variant used to be pushed, and both step proofs run the
original's own `engine_update` inside `bike_ground_step`. So the engine's revs, its torque, the
cursor's index and the driven wheel's torque accumulator are now part of the compared object graph in
those two proofs as well, carried from substep to substep and frame to frame.

The port can now accelerate a bike. It could not before: `clutch`, `shift` and `engine_post` all shape
a torque that nothing was producing.

## What the original does

`./kit fn 0x00246fd0` — 100 instructions, two callees, no loops. `./kit dis 0x00246fd0 --len 648` is
the whole of it; everything below was read off that, because the decompiler reorders multiplies and
this is a bit-exact port.

**The curve, and its cursor.** Every table in this game is read through a four-word cursor —
`{count, index, frac, xs*}` — that remembers where it was last time. `curve_seek` (`0x0013e6f0`) walks
it: up while the input is still at or above the point the index sits on, then down while it is below
the point beneath it, and then the weight is `(xs[i] - x) / (xs[i] - xs[i-1])`, or 0 on the bottom
point. The weight runs *backwards* — it is the distance down from the point at `index`, not up from
the one below — which is why `curve_value` (`0x0013e7c0`) reads `ys[i] - frac * (ys[i] - ys[i-1])`.
Off the top of the table the cursor parks on the last point with a weight of 0, so a curve is flat
past its end rather than extrapolated. Both bounds tests are unsigned.

Keeping the table out of the cursor is what makes the second read free: the engine seeks once on the
RPM axis and could then read any number of curves at that place. It reads one.

**The engine object**, 0xc0 bytes, hanging off the bike at `+0x7b8`. The `.phy` key names come from
the ELF's string table at `0x332a20` and the setters that consume them (`./kit fn 0x00246ce0`,
`0x00246e80`, `0x00246f20`):

| off | what |
|---|---|
| `+0x30` / `+0x34` | pointers to the *rate-limited* throttle and clutch, `bike+0x658` / `bike+0x65c` |
| `+0x38` / `+0x3c` | `IdleThrottle` / `EngineBraking` |
| `+0x40` | `FinalGearRatio` |
| `+0x44` / `+0x48` | `EngineInertia` and its reciprocal |
| `+0x50` / `+0x54` | `ClutchStaticFriction` / `ClutchKineticFriction` |
| `+0x60` | `MinRPM` |
| `+0x64` / `+0x68` | the clutch-locked flag / the gear |
| `+0x6c` / `+0x70` / `+0x74` | `NumberOfRPMPoints`, the RPM array, the torque array |
| `+0x78` / `+0x7c` | `NumberOfGears` / the gear-ratio array |
| `+0x80` / `+0x84` | the gear's overall ratio and its reciprocal |
| `+0x88` | the clutch's launch multiplier, which `bike_clutch` writes (S3) |
| `+0x8c`..`+0x98` | the curve's cursor: count, index, weight, and a third pointer to the RPM array |
| `+0x9c` | the driven wheel — `bike+0x7b4`, the rear, set at `0x00253140` |
| `+0xa0` / `+0xa4` | the engine's torque / the torque to the wheel |
| `+0xa8` / `+0xac` | the revs / the revs the road is asking for |
| `+0xb0` / `+0xb8` | the revs at peak torque / the peak torque |
| `+0xb4` | the rev limit |
| `+0xbc` | a scale on the torque leaving the gearbox |

The file gives horsepower against RPM (`HP%d`, `RPM%d`) and the loader turns each pair into a torque
with `5252 * hp / rpm`, so what is in `+0x74` is already torque. A point with `rpm <= 0` is *skipped*,
not zeroed. The rev limit is the last point's RPM, so the engine is never asked for a torque past the
end of its own table — which is also why the port's proof sets it that way.

**The body**, in exact order. `f21` holds the frame's `dt`, `f20` the torque reaching the gearbox:

```
seek the cursor to the revs; curve = the torque there
brake  = rpm * (EngineBraking * rpm)                  # mul.s f4,f4,f5 then mul.s f4,f5,f4
thr    = min(throttle + IdleThrottle, 1.0)            # added then capped; never floored
+0xa0  = launch * ((curve + brake) * thr - brake)
was    = +0xac                                        # read before it is overwritten
+0xac  = (9.549296 * wheel.spin) * ratio
if locked:
    if clutch == 1.0 or not (|+0xa0| <= static * (1 - clutch)):  locked = 0
    else:                                                        shaft = +0xa0
if not locked:
    sign  = -1 if (rpm - +0xac) < 0 else 1            # built as an int, then cvt.s.w
    shaft = (kinetic * (1 - clutch)) * sign
    if clutch < 1.0 and (was < rpm < +0xac or +0xac < rpm < was):
        locked = 1 ; shaft = +0xa0
+0xa4  = +0xbc * (shaft * ratio)
wheel+0x210 += +0xa4                                  # 0x0025c380, an accumulator
if locked:  +0xa8 = +0xac
else:
    load = shaft
    if wheel+0xe0:  load = invRatio * (radius * ((1 - clutch) * wheel+0x858))
    +0xa8 = +0xa8 + 9.549296 * ((invInertia * (+0xa0 - load)) * dt)
if   +0xa8 < MinRPM:     +0xa8 = MinRPM
elif revLimit < +0xa8:   +0xa8 = revLimit
```

**Five things worth the ink.**

- The engine's braking is subtracted **twice**, once inside the throttle's blend and once outside it.
  So a shut throttle leaves exactly `-brake`, a pinned one leaves the curve, and the throttle fades
  between the two. It is not "curve times throttle, minus braking".
- `IdleThrottle` is *added* to the rider's throttle and the sum capped at 1. It is never floored, so
  it is an offset and not a minimum — a rider with the throttle shut still gets `IdleThrottle`'s worth
  of the curve.
- The clutch locks up on the revs **crossing** the road's revs, not on matching them, which is why the
  previous substep's `+0xac` has to survive into this one. Both directions of the crossing count.
- A clutch that breaks away does it in the **same** substep it is tested in: the static test unlocks
  the flag and then falls straight into the slipping branch, which writes the kinetic torque over the
  top. There is no substep of neither.
- The `MinRPM`/limit clamp applies to the **locked** path too, so an engine being driven over its
  limit by the road is caught here as well as by `engine_rev_limit` after the wheels integrate.

Two multiply orders are not the ones a decompiler prints, and both show up as a last-bit difference:
`rpm * (EngineBraking * rpm)` and `9.549296 * ((invInertia * (torque - load)) * dt)`.

**S3's note about `f20` was wrong**, and `research/oracle.txt` now says so. S3 recorded that
`engine_update` reads `f20` before writing it and so picks up the 0.0 the ground step leaves there.
It does not: every path through the function writes `f20` first. Two of those writes sit in **branch
delay slots** (`0x002470b0` and `0x00247124`), which is exactly what makes Ghidra render the register
as `unaff_f20` and the read as uninitialised. The initial value is never observable, and a port that
modelled it as an input would have an argument nothing can set.

## How it was proved

`crates/game-sim/tests/move.rs`, one new proof and two existing ones that got stronger.

| proof | against | n |
|---|---|---|
| **the engine** | `engine_update` `0x00246fd0` | 10,000 generated states |
| the whole ground step | `bike_ground_step` `0x00257a00`, now running the real `engine_update` | 2,000 generated frames |
| a 240-frame scripted ride | the same, stepped 240 times in one piece of memory | 1 fixture |

The new proof works the way the other six do: lay the bike, its chassis, its two wheels, its engine
and its collision record out in the interpreter's RAM; write a generated state in; run the original;
write the *same* state in again and apply the port over the top; compare **every word** of all six
regions. A field the original writes and the port does not model is a difference, not an omission —
which is how `+0x6c` (the point count the loader keeps twice) and the cursor's own pointer got checked
rather than assumed. `written_ranges()` on top of that: a write outside the object graph fails.

The generator had to aim at one thing. The clutch locks up when the revs *cross* the road's revs, and
two unrelated random numbers almost never straddle a third, so half the states are built backwards:
the road's revs are placed a step either side of where this substep's wheel speed puts them, and the
engine's revs somewhere along that step — or, one time in six, just past the end of it, which is the
near miss that does not lock. Both directions of the crossing fall out of the same three draws.

Twelve paths are tallied and each asserted to have been reached at least 100 times: below the curve,
on it and past its top; the load on the crank coming from the clutch and from the tire; the clutch
staying locked, breaking away, locking up and staying slipped; and the revs ending at the idle floor,
at the rev limit and between the two. The clutch is generated as a *control* (exactly 0, exactly 1, or
between) because the original tests it against both by equality — one of those tests is the whole
`clutch == 1.0` break-away path.

`./kit check`: **381 tests**, all passing. No emulator was needed, so none was started.

## Not verified / not 1:1

- **`+0xbc`, the scale on the torque leaving the gearbox.** The engine's constructor sets it to 1.0
  (`0x00246c18`) and no setter for it was found: nothing in `bike_phy_load`'s key list writes it, and
  no other writer turned up. It is modelled as `Engine::torque_scale` and the proof *varies* it, so
  the port is proved to read it rather than to assume it — but whether anything in the game ever
  changes it from 1.0 is unknown. If something does, it is a feature this port already handles; if
  nothing does, it is a field that could be deleted.
- **`MAX_RPM_POINTS = 16` is the port's capacity, not the game's limit.** The original allocates
  exactly `NumberOfRPMPoints` floats at load time, so the real bound is whatever the `.phy` files ask
  for. `context/res` and `context/psr` are empty in this worktree and the `.phy` members inside
  `MX.PSR` are compressed, so the key's actual value could not be read off the disc. Sixteen is a
  guess with room in it; nothing reads past `Curve::points`, so a bike with more points would
  overflow the array rather than read garbage. Reading one real `bike*.phy` would settle it.
- **A curve with no points at all.** `curve_seek` then parks the index at `points - 1` *wrapped*, and
  the original reads off the end of its own heap allocation; the port panics instead. Unreachable
  either way — the loader takes the rev limit from the last point, so a zero-point curve is already
  broken before this function sees it — but it is a difference.
- **`bike_phy_load`'s engine keys** (the table above) are recovered by reading the setters and the
  string table, not by running the loader. Same standing as S3's `Params` map: the proofs write the
  engine's fields directly.
- **Wheel `+0xe0` and `+0x858`** are named from their readers, not their writers:
  `Wheel::spin_from_torque` and `Wheel::contact_force_fwd`. What sets them is `wheel_integrate` and
  the tire forces, which are S16's. They are inputs here, generated on both sides.
- **The engine's peak (`+0xb0`, `+0xb8`)** is not modelled. `engine_set_power_curve` writes it and
  `engine_update` does not read or write it; the proof leaves both words zero on both sides, so
  "nothing in this function touches them" is proved at zero and by reading, not across values.
- **`engine_update`'s position in the substep loop** is no longer proved by this file. S3 proved it
  with the callee hooked and the call recorded in `mv::step`'s ordered list (commit `42847ff`); now
  the call is real, the two step proofs cover it only as part of the whole graph — which is a stronger
  test of the *arithmetic* and a weaker one of the *position*, because a call moved one line up or
  down inside the loop would still have to produce a different graph to be caught. It does, for every
  neighbour it has (the drag reads the chassis the engine does not touch, but the engine's own revs
  feed `engine_post` later in the same substep), so this is a note and not a hole.
- **`NumberOfGears`/`+0x78` and the gear array** are S3's, unchanged.

## Next

The engine is done; S16 (the wheel: suspension ray, tire forces, integration) is the next one, and it
is the task that gives `Fresh`'s ten fields their names and `Wheel::spin_from_torque` and
`Wheel::contact_force_fwd` their writers. It also owns `wheel+0x210`, the accumulator this task adds
into: the wheel's integration is what spends it and clears it, and until that is ported the
accumulator grows all frame.

One cheap thing for whoever has the disc mounted: read `NumberOfRPMPoints` out of a real `bike*.phy`
and either confirm `MAX_RPM_POINTS = 16` or shrink it.
