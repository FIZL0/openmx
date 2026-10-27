# 2 — 2p truck rides (FINAL)

Task S159. 2026-10-27.

## What I did

- Copied C12's truck capture, state and video from s1 into this slot's `context/`. Then played `split/truck.p2m2`
  in PCSX2 copy 3 (`research/c10/play_at.sh`) and saved twelve RAM images of P1's ride on path car 3:
  132087/88 (getting on), 132168/69 (landing on the deck), 132229/30 and 132289 (parked, rolling back),
  132499/500 (reversing), 132891..93 (leaving off the front). They are in `context/states/s159/`, made by
  `research/s159/rams.py`, with the `.p2s` copies in `context/states/s159/p2s/`.
- `research/s159/deck_frame.py` prints P1 in car 3's own frame row by row: across, up and along the deck,
  the bike's velocity over the deck, the car's speed, the wheel car words, on_ground and the held controls.
  `research/s159/deck_pitch.py` prints the deck's pitch.
- New test file `crates/game-sim/tests/two_p_truck.rs`. The brief names `tests/platforms.rs`, but that file
  holds S81's pinned generated vectors and has nothing to do with this, so the truck ride got its own file:
  - `the_images_are_the_truck_ride`: each image is its own tick, and P1's wheels name car 3 or nothing.
  - `the_carried_velocity_is_the_originals`: on P1's real wheels, chassis and car in all twelve images,
    `Moving::of(before, node, 1/dt).at(contact)`, `ride::contact_velocity` and
    `ride::wheel_contact_velocity` match the original's own rerun of the step (+0x1a0 cleared) **bit-exact**.
    That covers the carried velocity while the wheels are on the deck and the outputs on every image,
    including getting on and off.
  - `the_capture_rides_the_deck`: pins the capture. The wheels name only car 3 or nothing and P2 never touches
    a car. The first car contact is at 132089 and the last at 132894. on_ground stays 1 every row from 132164
    to 132893, within |x|<6, |z|<33 of the car, so the bike never leaves the deck. With nothing held it rolls
    back to between −3.2 and −2.6 ft/s, and P1 reverses on the deck.
  - `the_port_keeps_p1_on_the_deck`: P1's race500p bike, set where image 132230 has it on the port's car 3
    (free09_ss props and the car's collision mesh), with nothing held for the 36 ticks the original holds
    nothing, stays on the deck with drag and without. It moves under 1 ft along and 0.5 ft up.
  - Ignored and red, documenting the gap filed as S181: `the_paths_measure_as_the_originals` and
    `the_truck_drives_from_each_image_into_the_next`.
  - Ignored, `print_the_roll`: prints the port's roll per tick.

## What the original does

- **Carried velocity:** the same as the port's. The contact velocity a wheel on the car gets is the car's motion
  from its matrix before the step to its node's now, at 1/dt, taken at the contact point. Bit-exact here, so
  nothing to fix in `mover_contact.rs`/`ride.rs`.
- **Wheel contacts on the deck:** both wheels name car 3 while P1 is on it, and nothing in the air. In the
  capture the word is often 0 on a row while on_ground stays 1 (the row is read between ticks, so it is torn).
- **Staying on vs sliding off:** P1 lands on the deck at ~132164 and brakes (ctl03) to rest over the deck by
  ~132210. Car 3 then runs at a steady ~45.3 ft/s, pitched ~5° down the way it goes (forward row y −0.088), so
  gravity alone would pull the bike forward over the deck at ~2.8 ft/s². Yet with nothing held (132231..) the
  bike rolls **back**, from −0.1 to −2.0 ft/s by 132266 (~1.8 ft/s²), and reaches −2.94 at 132290. So something
  pulls it back at ~4.6 ft/s², which is what air drag on the bike's speed over the world (not over the deck)
  would do. That bears out S146's reading that drag reads world speed. The original does not slide off,
  though: the roll only lasts until P1 brakes or reverses.
- **Getting on and off:** P1 rides up a ramp onto the rear at 132089. It leaves off the front at ~132893, about
  46 ft/s over the car, and is in the air by 132894.
- **Path cars on free09** are PresampleTerrain (path_car_update, `./kit fn 0x00115540`). The first update snaps
  each key to the ground (key pos +0x10, normal +0x20). From then on the car's up is the two current keys'
  normals lerped at the key fraction (+0x538), blended with the normal filter (+0x560) when within 0.707 of the
  last up, and not taken from the ground under the point. game-sim's `traffic::Spec` has no presample model.

## How it was proved

- Function oracle on save-state RAM images: 12 images × 2 wheels for the carried velocity (bit-exact),
  `two_p_truck.rs`.
- PINE capture `context/capture/2p/truck.csv`, 1801 rows (237 with a car contact), pinned by
  `the_capture_rides_the_deck`.
- The port's stay-on: one run from image 132230 (36 ticks, drag on and off), `the_port_keeps_p1_on_the_deck`.

## Not verified / not 1:1

- **Path cars on free09 are not 1:1 (S181).** Every key's arc length differs from the image's (1500 keys, e.g.
  car 0 key 0 38.476448 against 38.45532). Stepping from one image to the next, the clocks and cursors match.
  The unlagged cars' point and heading match exactly but their up does not. The lagged cars trail by ~0.2 ft.
  Both tests are left ignored and red. The deck's pitch is close (port −0.0837 against −0.0885).
- **The port's roll back is not the original's (S182).** From the same image the port holds the bike near
  −0.5 ft/s over the deck (drag) or drifts to +0.4 (no drag), and gets ~0.2 ft back where the original gets
  0.65 ft. Likely causes, none checked: the port bike is seeded from the image's chassis pose and velocity only,
  with no engine, gear, wheel spin or angular state; tire rolling resistance; the car's up (S181).
- The whole bike was never compared tick by tick against the original: the port's rider is not built from a
  RAM image, so only the chassis pose and velocity carry over.
- The drag share (~4.6 ft/s²) is inferred from the slope and the measured roll. It was not compared against
  race500p's drag constant.
- Getting on and off: only the wheel words and the carried-velocity step were checked at 132087/88, 132168/69
  and 132891..93. The port never rode the ramp up onto the deck or off the front.
- The capture rows are torn at NominalScalar 0.05 (rows read mid-tick). Bounds are therefore loose, and the
  car word is checked as "car 3 or nothing".
- `the_port_keeps_p1_on_the_deck` and the image tests skip without this slot's `context/` (images, the disc).

## Next

S181 (presampled path cars), then S182 (the port bike's roll back on the deck).
