# 1 — kiowa shell wreck (FINAL)

Task P216a1. 2026-11-03.

## What changed

- `vehicle::Heli` carries a `kiowa::Wreckage`, built in `KiowaRide::built`:
  - `Wreckage::new(phy, KIOWA_INERTIA, mass)`. C14's image has the built inertia +0x410 equal to the
    body's +0x110 (0x4628adfa 0x461f901b 0x44a2ae40 1), so `KIOWA_INERTIA` serves for both.
  - The place and the spawn are both the built body's position, the nose and up. S148's and C14's
    images hold place == spawn, (3026, 132.647, 4054) with nose (-1, 0, 0).
- `KiowaAround::crashed` calls `Wreckage::act(asked, ride, throw, owned = false, rng)` after
  `Grounded`'s own. It keeps the altitude fraction when the helicopter was put back.
  - Owned is false: both images have owner +0xb94 = 0 while the helicopter is flown, so the
    original's put-back copies the spawn over the place.
- `KiowaRide::step` drops the rebuild. After a put-back it runs `Kiowa::put_back_controls(fraction)`
  and `Collision::refresh` (the reset's probes go back into the broad phase where the body now
  stands).
- `KiowaRide::rotor`'s third value (the tail drawn bent) is now `!wreckage.nodes.wrecked`, the
  +0xd40 node the wreck shows, not `reason != 0`. These are the same apart from the order within the
  frame.

## Stand-ins

- `Throw`: every row is the measure's matrix. S270 is the follow-up for the blade nodes' real rows.
  The shell builds no blade bodies (P265), so the rows move nothing. The three rand() draws per blade
  (12 per throw) run all the same, so the rng is the original's.
- The measure's matrix (what `place()` and the report read) catches up with the put-back on the next
  tick's post. The presentation copy `kiowa_mx_present_a` is not ported.

## Checked

- `cargo test --release -p openmx --test kiowa_switch -- --nocapture`: 4 passed, none skipped.
  - `a_landing_at_speed_wrecks`: 1 wreck, then on its skids 0.6 ft from where it was taken. It was
    ~990 ft on under the old rebuild-in-place. The test now also asserts the put-back is within 5 ft
    of the spawn.
  - `a_soft_landing_stands`: 0 wrecks, on its skids, -0.6 ft.
- `./kit check -- -p openmx --lib --test bays --test kiowa_switch --test kiowa_particles --test overlay`:
  124 passed.
- `--test kiowa_picture --test bay_up`: 14 passed.
- `./kit audit`: only the 2 plan/DONE.md leaks that are already in HEAD.

## Not checked

- No PCSX2 side-by-side of a wreck's put-back. The spawn comes from the images' owner and place
  words, not a recorded wreck.
- No picture was looked at.
