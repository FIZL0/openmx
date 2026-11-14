# 1 — untamed monster (PART)

Task MD81c. 2026-11-14.

## What I did

- Did the task's first step: diffed SLUS-21701's monster-truck step against SLUS-21104's statically, over
  the whole call tree the truck's vtable reaches (about 45 pairs). The verdicts are in
  `research/untamed-machines.md` ("SLUS-21701's car step against SLUS-21104's (MD81c)") and the addresses
  are in `research/oracle.txt # MD81c`.
- Wrote `research/md81c/fp.py`, a fingerprint blind to instruction order (MD81a's `sdiff.py` reports
  hoisted loads and branch-likely as differences), and `research/md81c/keys.py`, which diffs a loader's
  `.phy` key strings.
- Did not capture, port or pack: no Untamed monster-truck save state exists (copy 9's are a Free Ride
  bike), and picking the truck in Untamed's menus needs a .p2m2 tape that nobody has written yet (uinput is
  root-only). The task's test needs a capture, so I split it: MD160 (the capture) and MD161 (port the four
  live changes behind an Untamed switch, the pack, and the bit-exact replay), and blocked MD81c on them.
  I noted MD144 and MD92 about the body-integrate change.

## What the original does

- Same code, only moved fields, for most of the step: the truck's PreProcess, Force, aero, anti-slide,
  launch and PostProcess; the engine and shift; the tire step and clear; the wreck path; the body's linear
  half; the loaders (the same 239 + 6 + 4 keys and defaults). So the truck's `.phy` changes (Weight,
  GravityScale, FrictionZone1, `[Preload]` off) need no new code.
- What changed:
  - **Body integrate** (`./kit fn 0x004bc670 -p game` on an Untamed KIT_CONFIG, against
    `./kit fn 0x00496240 -o atv`): before `rates = d·rates + h·accel`, each angular damping lane `d`
    becomes `powf(d, 0.333/h)` when `abs(0.333/h − 1) > 0.1` (h ≤ 0 takes a rodata vector). With the
    truck's damping at 1, `powf(1, y) = 1`, so this is a no-op on a plain drive. It changes things once a
    collider or ragdoll lowers a lane.
  - **Input step**: it reads the controls block, adds "unbound → not pressed" guards, and changes the
    clutch and hand-brake tests (they part only on negative strengths).
  - **Car PostProcess**: the yaw clamp is done by compares (it parts only on a denormal limit).
  - **Preload**: the cue is posted while firing, as on the bike (MD93).
  - **Car update**: the hazard clock with the grounded word, as MD81b found (already ported as
    `handling::untamed::stuck`).
  - Three -0/denormal-only changes: the odometer clause, a zero force added after gravity, and the
    quaternion copy.
  - The class step is model posing only.

## How it was proved

Statically only: each pair matched by `fp.py`'s fingerprint (op multiset, float constants, small
immediates, call count), and every pair it flagged was read by eye in `./kit fn` / `./kit dis --limit
100000`. No oracle, capture or replay.

## Not verified / not 1:1

- **No capture and no replay**: the task's test (a captured truck drive replaying bit-exact) was not done.
  There is no Untamed truck state, and the menus aren't tape-driven yet (MD160, MD161).
- **No port**: the four live changes (body damping, input guards, yaw clamp form, preload cue) are not in
  the port. With the pack absent, every replay is SLUS-21104's, as before.
- **The verdicts are static**: "the same code" means the same fingerprint plus a read by eye, not an
  oracle. A rescheduled pair could still hide a reordered float sum that `fp.py` can't see. Only the pairs
  it flagged were read in full.
- **The body damping's effect on a real ride is unmeasured**: whether any Untamed ride lowers a lane below 1
  (colliders, the ragdoll, the wreck path) was not checked.
- **Untamed's powf** has fdlibm's shape but was not proved against `ps2_float`'s powf. SLUS-21104's
  0x0034ff40 is only a wrapper, so the fingerprint couldn't compare them.
- **The class step and the loaders' defaults** were only fingerprinted (the key strings were diffed, the
  default floats compared as constants).
- **carengine.aud** (volumes about halved) was not looked at beyond MD81's inventory.

## Next

MD160: pick the monster truck in Untamed's Free Ride by .p2m2 tape in copy 3 or 9, save a spawn state, and
record drives with C28's recorder at the MD81c offsets. Then MD161.
