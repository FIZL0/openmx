# 1 — bike modifiers that show (FINAL)

Task P97. 2026-10-22.

## What I did

I took nothing from opentennis: it has no game modifiers.

I found three things holding the speed knobs back, by riding the disc's `race125p.phy` flat out on an
endless flat floor (the new `crates/game-sim/tests/modifiers.rs`) and on Free09's x = 4620 straight
(`openmx --track free09 --trace 600 --trace-gas --trace-feel --trace-at 4620,2320,0 --modifiers …`):

1. **The rear tire caps the launch.** `tire::force` swaps the drive for `SkidDriveForce × load`
   once torque/radius passes `MaxDriveForce × load`. The stock 125 already spins its tire off the
   line, so extra torque only spun it harder. Engine power 200% reached 40 ft/s in 0.87 s against
   0.97 s stock, and 500% did no better than 200% up to tick 30. Tyre grip 200% alone was quicker
   than Engine power 500%.
2. **The integrator's 135 ft/s limit capped Engine power.** `Mods::ride` only raised it for Bike top
   speed. Power 200% and 500% both stopped at exactly 135.000.
3. **A put-back reset the limit to 135 ft/s.** Every wreck's put-back, a respawn and a teleport onto
   another player went through `off_map::unwreck`, which writes the original's 135². So Bike top
   speed was a 135 ft/s bike (stock is 120) from the first wreck or reset on. This is probably most
   of "the bike modifiers don't work" in a real freeride, where a player resets often.

None of the others suspected in the brief (the clutch, the wheelie limit, the rev limiter) held
anything back. The user's free06 numbers come from the start line's terrain: the first ~5 s never
pass ~60 ft/s whatever the bike.

Changes:

- `Ride::riding_speed_sq` (ride.rs) is the limit a put-back gives back. It is built at the
  original's 135², and `Ride::unwreck` replaces the four `off_map::unwreck(&mut ride.chassis)` calls
  in rider.rs. It is on the wire (snapshot.rs), so a drop-in peer gets it. It is left out of the hash
  walk: it is a constant of the ride, like S73's arm, and the chassis' walked `max_speed_sq` is what
  it lands in. At stock it is bit for bit the old value, so no fixture hash moved.
- `Mods::ride` raises both limits by k² for Bike top speed **and** Engine power above 100% (the
  product when both are set).
- **Engine power** above 100%: the rear tire's `MaxDriveForce` and `SkidDriveForce` × k, so the extra
  torque reaches the ground. At 100% or below, the file's grip is kept.
