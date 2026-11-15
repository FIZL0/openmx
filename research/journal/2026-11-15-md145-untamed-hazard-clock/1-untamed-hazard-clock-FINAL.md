# 1 — untamed hazard clock (FINAL)

Task MD145. 2026-11-15.

## What I did

- Read the clock's two other SLUS-21701 callers, the bike's update and the car's, with `./kit dis` on a
  scratch Untamed KIT_CONFIG (memory disassemble-untamed-from-its-ram-image). I compared the bike's update
  with SLUS-21104's twin, the Motocross override, by a normalised diff of the two decompilations, and then by
  instructions where they differed.
- Bike: ported `handling::untamed::bike_hazard` (the held word, `grounded` = either wheel's ray down, and
  Untamed's clock). Added `handling::tail::vehicle_update_untamed`, the bike's update with Untamed's clock
  and its distance clause (`!(d <= 0)`).
- Car: MD81d had already routed it (`atv_car::Update::untamed`, `grounded` = any tire solid). I proved it
  here.
- Node update: added `atv_moto::Scene::untamed`. With it set, `Scene::update` is `update_untamed`, so every
  scene method goes through Untamed's. `atv_car::Load::vehicle` sets it from `Load::untamed`. The quad's is
  left for MD144, which routes the quad's other Untamed switches (noted on MD144).
- Wrote research/untamed-machines.md, "The hazard clock's callers and the node update on every machine
  (MD145)", and research/oracle.txt `# MD145`.
- Took nothing from opentennis: it has no counterpart.

## What the original does

- The bike's update sets the held word (either wheel's surface table word) and computes `grounded` as either
  wheel's ray word. It hands the clock `on = held && grounded`, then `grounded`, then the longer of the two
  surface times. Its other changes from SLUS-21104's: the rumble pad's vtable slot (presentation), and the
  distance clause, which became `if !(d <= 0) { dist += d·dt }`. That parts from SLUS-21104 only on a `-0`
  distance.
- The car's update sets on-hazard (some tire's surface is a hazard) and computes `grounded` as any tire's
  solid word. It hands the clock `on = on_hazard && grounded`, then `grounded`, then the longest of the four
  times.
- In MD164's Untamed trophy-truck spawn and air RAM images, the truck's scene node has parent word 0. So
  Untamed's node update, which re-orthonormalises only a node with a parent, leaves it as SLUS-21104 does.

## How it was proved

`crates/game-sim/tests/untamed_hazard.rs`, on `context/untamed/ram/nat_or.bin`:
- `untamed_bike_hands_the_clock_either_rays_hit`: 6000 generated states. Each runs the bike update's stretch
  from the inner update's return to the clock's return, with the clock run for real and the put-back
  hooked. It compares held, free, stuck, the time's bits and the put-back count with `bike_hazard`. Tally:
  no ray 1032, no ray + put back 500, ray not held 1209, held 2282, held + put back 977.
- `untamed_car_hands_the_clock_any_tire_solid`: 6000 states. Each runs the car update's stretch with the
  clock hooked, and compares a1, a2, f12 and f13 bit for bit with the port's `Call::UntamedHazardClock`. It
  also checks the on-hazard word. Tally: no tire solid 2943, solid and off hazard 1065, solid and on hazard
  1992.
- `untamed_car_node_has_no_parent`: reads both MD164 images (2 read), then checks over 200 random frames
  that a port scene's root updates the same with and without the switch.
- I confirmed all three tests ran rather than skipped, with a temporary panic (memory
  kit-check-pass-hides-skips). Neighbouring suites (untamed_atv, mxvatv_bike_ride, mxvatv_car_frame,
  atv_quad, untamed_truck, untamed_car) pass: 289 tests.

## Not verified / not 1:1

- **Nothing new runs on the `Handling::Untamed` bike ride.** That ride (ride.rs) is MX Unleashed's sim with
  MX vs. ATV's steps. It has no hazard clock and no scene graph, so there was nothing there to swap.
  `tail::vehicle_update_untamed` has no caller yet. It is proved only through the stretch above, not on a
  ride.
- **`vehicle_update_untamed` mixes versions.** Its inner update is SLUS-21104's `tail::inner`, not Untamed's
  `untamed::vehicle_update` (MD93c), because the two take different shapes. Its distance clause is ported
  from the instructions (0x004f2828) but not oracle-run. The rumble slot change is presentation and not
  modelled. The rest of the bike's update was compared by a normalised decompilation diff only. That diff
  showed moved fields, plus a node-pointer read moved after the inner call, which is assumed to be
  scheduling.
- **The quad's scene switch is not set.** Nothing sets `Scene::untamed` or `Tail::untamed` on the quad yet.
  That is MD144.
- **Untamed's node update does not reach the car's model nodes.** The port's car scene holds only the truck's
  node, a root, which matches the images. The original's tire and body nodes under it each have a parent and
  do go through the re-orthonormalisation, but the port does not model those nodes. Only MD164's baja
  images were read for the parent word. The monster truck, golf cart and sandrail were assumed to be the
  same, because they use the same car class and spawn path.
- The quad caller's `grounded` is still MD81b's static read. No stretch was run for it.

## Next

None. MD144 sets the quad's `Scene::untamed` with `Tail::untamed`.
