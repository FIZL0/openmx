# 1 — scene node calls (FINAL)

Task F43. 2026-11-05.

## What I did

- Ported the ten scene-node calls the bike's vehicle update still ran in the interpreter as
  `game_sim::handling::node` (`crates/game-sim/src/handling/node.rs`), over a word-level `Ram` trait:
  `place`, `rotate`, `local` (point into frame), `position`, `axes`, `frame`, `set_frame` (part from
  frame), `set`, `aim`, `spin`, plus the shared `update` / `moved` and the spin's `sin_cos`. The F39
  `Out` calls are unchanged, so the F39 oracle's call logs still compare the same way.
- `tests/mxvatv_bike_ride.rs`: `UpdateInRam` now serves `NodeSet`, `Spin`, `Aim`, `Pose`, `Place`,
  `Frame`, `Position`, `Axes`, `NodePlace`, `Local` and `Rotate` with the ports, through a `NodeRam`
  adapter over the `Ee`. The census lists (`RIDE_CALLEES` 23 → 13, `FLIP_CALLEES` 43 → 33) drop the ten.
  The scratch slots only the originals used are gone.
- `atv_moto::point_in_frame` and `frame_point` are now `pub(crate)` so the node port can reuse them.
- Split the rest of the brief, filed with `plan add F43`:
  - **F43a**: the anim controller's stop, play and blend.
  - **F43b**: the collision record's search and the attachment move.

  Why: each one pulls in a deep tree of callees (the anim clip mixer and joint blending; a
  2228-byte search with its own terrain and list callees; the attachment move's own walk). Any one of
  them is a task the size of this one. The ten node calls form one closed set, with shared update and
  moved logic, and they could be proved together.

## What the original does

The addresses and the node layout are in research/oracle.txt `# F43`. In words:

- **The node and its world frame.** A node has an own frame and a world frame. A flag bit says
  whether the world frame is current, and a second bit marks its bounds stale.
  - Every reader first brings the world frame up to date, top down from the first current parent.
    A root copies its own frame as words. A child multiplies its own frame by its parent's world frame.
  - A writer then calls *moved*. On a current node only, this marks the node's and every parent's
    bounds stale, clears the node's current bit, and puts its subtree out of date. That recursion
    walks the sibling list first, then a current node's children.
- **Set** takes the point into the parent's world frame first, when there is a parent.
- **Aim** builds the shared basis (orthogonalised, forward kept) at the node's own position. It
  ignores the parent.
- **Spin**:
  - It normalizes the axis with an FPU `mula`/`madda`/`madd` and `rsqrt`.
  - It takes the sine and cosine off the curve table: the angle is wrapped twice, then the entry is
    interpolated.
  - It builds the axis rotation on VU0. The axis's `w` lane is the `mfc1` sign extension of `z`:
    all ones when `z` is negative, and the oracle needs that.
  - It puts the old position back.
- **Words copied, not computed.** The original moves several words without doing arithmetic on
  them, so a denormal comes through unchanged. The port must copy those words, not round-trip them
  through `Vf`, which flushes denormals to zero. The oracle found all four:
  - a root's world frame;
  - the frame copies;
  - aim's position `xyz`;
  - the `w` word of place, rotate and point into frame.

## How it was proved

- **Function oracle, 10 000 random forests.** `the_scene_node_calls_are_the_originals`, in
  tests/mxvatv_bike_ride.rs.
  - Setup: six nodes per forest, every word random (small floats, exact values, any exponent, any
    bits). The links form a real forest, and the flags are either in order or arbitrary.
  - Each case makes one of the ten calls, in the update's shapes, on a random node.
  - The original runs in the interpreter, then the port runs on the same RAM. Every word of the
    forest and of the in and out quads must match.
  - The original must write nothing outside the forest and its stack.
  - Path counts must each reach 50 or more: each call × root/parent × current/stale, a stale parent,
    children put out of date, and spin's `z < 0`.
- **The sine table and the sine/cosine.** `the_spins_sine_and_cosine_are_the_originals`:
  - `CurveTable::build()` matches the image's table word for word;
  - 100 000 angles give the original's sine and cosine words, for small angles, multiples of π/4 out
    to ±39 270, and values out to ±10 737.
- **Replays.**
  - `ride_replays_bit_exact_on_the_ports_record_pass` (ride-2, 459 rows) and
    `flip_wrecks_and_is_put_back_bit_exact_on_the_ports_record_pass` (flip-1, 465 rows) stay
    bit-exact with the node ports serving the update.
  - The census tests confirm the ten labels are no longer interpreter calls.
  - All 43+2 tests in mxvatv_bike_ride pass.
- **Mutants.** Four were killed:
  - no sibling recursion;
  - no `z` sign lane;
  - no bit 7 on parents;
  - moved on a stale node.

  Three survived, and each is equivalent:
  - the point-into-frame column's `w` lane (that lane is overwritten);
  - one wrap instead of two (on finite inputs the first wrap already lands in −π..π);
  - `!(w <= π)` read as `w > π` (the PS2 has no NaN).

## Not verified / not 1:1

- **Only the update's call shapes.** The calls are proved only as the bike's update makes them: set
  with its second argument 0; aim with its three flags 1; frame, position and axes with their second
  argument 0. The original's other branches (another a1 or flags) are not ported. No caller here
  uses them.
- **Spin at huge angles.** At angles where the double wrap cannot reach −π..π (beyond about ±1e9,
  where `cvt.w.s` saturates or float spacing exceeds 2π), the original indexes past its table and
  reads whatever is there. The port would panic on the out-of-range index. Oracle angles go to about
  ±4e4, and the bike's angles are small.
- **The second wrap.** No input found makes it change anything (see the surviving mutant). It is
  ported as the original does it, but its effect is unproved.
- **Only the trees the oracle drew.** Deeper or wider node trees than the oracle's six nodes are not
  drawn. The update and recursion are generic, and the replays run the bike's real rider chain.
- **The split work (F43a, F43b).** The anim stop/play/blend, the record's search and the attachment
  move still run in the interpreter. The census lists still name them.

## Next

(FINAL.) Hand over to F43a (anim stop/play/blend) and F43b (search and attachment move).
