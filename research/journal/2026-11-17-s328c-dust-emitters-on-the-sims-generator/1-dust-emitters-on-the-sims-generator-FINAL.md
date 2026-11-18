# 1 — dust emitters on the sims generator (FINAL)

Task S328c. 2026-11-17.

## What I did

- New `crates/game-sim/src/dust.rs`: the manager's frame and the emitters' draw count.
  - `Feed` (wreck, throttle, clutch, speed, two tyres' ground/object/spinning/side), `Entry` (kind,
    tyre, Disable, live, zone-off, emitter), `Kind` (Exhaust, Generic, Spray, Quiet), `Emitter`
    (on, just-on, MinimumEmissionSpeed, place on the bike, point, previous point).
  - `RiderDust::frame` runs the manager on the last frame's feed, then each emitter: new point from
    the bike's transform, nothing on a just-on frame or while off, else 2 draws if the point moved at
    least `MinimumEmissionSpeed` (F, the original's mula/madda/madd, sqrt, divide).
  - `Sim::dust: Option<Box<Dust>>`. `Sim::dust_frame(i)` runs in `step_all` right after each rider's
    `think` (and in `step` after the bike), drawing on the lane's generator, else the race's.
    Hashed as `dust` groups only when present; not on the wire (snapshot keeps the local one).
- `tests/hash.rs`: size pin 12296 → 12304.
- `tests/ai/draws.rs` (the original's whole frame in Ee, 63 ticks): hooks `emit` and checks every
  emitter's draws per tick against the new `C33_DUST` table (rider, entry, first, last tick), and that
  the dust site's draws are exactly two per emitter in it. Removed the temporary probe.
- `tests/ai/race4.rs` (the Sim): `c33_dust_seed` reads each rider's 14 entries, their emitters and the
  manager's feed from the image; each emitter's place on the bike is its image point in the seeded
  bike's frame (`dust::place_on`). Per tick the Sim's drawing entries must equal `C33_DUST` for each
  rider while its body agrees; from 17 to 28 the Sim's generator must step exactly 2 × that.
- Follow-ups: S420 (the shell's races build the dust and particles.rs places the Sim's counts), S421
  (the emitter points follow the suspension), S422 (foot dust and collision models).
- `crates/openmx/src/particles.rs` is unchanged: the shell's own speed gate and dice still place its
  particles. Moving it onto the Sim's counts needs the shell to build `Sim::dust` (S420).

## What the original does

- Manager frame `./kit fn 0x003d5aa0`. Each tyre record's flags: wreck 8 or 0x10, ground 2, object
  4, none 1. Each entry: allowed if its Disable masks neither its record's flags nor the wreck flag.
  Allowed and not live → enable (vt+0x14: live, emitter on unless zone-off). Not allowed and live →
  disable (vt+0x18: not live, emitter off; the spray also zeroes its clock). Then a live entry
  (flag exactly 1) runs its update (vt+0x10).
- Updates: exhaust `./kit fn 0x003d5f70` (f = throttle·0.5·(MaxSpeed − speed)/MaxSpeed, off at f ≤ 0);
  generic `./kit dis 0x003d70f0` (emitter on every frame); spray `./kit fn 0x003d6b50` (idle =
  no throttle or clutch or not spinning; idle and |side| ≤ threshold → the 0.25 s clock runs down,
  else clock 0.25 and on).
- The feed points into the bike's stunt-state snapshot. The snapshot is taken after the attachment
  walk that runs the manager, so the manager sees the frame before's. Without that lag the Sim's
  exhausts drew at 16, a tick before the original's.
- emit `./kit dis 0x00150540`: the gate (emit_timing `./kit fn 0x0014daa0`: point from the node,
  just-on → only the point), the other path (+0x23c), a node-model visibility check (C33's nodes have
  no model), speed = |pos − prev| / dt, nothing under +0x3c4, else two rands (0x0015070c, 0x00150744).
  Two draws per emitter per frame, however many particles.
- Order: each rider's dust draws come straight after its own AI draws (tick 34: Ai, 8 dust ×3), and
  before the notes (ticks 30, 48).
- C33's table: the exhausts (entry 5) of ai0/ai2 draw 17–23 and ai1's 21–28. The generic rear dust
  (6) of ai0/ai2 draws from 18 and ai1's from 22. The rear sprays (8, 10, 11) of ai0/ai2 draw from
  20 and ai1's from 24. ai0's sprays stop after 51. All of ai2's stop after 54. Front sprays,
  foot dust and the human's emitters never draw.

## How it was proved

- `tests/ai/draws.rs` `s328_c33s_other_draws_are_looks_notes_and_dust`: the original's frame in Ee
  over C33's 63 ticks. Every emitter's draws match `C33_DUST`, two each.
- `tests/ai/race4.rs` `s12e_free09s_four_racers_ride_as_the_capture`: the Sim against the same
  table for every rider through its body's agreement (ai0/ai1 28, ai2/human 63), and its generator's
  steps for ticks 17–28. The lanes' agreement (`C33_AGREES_THROUGH`) is unchanged.
- `./kit check --all`: everything green but `crates/openmx/tests/untamed_cars.rs`
  `the_untamed_baja_drives_in_the_shell_on_untameds_step`, which is equally red with this task's
  crates reverted to the base (the Baja never reaches the shell's body words). Not touched here.

## Not verified / not 1:1

- **Emitter points are a fixed place on the chassis.** The original's tyre emitters hang under
  SwingArm or Shocks, which the suspension moves. A parked bike's rear dust moves 0.3–2.2 ft/s. Near
  the 5 ft/s generic gate (or 3/1 for sprays) the Sim can flip a tick apart from the original. C33
  matched on every checked tick, but no emitter point was compared bit for bit. S421.
- **Foot dust (PCC) and CollisionModel entries are Quiet.** C33 never makes them draw (foot dust's
  gate is 30 ft/s; no wreck). Their real draws are not ported. S422.
- **TireTrack entries are Quiet.** I did not check whether they draw on the shared generator. None
  did in C33.
- **Not ported:** zone changes (an entry's zone-off bit is seeded and never updated), the pause byte,
  the view cull, DelayTime / EmitModes (all mode 0 on the disc), and the visibility stamp. emit_other's
  extra gates (a global and the system's +0x174/+0x58) are taken as passed; C33's entries 9/10 match
  that.
- **The feed's sources are the shell's choices, not proved field by field:** throttle `held.gas`,
  clutch `mv.clutch_in`, speed `chassis.speed`, wreck `crash.is_some()`, ground `ray_hit`, object
  `swept_contact`, spinning `slip.spinning`, side `side_per_load`. C33 checks them only as far as the
  draw table goes.
- **Generator order:** dust runs right after each rider's brain, as the original's sequence shows. The
  generator's values are not checked past 16, because the render draws (S328d) are missing.
- **Only C33 seeds it.** No live shell race builds `Sim::dust` yet (S420), so a live race draws nothing
  for the emitters, as before.

## Next

None for S328c. The Baja test's red is the base's (above).
