# 1 — atv modifiers (PART)

Task P287. 2026-11-12.

## What I did

- **game-sim `modifiers.rs`**: a new `Category::Atv` ("ATV"), kept out of `Category::ALL` (the
  disc's machines and the world, as before) since it is an addition's. Four new per-seat knobs in
  `Knob::MENU`, words `atv_speed`, `atv_accel`, `atv_grip`, `atv_suspension`. `KNOBS` is now 32. They
  go over the wire in the text form, so the online wire layout is unchanged.
- **`Mods::atv(seat, &mut quad::Load)`** scales what the quad's `.phy` loaded, before the ride is
  spawned from it. Each knob is reasoned the way the bike's same-named knob is:
  - top speed: torque curve, clutch frictions and engine braking ×k; final drive /k (gear set again);
    `[Aerodynamics] Drag` and every surface's WheelDrag /k²; reverse speed and clutch-scale speed ×k;
  - acceleration: torque and clutch ×k; Drag, WheelDrag and WheelFriction ×k; above 100% the rear
    pair's MaxDriveForce and SkidDriveForce ×k;
  - grip: all eight of each surface's drive-row keys ×k, on all four wheels;
  - suspension: each wheel's spring rate ×k and its two damper rates ×√k (as the cars', because the
    loader has already turned the ratios into rates against the spring); the stopped and moving
    pair springs in `TireSetup` and `mv` ×k.
- **`Mods::atv_body`**: with top speed above 100%, the body's 300 ft/s spawn limit ×k, so the limit is
  not what holds a faster quad back. This does what `Mods::ride` does for the bike.
- **vehicle.rs**: `AtvRide` now carries `mods` and `seat`. `Garage::modify` sets them, and `made`
  applies them, so the change takes on the next take.
- **menu_pages.rs**: `mods_categories()` puts ATV before World while the addition is on (`atv_on`):
  a handling file is picked on the Garage's Quad row, or under the pause the ride has a quad
  (`--atv`). Under the pause on the quad the page opens on ATV. The knobs are held online ("Online
  rides have no quad yet."), the way the Kiowa's are.
- Tests: new `crates/openmx/tests/atv_modifiers.rs` (4 tests); new
  `the_atv_category_shows_while_the_quad_addition_is_on` in tests/menu.rs. I changed
  `the_modifiers_page_shows_one_machines_knobs_at_a_time` (it compares against the MENU knobs minus
  ATV's) and `every_modifier_off_its_default_reaches_the_ride` (the ATV knobs leave the bike alone).
- opentennis: I did not look. It has no machines, and this is a category over code already here.

## What the original does

Nothing. These knobs are an addition on an addition (MX vs. ATV's quad in MX Unleashed, MD37d). At
100% each knob is skipped outright, so the quad loads exactly MD37d's numbers.

## How it was proved

`tests/atv_modifiers.rs`, on openmx-mods' `mxvatv_atv` pack, Race 250 P, on `world::Flat`:
- With every knob at 100%, and with every other machine's and the world's knobs moved, the quad's
  `Load` (Debug, every float's bits) matches stock. A 120-tick gas-and-turn ride's body
  (position, velocity, quat bits) also matches stock.
- Each knob moves its keys by the stated factor (exact float equality on samples).
- On the ride:
  - top speed in 20 s: 60.2 / 120.4 / 178.7 ft/s at 50/100/150%;
  - speed 2 s off the line, leaning forward: 51.6 / 79.6 / 102.4 ft/s at 50/100/150%;
  - sideways pull in a full-lock turn on the gas: 114.8 → 72.5 ft/s² at 40% grip;
  - settled height: 0.62 / 0.90 / 1.03 ft at 50/100/200% suspension.

## Not verified / not 1:1

- **Acceleration of 150% or more loops the quad off the line.** With the stick centred, the Race
  250 P wheelies over onto its back within a second at 150%. Leaning forward rides out 150%. At
  200% it loops even leaning. That is the extra torque at the rear tires. I left it alone, because
  taming it means changing the wheelie controller's keys, and that is no longer "a scale on its
  .phy". The test measures acceleration while leaning forward for this reason.
- **A quad already out when the pause closes keeps its old numbers until it is next taken.** A car
  out does the same. Retuning a ride in place would mean reaching into `quad::Ride`'s copies
  (`Load::tune` also resets the gear), so I did not.
- Only Race 250 P was measured. The other seven handling files build through the same code, but
  were not ridden under the knobs.
- The rides were measured on a flat floor in a test, not on an MX Unleashed world in the shell, and
  were never watched: the quad is still undrawn (MD37d).
- The suspension's √k on the dampers is a choice copied from the cars'. The bike's suspension knob
  leaves its dampers alone.
- Online: the knobs are greyed by the same rule as the Kiowa's, but no online ride was run; the
  quad itself is offline only.

## Next

None. The work merged to main unticked (MD143 had the gate red). MD143 is now closed (K51 took the
triangle-test scratch out of atv_quad's compare). The reopened run changed no code and only ran the gate
and ticked P287.

## Gate note

`./kit done` was refused twice by `game-sim --test atv_quad`'s whole-frame compares (sim_frame.rs:1129),
and nothing P287 touches is involved (atv_quad reads no modifiers):

- On base 22c81ae31 (MD118): only `race_stoppie_short_replays_by_the_port` failed, at frame 109. It failed
  alone too.
- After I cherry-picked MD119 (main 53d0f3410, the quad record's sweep, now 205ed1af3 here) on the guess that
  it was the fix, three tests failed: that one, `race_wheelie_launch_replays_by_the_port` (frame 88) and
  MD119's own `the_sweep_that_hits_the_palm_is_the_ports` (frame 124). They fail the same way alone.
- Bare main dfbb57886, in a scratch worktree with this slot's context: the same three, at the same frames.
  The context files (MC02.esb, mc02.res, the atv-r-spawn images) have the same md5 in s1, s2, s3 and
  kept-context.

So main is red. The words that differ are seven quads at 0x003dce50..0x003dcebc, after MD118's tree-walk
globals. The original writes them (points with w 1.0) and the port leaves them at 0. That is MD143. The
MD119 cherry-pick stays on the branch: it is main's code unchanged, so the merge is a no-op for it.
