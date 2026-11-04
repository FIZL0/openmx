# F31b3: the bike's PostProcess

2026-11-04. Task: `./kit plan F31b3` (F31b split, 3 of 5).

The bike's PostProcess (its air state, once a frame after the substeps) is now ported as
`game_sim::handling::bike::air_state`, with the bike's no-hander (`bike::no_hander`) and the vehicle post
(`bike::vehicle_post`). They run on the quad's types, as F31b1/F31b2 do. Addresses and the call map are in
research/oracle.txt under `# F31b3`.

## What it does

- It is guarded the same way as the PreProcess. With control on and the balance words differing, it
  clears the balance state (the `Rider`'s inertia, command, point and noise time) and runs the no-hander.
- The no-hander is MX Unleashed's `move::no_hander` clause for clause, read off the bike's words: the
  stand-upright speeds, the roll command, speed and gain, the donut scale, each wheel's ray-hit and swept
  words, and the rear's donut. It runs through a `move::Outside` (`Hands`) on the bike's body: unrotate with
  the body's `to_local`, take the forward axis from the quaternion, and set the axes as the frame at
  `(0,0,0,1)` orthogonalised, keeping forward, then `mat4_to_quat`.
- Then the quad's air state (`quad::air_state`) on two wheels: speed, local velocity, axes, euler, world
  spin, on-ground/all-down/airborne, hang time, contact velocity (the front's, then the rear's, then the
  body's own), crossup, tip-over flag. Unlike the quad's, it does not write the front and rear pair flags.
- The origin (the position less the turned centre of gravity, `atv_moto::origin`) goes through
  `atv_moto::in_world`. Both are now `pub`; there is no other change to atv_moto. If the origin is
  outside the world, the bike zeroes the wreck reason and the two words beside it, posts its
  out-of-world message (`Out::OutOfWorld(post id)`) and puts itself back (`Out::PutBack`, place and keep).
- The vehicle post: with the body on, the origin is tested again. Inside, the node is placed
  (`Out::Place`). Outside, the body reset and the respawn run. Then the spatial update, the node bounds
  when bit 1 of the flag byte is set (read after the calls before it), the collision sync, and bit 2 of
  the flag byte cleared.
- Last, the node's position, and its kept copy.
- The new words are a `bike::Post` (`read_post`, `post_words`). The `bike::Posts` trait (a `Rides` that
  owns the `Post` and makes the bike's own calls, `bike::Out`) hands it over.

## Checked

- `the_bikes_post_process_is_the_originals`: 12 000 bikes, each state 4's with the PostProcess's inputs
  drawn at random: guards, wreck reason, balance words and state, the no-hander's keys (stand-upright
  speeds tied and roll speed 0 now and then), both wheels' contact, ray and sweep words, normals (`y` at the
  0.17 edge now and then) and contact velocities, the rear's donut, signed speed and lean, the body's on
  word, quaternion, velocity, spin, position and cofg, airborne and hang time, flipping, spinning,
  authority, the flag byte, the world's floor and box (or none), the post id, and `dt`. Each one runs
  through SLUS-21104's 0x004d9290 and through `air_state`. Every word of the bike, both wheels, the body
  and the record's tip-over word matches. The calls also match in order and arguments; the original's
  callees outside the bike's own code are hooked to log, and the node position answers a fixed value.
  The no-hander, the in-world test and the math are the original's own code. Path counts: off 2936,
  wrecked 1497, balancing 1957, no hands front 2932 / rear 1425 / no ground 1253, rolled 4357, donut
  2818, took off 291, in the air 571, both down 5082, one down 3411, crossup 3721, crossup ramped 3464,
  tipped 4656, out of the world 3361, body off 1143, placed 4978, reset 2943, node bounds 3940, anywhere
  2257, no box 862, boxed 3689, floored 2256.
- `ride_replays_bit_exact_on_the_ports_post_process`,
  `flip_wrecks_and_is_put_back_bit_exact_on_the_ports_post_process`: F31b2's hybrid frame (the port's head
  and Force body), with the bike's post element (bike `+0x34`) run by `air_state`, once a frame
  (asserted). Its callees run in the interpreter on the bike in RAM, with `dt` in `f12` and only the
  changed words written around each one. All 459 and 465 rows match word for word, and the flip wrecks on
  row 125 and is put back on row 193.
- Mutations:
  - Skipping the no-hander fails the oracle (bike 0) and both replays (row 0).
  - Keeping bit 2 of the flag byte fails the oracle (bike 2).
  - Taking the rear's contact velocity first when both wheels touch fails the oracle (bike 2).
  - The last two pass the replays: there only the oracle covers them.
- `./kit check -- -p game-sim --test mxvatv_bike_ride --test mxvatv_step`: 29 passed. The same in
  `cargo test --release`, where the oracle printed the path counts above (so it did not skip).
- `./kit audit`: the same 4 leaks as on main (atv_wheel.rs, plan/DONE.md); none are in these files.

## Not verified / not 1:1

- The original's no-hander has `adda.s 0, f7` where `move::no_hander` adds straight. These differ only
  when the step's product is itself `−0` (an underflow flushed to a signed zero). The port does not fold
  it, and the oracle never drew one.
- In the replays the PostProcess's callees (node placement, position and bounds, body reset, respawn,
  spatial update, collision sync, the message post, the put-back) are the original's. Their Rust ports on
  the bike are F31b4/F31b5's.
- Neither ride leaves the world, so the out-of-world message and put-back, and the vehicle post's
  reset/respawn branch, are covered only by the oracle, where they are hooked to log.
- The oracle's world box is drawn in scratch RAM, not the map's own; the flag bits it draws include
  ones the game may never set together.
- The oracle's node position is a stand-in value on both sides.
- Only the targeted test file was run, not the full replay suites. Nothing outside `handling::bike` and
  the two now-`pub` atv_moto functions changed.
