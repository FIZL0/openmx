# 1 — kiowa wreck (FINAL)

Task S220a1. 2026-11-02.

## What I did

- `crates/game-sim/src/kiowa.rs` has a new section at the end, "the wreck and the put-back":
  - `Wreckage`: what the four functions touch besides `Ride`:
    - `[Wrecks]`' gravity, damping and inertia
    - the built inertia (+0x410), the mass, and the body's +0x110 copy of its inertia
    - the wrecked flag (+0x3d8)
    - the wrapper's place (+0xe0 x3) and the MX spawn (+0xcd0 x3)
    - which nodes are hidden (`Nodes`)
    - the four blades (`Blade`: a body, plus whether it was switched on)
  - `Throw`: the blades' nodes' and the model's world rows, as the caller's input.
  - Methods:
    - `wreck` (base), `wreck_mx`, `throw_blades`, `unwreck` (base), `unwreck_mx`, `put_back` (MX)
    - `act(&Crashed, ...)`: the three in the crash test's order. It returns the put-back's altitude
      fraction.
  - `Kiowa::unwreck_controls` / `put_back_controls`: the controls share, which the frame cannot reach.
    The caller runs it after the tick.
  - Private helpers:
    - `set_inertia` (body_set_inertia)
    - `place` (physics_object_place on a body, reusing `flight::spawn_turn` / `spawn_axes` and
      `ragdoll_pose::Start::reset`)
    - `node_point` / `node_rotate` / `node_unpoint` (the three scene-node transforms)
- `research/oracle.txt`: an S220a1 block. It names kiowa_wreck, kiowa_unwreck, kiowa_put_back,
  kiowa_wreck_mx, kiowa_unwreck_mx, kiowa_put_back_mx, kiowa_throw_blades and physics_object_reset, and
  maps the fields.
- No `Ride` or `Crash` field was added: both are built as struct literals in other tests.

## What the original does