- **Bike acceleration** (P95's knob) above 100%: the same drive-grip scaling. Without it, its 200% was
  the same 0.87 s to 40 as power's.
- **Bike top speed**: every tire surface's `WheelDrag` / k², next to the air's `Drag` / k².
  race125p's rear has a v² wheel drag of 0.1 on surface 2, four times the air's 0.025.
- The menu's blurbs and field lists for the three knobs say what they now do.
- Pinned numbers moved, each with its reason in a comment: `size_of::<Sim>` 11160 → 11168
  (tests/hash.rs) and `size_of::<Ride>` 2416 → 2424 (tests/ride.rs). The riders' resync bytes
  (tests/platforms.rs `RIDERS_WIRE_DIGEST` / `RIDERS_WIRE_LEN`, 13303 → 13319: 4 bytes × 4 riders)
  moved too. All per-tick hashes held.

### Measured, disc's race125p, flat floor, 30 Hz

| knob | 0–40 ft/s | 0–100 ft/s | top |
|---|---|---|---|
| stock | 0.97 s | 4.30 s | 120.0 |
| Bike top speed 150 / 200 / 300 / 50% | 0.97 s | 2.83 s (200%) | 180.0 / 240.0 / 359.6 / 60.0 |
| Engine power 200% (before → after) | 0.87 → **0.53 s** | 2.73 → **1.40 s** | 135.0 → **142.4** |
| Engine power 500% (before → after) | 0.73 → **0.37 s** | 2.60 → 1.20 s | 135.0 → 229.5 |
| Engine power 50% | 2.10 s | never | 93.4 |
| Bike acceleration 200% (before → after) | 0.87 → **0.53 s** | 3.27 → 1.67 s | 120.0 |
| Bike acceleration 50% | 1.97 s | | |

Free06's start line in the shell (`--trace 300 --trace-gas --trace-feel`), the brief's own case. Speed
at tick 30: 38.7 stock, **62.5** at power 200%, **94.7** at 300% (39.8 / 42.6 / 46.5 before).

The other bike knobs, each against the measure it is named for (50% / stock / 200%):

- Tyre grip, 1 / seconds to 40: 0.48 / 1.04 / 1.20.
- Suspension, height at rest + 1 ft: 0.08 / 0.53 / 0.79.
- Jump strength, peak height off THE_FLIP's preload: 26.4 / 31.1 / 45.7 ft.
- Flip spin, peak rad/s: 1.69 / 3.73 / 5.29.
- Air control, peak rad/s with the stick held: 0.58 / 1.16 / 2.32.
- Crash tolerance, by drop: an 80 ft drop wrecks stock and not 200%; a 40 ft drop wrecks 50% and not
  stock.
- Air drag, top speed: 135 (the limit) / 120 / 95.6.
- Gravity, speed after a 1 s fall: 16.1 / 32.1 / 63.6.

**The Settings screen's values reach the ride.** I checked this by reading the code (main.rs's menu
loop, `ride_modifiers`, menu.rs's modifiers screen):

- With no `--modifiers`, the menu's `settings.modifiers` becomes the ride's `args.modifiers`.
  `ride_modifiers` hands it on (or the host's room text online, or a tape's own), and the ride applies
  `mods.bike(i)` / `mods.ride(i)` to seat i.
- 1P sets seat 0, All sets every seat, and a preset copies the whole set.
- The online host's text goes in `room.modifiers` (menu.rs's lobby and main.rs's Open to Online), and
  every peer's seats ride 1P's numbers.

## What the original does

Nothing new: modifiers are additions, and at 100% none of them touches anything. The tire's drive
cap is `tire::force` (S58, `wheel_tire_drive_force` in research/oracle.txt). The 135 ft/s riding
limit and its put-back are `off_map.rs`'s (S59/S73). They are unchanged and are still the original's
path at stock.

## How it was proved

- `crates/game-sim/tests/modifiers.rs`, on the disc's `race125p.phy` and the `125_faisst` wheel nodes,
  ridden by the real `Rider::frame`. It skips without a disc. Five tests:
  - top speed ×0.9k to ×1.05k at 150/200/300% and about half at 50%, 0–40 unchanged;
  - Engine power 200% 0–40 < 0.7×, 0–100 < 0.5×, top > 138; 500% quicker again, no wreck, nose
    < 45°; 50% slower;
  - Bike acceleration 200% 0–40 < 0.7× with the top within 2%, 50% slower;
  - a respawn put-back keeps a 200% top speed / 300% power bike's limit, and the stock one at 135².
    This test fails with `Ride::unwreck`'s restore line taken out; I tried it;
  - every other bike knob moves its measure both ways (list above).
- P95's and P55's tests still pass on the stand-in bike. The replay and acceptance hashes held, so
  100% is still the original.

## Not verified / not 1:1

- **The menu → ride path was not driven live.** I checked it by reading the code only. Reason:
  keystrokes do not reach windows on this desktop, and a `--play` tape overrides the menu's modifiers
  with its own, so the menu cannot be driven unattended into a ride. `openmx --modifiers` and the
  ride code it reaches were measured.
- **Engine power and Bike acceleration above 100% also raise the rear tire's braking grip.**
  `MaxDriveForce` is also the cap the rear brake locks past. The rear brake locks later on those
  bikes. I did not separate it out, because the original has one number for both.
- **Bike top speed changes nothing off the line, by design** (same force at the wheel, taller
  gearing). On free06's start, which never passes ~60 ft/s in its first five seconds, it barely shows
  (tick 150: 62.4 at 200% against 58.3). It only shows on a long straight or a downhill. If the user
  wants it felt everywhere, that is Engine power's job, or a change of design for them to decide.
- **Air drag below 100% tops out at the integrator's 135 ft/s.** I left the original's limit for
  the world's knob. So 50% drag reads 135, not the drag-limited ~150, and 0% is a 135 ft/s bike.
- **Engine power's top speed is not a clean multiple.** 200% tops at 142 and 500% at 229: the
  curve's last torque (17.7 at 9500 rpm) holds past the rev limit, so the top is wherever drag meets
  it. I did not chase it, because the knob's job is the pull.
- I did not test the combined knobs (e.g. top speed and power both at 200%) beyond the limit's
  product.
- Cross-architecture: `RIDERS_WIRE_DIGEST` was regenerated on x86-64 only (the bytes are written
  little-endian, so other platforms should agree). I did not run tests/cross/platforms.sh.

## Next

Nothing for this task.
