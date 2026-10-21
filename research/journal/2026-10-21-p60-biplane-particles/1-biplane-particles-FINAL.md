# 1 — biplane particles: tire dust and prop wash

Task P60 (P58's follow-up). 2026-10-21.

## What I did

- `crates/openmx/src/particles.rs`:
  - Two new entry kinds:
    - `Kind::Dust(DustKeys)`: the three tire dusts.
    - `Kind::HelicopterDust { begin, end }`: the prop wash.
  - `PlaneFx::new` now reads both. It also reads `mxparticlezones.dat` (`ZONES`) into each entry's 16 `zones`.
  - Every entry keeps `AttachedToTire`, and answers to that tire's flags as the manager does.
  - `Emt` gains `MatchTerrainContour`. `Source::emit_over` and `PlaneFx::step_over` take a ground query. `emit` and `step` remain as before, with no ground.
- `crates/game-render/src/biplane.rs`: `Emitter` keeps every key of its section (`keys`, `key()`), so a type can read its own keys.
- `crates/openmx/src/main.rs`:
  - `plane_fx_frame` passes the flight's three tires: `below`, `held`, `compression`, `surface`.
  - The fx steps over `world.place`'s terrain query (`plane_fx_ground`).
- `crates/openmx/tests/biplane_particles.rs`:
  - The synthetic list gains a tire dust, a wash and a zones file.
  - Four new tests: contact and compression, the burst, zones, and the wash's fade and terrain match.
  - The disc test now checks all nine entries, their keys and zones, and rolls all three tires and the prop over flat ground.
- `research/oracle.txt`: a P60 block.

Nothing was taken from opentennis: it has no particle manager.

## What the original does

The `.dat` reader (`./kit fn 0x003d52b0`) picks the class by `Type`:

| Type | constructor | init |
|---|---|---|
| Exhaust | 0x003d5e00 | 0x003d5e40 |
| Dust | 0x003d6020 | 0x003d6100 |
| Spray | 0x003d6710 | 0x003d67d0 |
| Generic | 0x003d6e00 | 0x003d6e60 |
| HelicopterDust | 0x003d7110 | 0x003d7190 |
| TireTrack | 0x003d7c40 | — |
| CollisionModel | 0x003d7370 | — |
| ParticleCollisionConstraint | 0x003d77c0 | — |

- `AttachedToTire` points the entry at the plane's tire record: 3 records of 0x2c bytes, set up in `0x00354750`.
- `ZoneDescriptor` (default `Default`) selects a section of `mxparticlezones.dat`. The file is looked up in the track folder first, then in `Tracks\`.

The disc's entries:

- **Tire dust:**
  - `AttachedToTire` 0, 1, 2 and `MinCompressionThreshold = 0.0`, ZoneDescriptor `Dust`.
  - Disabled by `ObjectContact | Wrecking`.
  - `biplanetiredust.emt`: 100–250/s, gate 15 ft/s, group Dust 1.
- **Prop wash (`[Dust]`):**
  - HelicopterDust on `PropDummy`, `BeginFadeHeight 20`, `EndFadeHeight 25`, disabled by `Wrecking`.
  - `Biplanedust.emt`: 35–45/s, no gate, extent y 1..3, `MatchTerrainContour=T`.
- **`mxparticlezones.dat`:** its `[Dust]` section is empty. Every zone therefore falls back to the `.emt`'s group.

How it runs:

- **The manager** (`0x003d5aa0`), per tire:
  - flags NoContact (neither contact), GroundContact (record +8 → plane tire +0x80), ObjectContact (+0xc → +0x84);
  - a zone change (+0x24 → the tire's +0xb0 record byte) calls each attached entry's vt +8.
  - An entry is on unless `Disable & (tire flags)` or `Disable & (Wrecking/NotWrecking)`. Switching it off (`0x003d87e0`) also clears the dust's latch.
- **Dust update** (`0x003d64e0`):
  - Latch: a ground contact sets it, an object contact clears it, neither leaves it as it was.
  - It is off unless latched and the compression c > `MinCompressionThreshold`.
  - Rate = file × (`ModifierBase + ModifierScale·c`).
  - **Burst** (`UseBurstDust`, c > 0.85):
    - the rate factor gets another `BurstModifierScale·c`;
    - growth min ×`GrowthMultiplier` (span `+0x350` not recomputed);
    - noise min.x and max.x ×`VelocityMultiplier`;
    - noise max.y = the file's max.z × `VelocityMultiplier`. That is the original's own field mix-up, ported as written.
  - **No burst:** growth and noise go back to the file's values.
- **Zone handler** (`0x003d63f0`, vt +0x1c/+0x20 disassembled at 0x003d6460/0x003d6480):
  - `none` sets entry flag 2 and turns the emitter off. The Dust update then switches the emitter on again directly, so `none` does not stop tire dust.
  - Otherwise it sets the zone's group.
- **HelicopterDust update** (`0x003d7210`):
  - `ground_query` (0x001dc960) under the emitter's point gives the height and the zone.
  - Zone `none` → off.
  - h < Begin → the file's rate; h < End → rate × (1 − (h−B)/(E−B)); otherwise off.
- **MatchTerrainContour** (emitter +0x238, in `0x00150540`): `ground_query` on the particle's start point, then + the turned extent's y.

## How it was proved

- **Rules, against the decompilation above.** `./kit check -- -p openmx --test biplane_particles`: 13 tests pass. The new ones:
  - `the_tire_dust_rides_its_tires_contact_and_compression`:
    - the threshold is strict;
    - the rate is ×(0.5+5c);
    - the latch holds through no-contact frames;
    - an object contact unlatches it;
    - ObjectContact and Wrecking switch it off;
    - a missing tire means no dust.
  - `the_tire_dust_bursts_past_the_burst_compression`: the burst's rate, growth (span kept) and noise, including the max.y = max.z quirk; the restore; UseBurstDust off.
  - `a_zone_picks_the_dusts_group`: Zone2/none/default, `[Default]`, an unknown group, a missing file. Tire dust on a `none` zone still emits.
  - `the_prop_wash_fades_with_height`: full rate, half rate at 22.5 ft, none at 25 ft, when wrecked, or with no ground. Particles start at ground + 2. Zone group and zone none.
- **The disc's data.** `the_discs_biplane_particles` reads free06's list, `.emt`s and zones file:
  - the nine entries, their tires, the keys, the gates, the rates, MatchTerrainContour, the 16 zones;
  - then a roll on flat ground where all four new entries emit and the wash stays on the ground.
  - The disc opened in this slot: a throwaway dump test over the same `Disc::open_configured` printed the files, then was deleted. So this test runs rather than skipping.
- **Live**, headless:
  - Command: `openmx --vehicle biplane --headless --play <tape>` with Cross held from tick 30, 450 ticks.
  - Per-second counts:
    - wash 41–63 while parked and rolling;
    - tire dust from 3 s (past the 15 ft/s gate): 42/35/35, then 99/101/52;
    - the rear stops at 5 s (tail up), the fronts by 7 s (airborne);
    - the wash fades out at 8–9 s as the plane climbs past 25 ft.
  - A shot at tick 110 shows a faint dust haze under the wings on the roll.

## Not verified / not 1:1

- **No comparison against the original's picture or counts.**
  - S57's capture video (`s5/context/s57/run1.mp4`) could not be read: the context guard refuses the file by name.
  - No PINE capture of particle counts was made.
  - The rates, gates and fades are the decompilation's and the disc's. Their look is checked only in the port's own shot.
- **The tire's zone is `Tire::surface`** (the port's friction-surface byte from the terrain query). The original reads the first byte of the tire's ground-query record (+0xb0), which I did not prove is the same byte. It changes nothing on this disc, because every zone of `Dust` is the `.emt`'s group.
- **Zones are applied every frame, not only on a change.** The original calls the zone handler only when the tire's zone differs from the last one it saw (+0x29, whose initial value I did not read). With a file that names zones, the first frame could differ. On this disc, none.
- **The Exhaust entries' zones are not wired.** They sit on tires 0 and 1, so a zone change would reach their handler. Their handler (Exhaust vtable) was not read, and the disc's `Exhaust` section is empty too. They do answer to their tire's flags (Disable = Wrecking only, so no change).
- **A zone byte of 16 or more** keeps the current group. The original indexes past its 16-byte table into other fields.
- **The prop wash with no ground under it is off.** Off the terrain, the port's query answers its no-ground depth (−1e6). I did not read what the original's `ground_query` leaves in the point when nothing is below.
- **Order within a frame:** the manager reads the emitter's point as this frame's node position. The original's emitter point (+0x40) may still be last frame's when the manager runs. Not checked.
- **`EmitterOffset`** (none on the disc) is applied as an offset in the node's frame. That reading is assumed from where it is passed, not proved.
- **The tire record's object contact is the plane tire's +0x84**, which is only ever cleared (S39). So ObjectContact never happens on the plane's tires, in the port or, as far as read, in the original.
- **Burst never seen live.** I did not check whether the plane's strut ever passes 0.85 ft; StrutCompression is 1.0, so a hard landing could reach it. Only the unit test covers the burst.
- **Wind and terrain collision of the particles** are still P61's. The `.emt`s set `Wind=T`.
- **The prop wash raises dust while parked with the engine idle.** The code has no gate on the throttle and the `.emt` has no speed gate. That follows the decompilation; I did not see it in the original.

## Next

Nothing in P60. P61 (wind, terrain collision) and P63 (the wreck dust's contacts) remain.
