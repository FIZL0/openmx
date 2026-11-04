# 1 — trick gear net virtuals (FINAL)

Task F42. 2026-11-04.

## What I did

- Ported the four bike virtuals that F39's `game_sim::handling::update` still called out to as
  `update::gear` (vt +0x358), `update::trick` (vt +0x4e4, `dt`), `update::net_check` (vt +0x3e8) and
  `update::net_unpack` (vt +0x3f0, `dt`). The two table lookups the step makes, the stunt entry (already
  `stunt_entry`) and the stunt of a kind (new `stunt_of_kind`), are inlined. The step's own deeper calls
  and the unpack's are new `Out`s: `TrickRemote`, `TrickHold(entry)`, `TrickEnd(id)` (was a bare
  `TrickEnd`, now carrying its id; the lean's call passes `u32::MAX`, as before), `TrickWanted`,
  `TrickStart(id)`, `Mark { run, player, on, id }`, `NetInterpolate { older, newer, t }`,
  `Orient { node, at, pos }`, `WheelClear(w)`, `Contacts`, `Suspension(w)`, `TireForces(w)`, `NetPost`.
- `quad::wrap_angle` became `pub(crate)` so the unpack's wheel spin can use it (one word; nothing else
  in quad.rs changed).
- `tests/mxvatv_bike_ride.rs`: `UpdateInRam` now serves `Out::Gear`, `Trick`, `NetCheck` and
  `NetUnpack` with the ports, after it asserts the bike's slot holds the original function. It maps
  each new `Out` to the original it stands for, so the replays run the ports with the originals below
  them. The F39 rig follows the `TrickEnd(u32)` change. `UpSays` gets the two new answers, and
  `UpdateRig::check` now compares v0 for the gear, trick step and net check too.
- New oracle `the_trick_step_gear_and_net_virtuals_are_the_originals`.
- research/oracle.txt `# F42`: every address and word used.

## What the original does

`./kit dis 0x004daab0 --len 16 -p atv`, `./kit fn 0x004e49c0 -o atv`, `./kit fn 0x004a86f0 -o atv`,
`./kit fn 0x004dd490 -o atv`. The words are in oracle.txt `# F42`. In short:

- The gear is the rear wheel's gear byte.
- The trick step clears its held word. A remote bike answers through its own stunt clocks. Otherwise
  it takes the next stunt's table entry, and its kind (4 for one of the five kept stunts on a driven
  bike). If the clip is of the asked kind, has a length, and the clock is between min(length, 0.3 s)
  and length + slack, the player's stunt mark is set. If the clock is still at the clip's end, the clip
  is held and the step answers 1. Otherwise the clock runs on by `dt`, the fraction is set, and (for a
  stunt not kept) the mark is cleared, or the trick ends once past the clip's `+0x48`. With no stunt,
  or its clip run out, a wanted trick of the asked kind starts.
- The net check: with at least five snapshots, it walks back from the newest to the pair around the
  network clock less two lags (at most four steps; the second slot is read twice), and hands the pair
  and the clamped `t` to the interpolate (vt +0x3ec).
- The net unpack puts the body where the snapshot says (velocity, quaternion, position, the centre
  through the VU quaternion turn, the speed). It turns the node, spins both wheels by the velocity in
  the body's frame, and filters the front's steer towards 0.08 of the body's local x. Then it runs the
  wheels against the ground (clear ×2, contacts, suspension ×2, tire forces ×2), sets the air words,
  and makes the vehicle's closing call.

## How it was proved

- **Function oracle, 10 000 bikes**: `the_trick_step_gear_and_net_virtuals_are_the_originals`. It
  starts from the image (F39's rig), draws every word the four read at random, and runs each original
  against its port from the same words. Every word of the bike, body, both wheels, the fake world and
  the three globals (slack, clock pointer, lag) matches; so do the calls, in order with the same
  arguments (each hooked call also asserts f12 = dt and its fixed arguments: the mark's t0 0, the
  start's a2 0, the turn's a1 0); and so do the gear's, step's and check's v0. Paths, each taken at
  least 270 times: gear in range 7575 / past it 2425; trick remote 1213, held 487, started 2777, ended
  1255, runs on 1199, a kept stunt runs on 595, none wanted or none of the kind 2474, it fits 980, no
  stunt 2413; net check too few 2421, between 6430, clamped low 879 / high 270, older slot 2/3/4/5 2393
  / 1328 / 1095 / 2763; unpack both down 2493, one 5058, both up 2449.
- **Mutants**, each failing the oracle (reverted): the step's `time <= len + slack` to `<` (case 27),
  the net walk's 4 steps to 3 (case 3), the steer's 0.08 to 0.0801 (case 0, 3 words), the gear byte's
  +0x110 to +0x111 (case 2, the answer).
- **Replays**: both `_on_the_ports_record_pass` replays stay bit-exact with the ports in the tail. A
  temporary print counted 924 gear and 856 trick-step calls by the port over the two, with 21 marks, 12
  holds, 1 trick end and 826 trick-wanted calls among their callees. `./kit check --all`: 4821 passed.

## Not verified / not 1:1

- **The net check and net unpack never run on a replay.** Both replays are a local bike, so the vehicle
  state never takes the net path (`Out::NetCheck` / `NetUnpack`: 0 calls). Those two are proved by the
  oracle only, on random words; no recording of a remote bike exists to replay.
- **The f12 handed to the net interpolate (vt +0x3ec) and the point-into-frame (0x00195320)** in the
  unpack is the caller's dt in `UpdateInRam`. The oracle checks neither call's f12. If the original
  leaves something else there and the callee reads it, the replays would not show it (they never reach
  the net path). The interpolate's `t` (f13) is checked.
- **The callees are still the originals**: the remote stunt clocks 0x004e4900, clip hold 0x004e4810,
  trick end 0x004e4db0, trick wanted 0x004e47a0, stunt start 0x004e6370, the stunt mark 0x0046b110, the
  interpolate 0x004e7bc0, the node turn 0x00194650, and the unpack's wheel clear, contacts,
  suspension, tire forces and net post. In the oracle they are hooks that log and answer, so it proves
  the ports' calls and their use of the answers, not the callees. The wheel ones have ports in
  `wheel.rs` / `contacts`, but `UpdateInRam` runs the originals because the task says so. Follow-up
  filed.
- **The oracle's quaternions are random words, not unit quaternions**, and the snapshot times are
  drawn about the wanted time with a 1-in-20 tie. This tests the arithmetic bit for bit, but not
  whether a real remote bike's words take another path. No real one exists here.
- The F39 rig's `U_AREA` is reused for F42's fakes (a clock, filter, snapshot, times, table, clips),
  rewritten every case. A word the originals read outside those, the bike, the body and the wheels is
  the image's. Such a word would be the same for both sides, so the comparison holds, but its value is
  not varied.
- `./kit audit` reports 4 leaks, all in atv_wheel.rs and plan/DONE.md, the same 4 F39 met; none come
  from this change.

## Next

None for F42. Follow-up F45 filed for the step's, interpolate's, node turn's and unpack's callees.
