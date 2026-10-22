# 1 — radar icons like the original (FINAL)

Task P92. 2026-10-21.

## What I did

- `crates/openmx/src/overlay.rs`: the rest of the original's radar set as `Icon` / `Blip`, placed by
  `radar_icon` (which is `radar_mark` plus each kind's texture, size, colour and turn):
  - the activities' start zones, `map_icon_h` / `_r` / `_m` / `_s`, 32x32 scaled by `zone_size`
    (clamp(0.3 + (R − d)/R, 0.5, 0.8), R = half the span), in the original's colours;
  - a machine nobody is in, 32x32 in its livery's colour (all five kinds have their texture and
    colours, though only the biplane stands in the port), and `map_icon_bike` 0xc8c8c8 for a bike
    whose rider is in the plane;
  - an activity's hit point (`map_icon_hit_freestyle` 8x8, green, grey when done): in the set but
    fed by nothing, as the port has no activities;
  - the port's session markers (P88) on `map_icon_gate` (32x8), the original's activity-gate icon,
    turned to lie across the way the marker faces and turned with the rose; grey 200, white when
    selected. The original has no markers, so this is the port's choice: the gate is the one icon in
    the set that means "a spot to ride through, facing this way".
  - `zones` reads `hit.dat`'s `[ASZ n]` sections (`Position`, `Type`, spaces round `=` or not, as
    free07's are); `shown_zones` picks what the original shows a player who has started nothing:
    the first zone of each activity, and not the race's while that player is in a machine.
- Fixed V26's rider mark colour: the original's UI colour words are 0xAARRGGBB, so a human's mark is
  yellow (0xffffff00), not cyan, and an AI's red (0xffff0000), not blue (`HUMAN_MARK`, `AI_MARK`).
  The original's own screenshot shows its AI dots red. The test that checked for cyan now checks for yellow.
- `Pen::turned` takes a width and a height and a tint (the gate is not square and is tinted); the
  rose is drawn as before, with `[255; 4]` passing every pixel through unchanged.
- `crates/openmx/src/main.rs`: `radar_blips` feeds each view's blips every frame into `Art::radar`:
  the planes nobody flies, the bikes whose riders fly, the zones (off the track's `hit.dat`, read
  once beside the radar span), and the view's own markers. Order: machines, bikes, zones, markers,
  then the riders' dots and the arrow on top, the original's build order (see below).

## What the original does

- `./kit fn 0x0035bb70` builds the icon container: a pool of 20 8x8 dots, the two machine icons, two
  bikes, pools of 5 H and 5 R, one M, one S, each with its colour word. The machine icon's texture
  and colour come from the event's chosen machine kind and copy for P1 and P2.
- `./kit fn 0x0035d3e0` (radar_update) walks the activity list and places a zone whose per-player
  flag byte has 1 and 4 (`./kit fn 0x0035efa0`, the same r/a as the riders, scaled). It also walks the
  machine list and places a machine (`./kit fn 0x0035eac0`) that is a bike or unlocked, standing
  with nobody in it (+0x6c0 parked), for a rider answering an interface. Then it places the riders.
- `./kit fn 0x0035df50` (machine point/arrow) and `./kit fn 0x0035f480` (gates, with the current
  gate's 0.5 s fade) are activity-only.
- The flags: in all three RAM images of the human's save (first_recording on free09, recording2,
  biplane on free06) only the first zone of each type has 1|4. The race's is cleared while the player
  is in the plane (biplane.ram), by the swap (`./kit fn 0x003601b0` → `./kit fn 0x003aa640`).
- Full addresses, colours and offsets: `research/oracle.txt`, the V26 radar lines and the new P92 block.

## How it was proved

- **Against the original's RAM, to 0.1 px**: first_recording (free09, on the bike, heading north).
  The port's formula, fed with the player's bike (4310, 3320), the radar's forward and the
  activities' own positions, gives H 74.7,362.0 ×0.8, R 36.5,350.6 ×0.5, M 105.3,337.7 ×0.7 and
  S 35.0,361.3 ×0.5. The original's icon objects hold 74.7,362.0 / 36.5,350.6 / 105.3,337.7 /
  35.0,361.4 and the same scales. biplane.ram (free06, flying south): the player's parked bike comes
  out at 54.1,393.2 against the original's 54.2,393.0, H/R/S likewise, and the three AI dots land on
  the three AI bikes.
- **Beside PCSX2 screenshots** of those two states: the port's free09 start (`openmx --track free09
  --headless --shot`, which starts at 4310,3316) has H, R, M and S where the original's screenshot
  has them, in the same colours and sizes. The biplane screenshot shows the grey bike icon by H, S
  drawn over H and the red dots over the bike, which is where the draw order comes from.
- **Tests** (`crates/openmx/tests/overlay.rs`):
  - `each_icon_kind_is_placed_by_radar_mark`: every kind, inside and past half the span, at
    `radar_mark`, on the ring past it, with its texture, size and colour;
  - `an_activitys_icon_grows_as_it_nears`;
  - `a_markers_gate_turns_with_the_rose`;
  - `the_start_zones_are_the_first_of_each_activity`, on synthetic text;
  - `the_freeride_tracks_have_their_start_zones`, which needs the disc: 5/5/1/1 on all five freeride
    tracks, and free06's first of each at the positions the original's RAM has;
  - `the_radar_draws_its_icons`, which needs the disc: CPU positions, GPU tints, the turned gate;
  - `the_gpu_draws_the_hud_as_the_cpu_does`, now with every icon in every view.
- `./kit check`: passed.

## Not verified / not 1:1

- **Which zones show is inferred, not ported.** The original's rule is its activity progression
  (per-player flags, `hit.dat` / `free.hit` / `free.set` / `freestyle.aty`), which the port does not
  have. "The first of each type, the race's hidden in a machine" is what three RAM images of one
  save show, all of a player who has finished nothing. A player who has done activities would see
  others (and done ones dimmed to alpha 0x6e), and the port never shows those.
- **Activity hit points, gates, the machine point/arrow and the current gate's blink are not fed**:
  activity-only, and the port has no activities. The hit point is in `Icon` with its colours and is
  tested, but nothing produces one; gates and the machine arrow are not in `Icon` at all.
- **Machines other than the biplane** have their textures and colours (tested) but nothing places
  them, as the port does not stand them up. Locks: the original places a parked machine only if
  unlocked in the save (first_recording's parked monster truck was locked and not drawn). The port
  has no locks, so its parked plane always shows. No screenshot of the original with a parked
  plane icon was taken.
- **More than two players**: the original has two machine and two bike icons. The port marks every
  seat's parked plane and every flying seat's bike, in the liveries alternating as the lot gives them.
- **The interface check** the original makes before placing a machine (the rider answering the
  0x0043fc40 interface) was not identified. The port places them unconditionally.
- **Draw order** between the bike icon and H is from one blurry screenshot plus the build order.
  S over H and the dots over the bike are clear in it.
- **The human mark's yellow** follows from the colour words' byte order, proved on the AI dots (red
  on screen). A human rider's dot in the original was never seen on screen (it needs a two-player
  free ride).
- **The markers' icon** is the port's choice (gate, grey/white, turned to the marker's facing).
- The parked bike icon sits at the bike's ride origin, which is the original's machine transform
  only as far as the port's bike origin is (54.1 vs 54.2 px in biplane.ram).

## Next

Done, unless the freeride activities are ported (follow-up P100). They would feed the radar the
real zone flags, the hit points, the gates and the machine arrow.
