# 1 — all machines per player (FINAL)

Task P87. 2026-10-21.

## What I did

- **Listed every machine on the disc.** `machine.res` has a directory per machine, each with a `.phy`,
  collision and four models in the factory's skin order (`X`, `X2`, `X_ss`, `X2_ss`):
  | kind | machine | dir | models | `.phy` | camera presets |
  |---|---|---|---|---|---|
  | 1 | Biplane (MXBiplane) | `biplane\` | biplane, biplane2, biplane_ss, biplane2_ss (+ propconeblur/propconebent) | biplane.phy | AppStart\BiplaneCPresets.ini |
  | 2 | Kiowa helicopter (MXHelicopter.cpp) | `helicopter\` | kiowa, kiowa2, kiowa_ss, kiowa2_ss (+ bladeblur/bladebent, tailrotorblur/tailrotorbent) | kiowa.phy, wreckblade.phy | AppStart\HeloCPresets.ini |
  | 3 | Buggy (MXBuggy.cpp) | `buggy\` | buggy, buggy2, buggy_ss, buggy2_ss | buggy.phy | AppStart\BuggyCPresets.ini |
  | 4 | Monster truck (MXMonsterTruck.cpp, `actor_class_monstertruck`) | `monstertruck\` | monstertruck, monstertruck2, …_ss | monstertruck.phy | AppStart\MonsterTruckCPresets.ini |
  | 5 | Baja (MXBaja) | `baja\` | baja, baja2, baja_ss, baja2_ss | baja.phy | AppStart\FourWheelCPresets.ini |

  Plus `aud\carengine.aud`, `machine.aud`, `otherengine.aud`, `event.aud`. **Which track**: only the five
  freerides with a `tracks\<t>\machine.txt` in `machine.res` — free06, 07, 09, 10, 11 — and **every one of
  them parks two of every machine** (`[Biplane1]/[Biplane2]`, `[Helicopter1/2]`, `[Buggy1/2]`,
  `[Monster1/2]`, `[Baja1/2]`, the "Single Player Machines for Freeride" block), plus a `[MachineN]` block
  for the career machine race (the `Machine%d` path below). No other track has machines.
- **Liveries.** The `_ss` models are not more liveries: `biplane_ss.psg` binds `pitts_body01_ss` etc., the
  split-screen texture set of the same livery (`biplane2_ss` binds `pitts_body02_ss`). So the disc has two
  liveries per machine, skin 0 and skin 1. `vehicle::livery(copy) = copy % 2`.
- **`crates/openmx/src/vehicle.rs`**: `Machine` (the five, with kind number, section word, directory,
  models, spacing, and `kind()` = the shell's `Kind` where ported — the biplane only), `Spawn::machine`,
  `spots`, `park`, `Bay` and `Lot` (`new`, `resize`, `of`, `bay`, `plane`); `Plane.livery`, kept across the
  swap's reset. Copy `n` of every machine is player `n`'s; the swap only takes the player's own copy, so
  no two players are ever on one machine.
- **Parking past the file's two** (the port's rule, the original has no third player): copy `k` beside
  the file's spot `k % 2`, stepped across its `Direction` one `Machine::spacing` at a time (+1, −1, +2,
  −2, …) to the first place at least that far from every copy already parked; its height keeps the file
  spot's clearance over the dirt. No cap. A copy's place depends only on those before it, so a ride
  that grows (`Lot::resize`) keeps every machine where it was.
- **`crates/openmx/src/main.rs`**: the lot is built off the track's `machine.txt` and the terrain (a
  `Ground` probe made before the ride's `Place`) for `--players`; every seat gets `lot.plane(i, flight)`
  with its own `Flight` (and its own seat's modifiers, `mods.plane(i, …)`), its own plane camera (a copy of
  the first's presets), and starts in its plane when the setup picked it (P41's `settings.machines[i]`;
  before, every seat past the first was forced onto the bike). Both liveries' models are loaded and every
  seat's plane is drawn, GPU and software, in `plane_drawn[plane.livery]`, each interpolated from its own
  previous tick. The pause's per-seat "Switch To Machine" (P41) now offers every player their plane,
  because `switchable[i]` reads each seat's own garage. A line says what was parked:
  `machines: 4 biplane, 4 helicopter (not ported), … for 4 players`.
- menu.rs did not need a change: the pause row and the setup pick are already per seat (P41), and a
  machine that is not ported has nothing to offer in them yet.
- Filed S86 (helicopter), S87 (buggy), S88 (monster truck), S89 (baja) — one port task per machine, each
  update + model/animation + camera + sound/particles, bit-exact as the plane — and P88 (every plane's own
  particles); noted on P76 that its join/leave must call `Lot::resize`.
- opentennis has no vehicles; nothing was taken from it.

## What the original does

`machine_placement` (`./kit fn 0x0020f6e0`): for each player `i` of the event whose per-player kind
(event `+0x11c + i*0xc`) is 1..5 it reads `Biplane%d`, `Helicopter%d`, `Buggy%d`, `Monster%d` or `Baja%d`
with `i + 1` (format strings at 0x42edc0..0x42ee08; `./kit data 42ed50 0xd0 --as hex`), else (career)
`Machine%d`. The skin passed to `vehicle_factory`: with one player, the event's own (`+0x124`) for the
first machine; with two, 2 (`_ss`) for the first; the second machine (`i == 1`) gets 1 (`X2`) with one
player and 3 (`X2_ss`) with two. So the second player's machine is always the alternate livery, and the
`_ss` pick is the split-screen texture set.

## How it was proved

- `crates/openmx/tests/machines.rs` (disc): every machine's four models are in `machine.res`; on all five
  freeride tracks, with the real terrain, the file has exactly two spots per machine, and for 1, 2, 4 and
  8 players every machine is parked that many times — copies 0 and 1 bit-equal to the file's spots,
  the rest within 4 spacings of their spot, ≥ 1 spacing from every other copy, at the file spot's
  clearance over the dirt; every player's plane stands in their own bay, survives a swap there and
  back, and no two players' planes coincide; copy 1 loads `biplane2`, whose materials are
  `pitts_body02`'s, and copy 2 is `biplane` again.
- `vehicle.rs` unit tests: the same on a synthetic file for 1..33 players, a growing lot keeps every
  copy, a file with one plane parks the rest beside it, none parks none, and the swap's reset keeps the
  livery.
- `openmx --track free06 --players 4 --vehicle biplane --seconds 5 --shot`: four views, the first player
  in the plane, "2 of 2 liveries loaded", nothing broken (looked at, 960 px).

## Not verified / not 1:1

- **The four other machines are not stood up.** Only the biplane is ported; the lot parks the
  helicopter, buggy, monster truck and baja on paper (counted, placed, tested) and the shell neither
  draws nor rides them. S86–S89.
- **Copies past two are the port's own placement**: the original has two players and two spots; the
  sideways stepping, the spacings (40/50/18/22 ft) and the clearance rule are invented, not read.
- **The first plane's livery** is skin 0 here; the original's single-player first machine takes the
  event's `+0x124`, which the port's freeride never sets (0 by the factory default was not read live).
- **`_ss` textures are not used**: the original draws `_ss` in split screen; the port draws the full
  textures in every layout (render-side, its own choice).
- **`SetGroundHeight`** is still not applied (as P19), for the file's spots and the stepped ones alike.
- **Particles**: only the first player's plane emits them (P88).
- **Join/leave mid-ride**: `Lot::resize` is proved in the unit test, but the shell has no local join or
  leave path yet (P76) and online rides do not build the lot; nothing calls it at run time.
- **No pilot** sits in any plane during a ride, the first player's included (as before P87: only
  `--moment` seats one, V21). Two players both flying was not looked at on screen.
- The second livery was not seen in a screenshot (no camera angle in a 4-player shot faced a second
  plane); proved by its model name and materials instead.

## Next

Nothing for P87. S86–S89 port the other machines; P88 the per-plane particles; P76 calls `Lot::resize`.
