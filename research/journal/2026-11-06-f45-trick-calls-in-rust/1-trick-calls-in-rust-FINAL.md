# 1 — trick calls in rust (FINAL)

Task F45. 2026-11-06.

## What I did

- `handling::update::stunt_clocks` (the remote bike's stunt clocks, vt `+0x4e8`) and
  `update::trick_wanted` (vt `+0x4b4`): new ports.
- `handling::node::orient` (the unpack's node turn: the original's node placement called with no frame
  of reference and a position) is new too. It is the RAM twin of `atv_moto::Scene::place`.
- tests/mxvatv_bike_ride.rs `UpdateInRam::out` now serves these calls with ports. It does so only where
  the bike's vtable slot holds the known original and no oracle hooks that original:
  - TrickRemote, TrickWanted, Orient;
  - TrickEnd and TrickStart (F31b5g's `trick_end` / `stunt_start`);
  - the player's stunt mark and the run's open/score/close/tally/end, through `serve_run` (F31b5h's
    `run` ports) with a new `RunHanded` adapter. The run's event posts still go to the original's post
    function in the interpreter, so the RAM stays whole. The fallback's Run* arm is now `unreachable!`.
- The F45 serving counts each port it serves (`F45_SERVED`). `ride/flip_replays_with_the_trick_calls_in_rust`
  assert those counts.
- F31b5c's census lists of callees left in the interpreter (`RIDE_CALLEES`, `FLIP_CALLEES`) lose
  TrickWanted, and flip-1's also loses Mark and TrickEnd, now that the ports serve them.
- The net path is split off as **F45a**. That covers the net interpolate, the unpack's wheel clear,
  contacts, suspension and tire forces, and the net post (see Next). Why:
  - Each needs a RAM adapter (net_interpolate) or a terrain-backed `quad::Steps` on the bike.
  - The net post has no port at all, and neither do its callees.
  - No replay has a remote bike to prove any of them in the frame.

## What the original does

Full detail in research/oracle.txt `# F45`. Recipes: `./kit dis 0x004e4900 -p atv`,
`./kit dis 0x004e47a0 -p atv` (both are vtable-only), `./kit dis 0x00194650 -p atv`.

- **Stunt clocks**: look up the stunt entry for the next stunt (an id under 46, signed).
  - No entry: clear `+0x14d8`, `+0x14e8`, `+0x14dc` and answer 0.
  - Hold flag `+0x1788` set: hand off to the trick hold and answer 1.
  - Otherwise: add `dt` to `+0x14d8`, set `+0x14e4` = min(clock / the clip's run, 1). Below 1 answers 1;
    otherwise clear the clocks as above and answer 0.
- **Trick wanted**: 0 if any of these hold, else 1:
  - `+0x324` is set;
  - the `+0x34` of the object at `+0xc8` is set;
  - `+0x14d4` is clear;
  - either nibble of the kind byte `+0x14ec` is zero;
  - `+0x13e4` is set.
- **Node turn** (no frame of reference, a position):
  - Build the quaternion's frame at the position.
  - With a parent: bring the parent's world frame up to date and multiply by its inverse.
  - Without a parent: copy the frame as it is. Its position row is the position's own words, so a
    denormal stays a denormal. The first port flushed it to −0, and the oracle caught that at case 20.
  - Then mark the node moved.

## How it was proved

- Function oracles, each over 10 000 generated cases on state 4's image (tests/mxvatv_bike_ride.rs):
  - `the_stunt_clocks_are_the_originals`. Paths: held 1839, no entry 2460, running 3759, run out 1942.
    It compares the answer, the bike's words, the stunt table, the clip and the noise knot, and checks
    the original writes nothing else outside its stack except the noise's one-time table build.
  - `the_trick_wanted_is_the_originals`. All 7 deciding clauses come up, each ≥263 times, and
    "wanted" 363 times. It compares the answer, and the original writes nothing.
  - `the_node_turn_is_the_originals`. F43's random forests: root 4141, parent 2375, a stale parent
    3484, with children 4020. Every word of the forest matches, and nothing is written outside it.
- Mutants:
  - `frac < 1` → `<=` in the stunt clocks: killed.
  - dropping the `+0x13e4` clause from trick wanted: killed.
  - The node turn's flushed position was a real bug, and the oracle caught it.
- Replays (whole RAM outside the stack, every frame, against the original's update):
  - `ride_replays_with_the_trick_calls_in_rust`: TrickWanted served 459 times.
  - `flip_replays_with_the_trick_calls_in_rust`: TrickWanted 367, Mark 21, TrickEnd 1.
  - F42's `the_trick_step_gear_and_net_virtuals_are_the_originals` and `flip_replays_bit_exact_on_quad_steps`
    still pass with the new serving.

## Not verified / not 1:1

- **The net path is not ported**: the net interpolate, the unpack's wheel clear, contacts, suspension
  and tire forces, and the net post. They are still handed to the originals. Split off as F45a; reasons
  are under What I did.
- **TrickRemote, TrickStart and Orient are never reached in a replay.** There is no remote bike, and
  the step never starts a stunt. They are proven by oracle only (TrickStart by F31b5g's), never inside
  a whole frame.
- **No Run* call (open/score/close/tally/end) is reached in a replay.** Flip-1's one TrickEnd had the
  run-open bit clear. They are proven by F31b5h's oracles only, so the `RunHanded` post handing is
  unexercised.
  - Its post call passes integer arguments only. If the post function read f12 here, nothing would show
    it. F31b5h logged the posts and compared them rather than running them.
- **The node turn's other paths are not ported**: a frame of reference given, or no position. The
  unpack never calls those, and `orient` covers only the shape the unpack uses.
- **The PC exe was not read for these functions.** They are short, and the PS2 disassembly was
  unambiguous. The address I first looked up belonged to a different exe, so I dropped that cross-check.
- opentennis has nothing relevant: it has no stunt or node code like this.

## Next

F45a: see its plan entry. It needs a capture with a remote bike first (2p or online) to prove the
ports in the frame.