- **Base wreck** (`./kit fn` kiowa_wreck). In order:
  - The reason; down = 0; the wrecked flag = 1.
  - The body's rates `vmulx.xyz` by 1.0.
  - Body vt+0x1c / +0x54: both dampings = WreckDamping (whole quads).
  - Body vt+0x50 (body_set_inertia) with WreckInertia·+0x410 (`.xyz`; `w` WreckInertia's):
    - The setter writes 1 into the source's `w` and copies it to body+0x110.
    - It sets each inverse lane to 1/v, or 0 when v <= 0.
  - Gravity +0x140 = (0, 32.2·(−WreckGravity·mass), 0, 1).
  - The sound object's vt+0x8c.
- **MX wreck**:
  - The base wreck.
  - Unless the reason is 13: vt+0x1b0, the blade throw.
  - The player's rumble.
  - +0xd3c is hidden and +0xd40 shown.
- **Blade throw**. Blade k uses node +0xd08+4k and wrapper +0xe48+4k (its body at wrapper+0x110):
  - p is the blade body's CofG through the node's rows.
  - The axes are the node's rows 2 and 1, with `w` 0.
  - Three `rand()`s give `5·2^-31·r` on each of x, y, z. They are added to the helicopter's rates
    (`.xyz`), and then y gets 7 more.
  - The lever is unpoint(model node +0x18c, p) minus the helicopter's CofG.
  - velocity = the helicopter's velocity + rotate(model, spin × lever).
  - Then, in order:
    - wreck_part_start(wrapper)
    - the wrapper's place (vt+0xe4)
    - the blade body's velocity = that velocity, and its rates = the helicopter's rates (whole quads,
      not the spin)
  - After all four blades: the rotor's 4 nodes and +0xd3c are hidden, and +0xd40 shown.
- **Base unwreck**:
  - If the reason is 13: the throw scale /= 0.5.
  - Zeroed: the reason, the clock and the wrecked flag. Down is not zeroed.
  - Both dampings = (1,1,1,1).
  - body_set_inertia(+0x410). This sets +0x410's `w` to 1.
  - Gravity from the gravity scale.
  - The sound's vt+0x88.
  - Zeroed: +0x334..+0x348 and +0x378.
- **MX unwreck**:
  - The base unwreck.
  - Then, if +0xd1c and +0xe48 are set:
    - The rotor's nodes and +0xd3c are shown, and +0xd40 hidden.
    - wreck_part_stop(blade, 1) on each blade.
- **Base put-back**: physics_object_reset(heli+0x50), which takes the probes out, then runs
  place(+0x90/+0xa0/+0xb0) and puts the probes back. Then vt+0x1bc.
- **MX put-back**:
  - The base put-back.
  - Then the place depends on the owner (+0xb94):
    - If there is no owner, or the owner's +0x5c bit 0 is clear: the spawn (+0xcd0) is copied over the
      place, and +0x33c = 0.
    - Otherwise: +0x33c = 0.5.
  - camera_snap_player, kiowa_mx_present_a, and +0xd00 = 0.

## How it was proved

- Function oracle on S148's live helicopter (`context/states/s148-kiowa.ee.bin`, which is also in s2).
- The test is `the_wreck_and_the_put_back_are_the_originals` in tests/kiowa.rs. It runs 10,000 cases, about
  1,650 per function.
- Each case generates:
  - the helicopter body's rates (denormals included), velocity, dampings, inverse inertia, +0x110,
    position, orientation, CofG and mass
  - reason, down, clock, throw scale, gravity scale, gravity and the wrecked flag
  - WreckGravity, WreckDamping and WreckInertia, and +0x410. These lanes are often 0, −0, negative, a
    denormal or 1.
  - the place and the spawn (the place sometimes with the shipped axes)
  - all six nodes' hidden bits
  - the pad and held height
  - every quad of the four blade bodies
  - the four blade nodes' and the model node's rows (turned, sometimes stretched, with any row `w`)
  - the rand state
  - the owner: none, keeping, or not keeping the place
- What runs real and what is stubbed:
  - Stubbed: the sound's two slots, the rumble, camera_snap_player, kiowa_mx_present_a and
    transform_rebuild (the test writes the rows).
  - wreck_part_start/stop are hooked and logged.
  - Everything else runs real, including both probe passes of physics_object_reset and the place's
    node writes.
- What is compared:
  - every quad of the helicopter's body and the four blade bodies, by lane
  - body+0x110 and +0x410
  - the gravity, reason, down, the wrecked flag, the clock and the throw scale
  - the place's 3 quads
  - the 6 node bits
  - the pad, +0x33c and +0x378
  - the rand state
  - each blade's on/off against the logged starts and stops
- Every kind is reached ≥ 1,564 times:
  - 2,761 cases switch blades on.
  - 541 put-backs are owned.
  - 1,831 cases have a wreck inertia lane not above 0.
- `a_crash_test_answer_is_acted_on_in_the_originals_order`: `act` equals the separate calls in order,
  over 200 cases.
- Mutations:
  - These turn the test red:
    - `<` for `<=` in the inverse
    - the throw scale not doubled back
    - no +7 lift
    - the owner test flipped
    - no rotate on the swing
    - the setter not writing `w` 1
  - Dropping the ×1 rates scale does **not** turn it red: the oracle finds no lane it changes, denormals
    included. It is kept as the original's.
- Found on the way: the unpoint's third column is the plain transpose. My first reading of the word
  shuffles had it take row 0's `w`, but the `move` is a 64-bit `or`, which keeps the register's upper half.
  Also, `flight::spawn_turn` squares its inputs itself, so it is given the raw nose and up.
- Other checks:
  - `./kit check -- -p game-sim --test kiowa`: 13 passed.
  - `--test kiowa_phy --test kiowa_capture`: 4 passed.
  - `cargo build -p openmx` builds.
  - `./kit audit`: only the 2 pre-existing plan/DONE.md leaks.

## Not verified / not 1:1

- Nothing in openmx calls `Wreckage` yet. KiowaRide's crashed answer is still only logged (follow-up
  filed).
- The blades' bodies are not built from `wreck_blade`'s part; `Wreckage::new` leaves them empty.
  Nothing steps or draws a thrown blade.
- `Throw`'s rows come from the caller. The port has no scene graph to take them from.
- What wreck_part_start/stop do inside a wrapper is not ported; only a flag is kept. Per oracle.txt,
  they show/hide its node, put its probes in and out, and call its vt+0x124/+0x128.
- Not ported, as they are not simulation: the sound, the rumble, the camera snap and the presentation
  copy. +0xd00, which the MX put-back zeroes, is unknown and not ported.
- The rotor-node gate (+0xd1c && +0xe48) in the MX unwreck is assumed always true, since the MX
  constructor builds both.
- The scale by 1 on the rates is unobservable on the oracle (see above).

## Next

Wire `Wreckage::act` into openmx's Kiowa ride, and build the blade bodies (follow-up).
